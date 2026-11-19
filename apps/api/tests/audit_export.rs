mod common;

use std::io::{Cursor, Read};

use axum::http::StatusCode;
use base64::{Engine, prelude::BASE64_STANDARD};
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn export(app: &TestApp, cookie: &str, query: &str, body: &Value) -> Reply {
    app.call(
        "POST",
        &format!("/audit-entries/export{query}"),
        Some(body),
        Some(cookie),
    )
    .await
}

/// The one file in the encrypted ZIP of `reply`, opened with `password`.
fn open(reply: &Reply, password: &str) -> zip::result::ZipResult<(String, String)> {
    let bytes = BASE64_STANDARD
        .decode(reply.body["content"].as_str().unwrap())
        .unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    assert_eq!(archive.len(), 1);
    let mut file = archive.by_index_decrypt(0, password.as_bytes())?;
    let mut text = String::new();
    file.read_to_string(&mut text).unwrap();
    Ok((file.name()?.into_owned(), text))
}

async fn exports(pool: &PgPool) -> Vec<(String, Option<String>, Value)> {
    sqlx::query_as(
        "SELECT users.username, object_type, details FROM audit_entries
         LEFT JOIN users ON users.id = actor_id WHERE action = 'audit_log.export' ORDER BY at",
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(
        |(username, object_type, details): (Option<String>, String, Value)| {
            (object_type, username, details)
        },
    )
    .collect()
}

async fn seed(pool: &PgPool, at: &str, (object_type, object_id): (&str, &str), changes: Value) {
    sqlx::query(
        "INSERT INTO audit_entries (at, actor, action, object_type, object_id, details, changes)
         VALUES ($1::timestamptz, 'system', 'test.change', $2, $3, '{\"note\": \"=1+1\"}', $4)",
    )
    .bind(at)
    .bind(object_type)
    .bind(object_id)
    .bind(changes)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn exporting_the_audit_log_needs_audit_read(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app
        .user_with(&pool, "operator", &["village.manage", "content.write"])
        .await;

    let reply = export(&app, &cookie, "", &json!({ "reason": "Pemeriksaan" })).await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(exports(&pool).await.len(), 0);
}

#[sqlx::test]
async fn an_export_without_a_reason_is_refused(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;

    for body in [
        json!({}),
        json!({ "reason": "" }),
        json!({ "reason": "  \n" }),
    ] {
        let reply = export(&app, &cookie, "", &body).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    assert_eq!(exports(&pool).await.len(), 0);
}

#[sqlx::test]
async fn an_export_needs_authentication_within_15_minutes(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    sqlx::query("UPDATE sessions SET authenticated_at = now() - interval '16 minutes'")
        .execute(&pool)
        .await
        .unwrap();

    let reply = export(&app, &cookie, "", &json!({ "reason": "Pemeriksaan" })).await;

    assert_eq!(reply.status, StatusCode::PRECONDITION_REQUIRED);
    assert_eq!(exports(&pool).await.len(), 0);
}

#[sqlx::test]
async fn an_export_is_a_filtered_csv_in_a_zip_only_its_one_time_password_opens(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    // Papua: WIT, UTC+9.
    pranata::village::set_code(&pool, "9103012004")
        .await
        .unwrap();
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    seed(
        &pool,
        "2026-01-01T14:30:00Z",
        ("hamlet", "1"),
        json!([{ "field": "name", "from": null, "to": "Dusun \"Satu\"" }]),
    )
    .await;
    seed(&pool, "2026-01-01T15:30:00Z", ("rt", "1"), json!([])).await;

    let reply = export(
        &app,
        &cookie,
        "?objectType=hamlet",
        &json!({ "reason": "Permintaan inspektorat" }),
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    assert_eq!(reply.body["fileName"], "log-audit.zip");
    let password = reply.body["password"].as_str().unwrap();
    assert!(password.len() >= 20, "{password}");
    assert!(open(&reply, "wrong password").is_err());
    let (name, csv) = open(&reply, password).unwrap();
    assert_eq!(name, "log-audit.csv");
    assert_eq!(
        csv,
        "\u{feff}\"Waktu (WIT)\",\"Pelaku\",\"Tindakan\",\"Jenis objek\",\"ID objek\",\"Perubahan\",\"Keterangan\"\r\n\
         \"2026-01-01 23:30:00\",\"Sistem\",\"test.change\",\"hamlet\",\"1\",\"name: (kosong) → Dusun \"\"Satu\"\"\",\"{\"\"note\"\":\"\"=1+1\"\"}\"\r\n"
    );
    // A second export draws a fresh password.
    let again = export(&app, &cookie, "", &json!({ "reason": "Arsip" })).await;
    assert_ne!(again.body["password"], reply.body["password"]);
    assert_eq!(
        exports(&pool).await,
        [
            (
                "audit_log".to_owned(),
                Some("admin".to_owned()),
                json!({ "reason": "Permintaan inspektorat", "file": "log-audit.csv", "filter": { "objectType": "hamlet" } }),
            ),
            (
                "audit_log".to_owned(),
                Some("admin".to_owned()),
                json!({ "reason": "Arsip", "file": "log-audit.csv", "filter": {} }),
            ),
        ]
    );
}

#[sqlx::test]
async fn an_exported_cell_never_starts_a_spreadsheet_formula(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    seed(
        &pool,
        "2026-01-01T00:00:00Z",
        ("=HYPERLINK(\"x\")", "-1"),
        json!([]),
    )
    .await;

    let reply = export(&app, &cookie, "?objectId=-1", &json!({ "reason": "Uji" })).await;

    let (_, csv) = open(&reply, reply.body["password"].as_str().unwrap()).unwrap();
    assert!(
        csv.contains(",\"'=HYPERLINK(\"\"x\"\")\",\"'-1\","),
        "{csv}"
    );
}

#[sqlx::test]
async fn exported_sensitive_values_are_masked_without_resident_sensitive(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let kades = app.user_with(&pool, "kades", &["audit.read"]).await;
    seed(
        &pool,
        "2026-01-01T00:00:00Z",
        ("resident", "1"),
        json!([
            { "field": "name", "from": "Budi", "to": "Budi Santoso" },
            { "field": "bloodType", "from": null, "to": "O", "sensitive": true },
        ]),
    )
    .await;

    let reply = export(
        &app,
        &kades,
        "?objectType=resident",
        &json!({ "reason": "Uji" }),
    )
    .await;

    let (_, csv) = open(&reply, reply.body["password"].as_str().unwrap()).unwrap();
    assert!(
        csv.contains("\"name: Budi → Budi Santoso; bloodType: ••• → •••\""),
        "{csv}"
    );
}
