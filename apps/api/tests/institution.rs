mod common;

use axum::http::StatusCode;
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A Village with a User holding `village.manage` and `audit.read`; their session cookie.
async fn manager(pool: &PgPool) -> (TestApp, String) {
    pranata::village::set_code(pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app
        .user_with(pool, "operator", &["village.manage", "audit.read"])
        .await;
    (app, cookie)
}

fn bpd() -> Value {
    json!({
        "name": " Badan Permusyawaratan Desa ", "abbreviation": " BPD ",
        "description": "Menampung dan menyalurkan aspirasi masyarakat.",
        "members": [
            { "name": " Sutrisno ", "role": " Ketua " },
            { "name": "Wahyuni", "role": "Sekretaris" },
        ],
    })
}

async fn create(app: &TestApp, cookie: &str, body: &Value) -> Reply {
    app.call("POST", "/institutions", Some(body), Some(cookie))
        .await
}

async fn audit(app: &TestApp, cookie: &str) -> Value {
    app.call("GET", "/audit-entries", None, Some(cookie))
        .await
        .body["entries"]
        .clone()
}

#[sqlx::test]
async fn a_village_manager_records_an_institution_and_the_public_lists_it(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let logo = app
        .upload("/village/logo", common::png(64, 64), &cookie)
        .await;
    let mut body = bpd();
    body["logo"] = logo.body["id"].clone();

    let reply = create(&app, &cookie, &body).await;

    assert_eq!(reply.status, StatusCode::CREATED);
    let id = reply.body["id"].as_str().unwrap().to_owned();
    let (status, list) = app.get("/institutions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        list,
        json!([{
            "id": id, "name": "Badan Permusyawaratan Desa", "abbreviation": "BPD",
            "description": "Menampung dan menyalurkan aspirasi masyarakat.",
            "logo": logo.body["id"],
            "members": [
                { "name": "Sutrisno", "role": "Ketua" },
                { "name": "Wahyuni", "role": "Sekretaris" },
            ],
        }])
    );
    let entry = &audit(&app, &cookie).await[0];
    assert_eq!(entry["action"], "institution.create");
    assert_eq!(entry["objectType"], "institution");
    assert_eq!(entry["objectId"], id);
    assert!(entry["changes"].as_array().unwrap().contains(
        &json!({ "field": "abbreviation", "from": null, "to": "BPD", "sensitive": false })
    ));
}

#[sqlx::test]
async fn members_are_reordered_and_only_changes_are_audited(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let id = create(&app, &cookie, &bpd()).await.body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut edit = bpd();
    edit["members"] = json!([
        { "name": "Wahyuni", "role": "Ketua" },
        { "name": "Sutrisno", "role": "Anggota" },
        { "name": "Darto", "role": "Anggota" },
    ]);

    let reply = app
        .call(
            "PUT",
            &format!("/institutions/{id}"),
            Some(&edit),
            Some(&cookie),
        )
        .await;

    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let (_, list) = app.get("/institutions").await;
    assert_eq!(list[0]["members"], edit["members"]);
    let entry = &audit(&app, &cookie).await[0];
    assert_eq!(entry["action"], "institution.update");
    assert_eq!(
        entry["changes"],
        json!([{
            "field": "members",
            "from": [
                { "name": "Sutrisno", "role": "Ketua" },
                { "name": "Wahyuni", "role": "Sekretaris" },
            ],
            "to": edit["members"],
            "sensitive": false,
        }])
    );

    // Saving it unchanged records nothing.
    let before = audit(&app, &cookie).await;
    app.call(
        "PUT",
        &format!("/institutions/{id}"),
        Some(&edit),
        Some(&cookie),
    )
    .await;
    assert_eq!(audit(&app, &cookie).await, before);

    let reply = app
        .call(
            "DELETE",
            &format!("/institutions/{id}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/institutions").await.1, json!([]));
    assert_eq!(
        audit(&app, &cookie).await[0]["action"],
        "institution.delete"
    );
    let reply = app
        .call(
            "DELETE",
            &format!("/institutions/{id}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn a_malformed_institution_is_refused(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;

    for (field, value) in [
        ("name", json!(" ")),
        ("description", json!("")),
        ("abbreviation", json!("X".repeat(31))),
        ("members", json!([{ "name": "Sutrisno", "role": " " }])),
        ("members", json!([{ "name": "", "role": "Ketua" }])),
        ("logo", json!("00000000-0000-7000-8000-000000000000")),
    ] {
        let mut bad = bpd();
        bad[field] = value;
        let status = create(&app, &cookie, &bad).await.status;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{field}");
    }
}

#[sqlx::test]
async fn only_a_village_manager_changes_institutions(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let reader = app.user_with(&pool, "reader", &[]).await;
    let id = create(&app, &cookie, &bpd()).await.body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = format!("/institutions/{id}");

    assert_eq!(
        create(&app, &reader, &bpd()).await.status,
        StatusCode::FORBIDDEN
    );
    for method in ["PUT", "DELETE"] {
        let reply = app.call(method, &path, Some(&bpd()), Some(&reader)).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{method}");
    }
    let (status, _) = app.post("/institutions", &bpd()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn a_replaced_or_deleted_institution_logo_gets_another_24_hours(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let upload = || async {
        app.upload("/village/logo", common::png(64, 64), &cookie)
            .await
            .body["id"]
            .clone()
    };
    // As if a day had gone by since every upload.
    let age = || async {
        sqlx::query("UPDATE files SET sweep_after = now() - interval '1 second'")
            .execute(&pool)
            .await
            .unwrap();
    };
    let fresh = |logo: Value| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, bool>(
                "SELECT sweep_after > now() + interval '23 hours' FROM files WHERE id = $1::uuid",
            )
            .bind(logo.as_str().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let first = upload().await;
    let mut body = bpd();
    body["logo"] = first.clone();
    let id = create(&app, &cookie, &body).await.body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    age().await;
    let swept = pranata::storage::sweep(&pool, app.files.path())
        .await
        .unwrap();
    assert_eq!(swept, 0);

    let second = upload().await;
    body["logo"] = second.clone();
    let path = format!("/institutions/{id}");
    let reply = app.call("PUT", &path, Some(&body), Some(&cookie)).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert!(fresh(first).await);

    age().await;
    app.call("DELETE", &path, None, Some(&cookie)).await;
    assert!(fresh(second).await);
}
