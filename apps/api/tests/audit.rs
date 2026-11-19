mod common;

use axum::http::StatusCode;
use common::{PASSWORD, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

// The one test below the HTTP seam: append-only is a database guarantee.
#[sqlx::test]
async fn audit_entries_cannot_be_changed_or_removed(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();

    for statement in [
        "UPDATE audit_entries SET action = 'forged'",
        "DELETE FROM audit_entries",
        "TRUNCATE audit_entries",
    ] {
        assert!(
            sqlx::raw_sql(statement).execute(&pool).await.is_err(),
            "{statement} succeeded"
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

async fn page(app: &TestApp, cookie: &str, query: &str) -> Value {
    let reply = app
        .call("GET", &format!("/audit-entries{query}"), None, Some(cookie))
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{query}");
    reply.body
}

async fn entries(app: &TestApp, cookie: &str) -> Vec<Value> {
    page(app, cookie, "").await["entries"]
        .as_array()
        .unwrap()
        .clone()
}

/// Appends an Audit Entry by `actor_id` (or the system) as the database allows: insert only.
async fn seed(
    pool: &PgPool,
    at: &str,
    actor_id: Option<Uuid>,
    (object_type, object_id): (&str, &str),
    changes: Value,
) {
    sqlx::query(
        "INSERT INTO audit_entries (at, actor, actor_id, action, object_type, object_id, changes)
         VALUES ($1::timestamptz, CASE WHEN $2::uuid IS NULL THEN 'system' ELSE 'user' END, $2, 'test.change', $3, $4, $5)",
    )
    .bind(at)
    .bind(actor_id)
    .bind(object_type)
    .bind(object_id)
    .bind(changes)
    .execute(pool)
    .await
    .unwrap();
}

async fn user_id(pool: &PgPool, username: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM users WHERE username = $1")
        .bind(username)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn objects(page: &Value) -> Vec<String> {
    page["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            format!(
                "{}/{}",
                entry["objectType"].as_str().unwrap(),
                entry["objectId"].as_str().unwrap()
            )
        })
        .collect()
}

#[sqlx::test]
async fn reading_the_audit_log_needs_audit_read(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app
        .user_with(&pool, "operator", &["village.manage", "content.write"])
        .await;

    let reply = app.call("GET", "/audit-entries", None, Some(&cookie)).await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn filters_by_object_user_and_village_date_combine(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    // Papua: WIT, UTC+9.
    pranata::village::set_code(&pool, "9103012004")
        .await
        .unwrap();
    app.administrator(&pool, "admin").await;
    app.administrator(&pool, "kaur").await;
    let cookie = app.sign_in("admin", false).await;
    let (admin, kaur) = (user_id(&pool, "admin").await, user_id(&pool, "kaur").await);
    // 2026-01-01 23:30 and 2026-01-02 00:30 in the Village.
    seed(
        &pool,
        "2026-01-01T14:30:00Z",
        Some(admin),
        ("hamlet", "1"),
        json!([]),
    )
    .await;
    seed(
        &pool,
        "2026-01-01T15:30:00Z",
        Some(admin),
        ("hamlet", "2"),
        json!([]),
    )
    .await;
    seed(
        &pool,
        "2026-01-01T16:00:00Z",
        Some(kaur),
        ("hamlet", "1"),
        json!([]),
    )
    .await;
    seed(&pool, "2026-01-03T01:00:00Z", None, ("rt", "1"), json!([])).await;

    for (query, expected) in [
        (
            "?objectType=hamlet&objectId=1",
            &["hamlet/1", "hamlet/1"][..],
        ),
        ("?objectType=rt", &["rt/1"]),
        ("?username=kaur&objectType=hamlet", &["hamlet/1"]),
        ("?from=2026-01-01&to=2026-01-01", &["hamlet/1"]),
        (
            "?from=2026-01-02&to=2026-01-03",
            &["rt/1", "hamlet/1", "hamlet/2"],
        ),
        (
            "?username=admin&objectType=hamlet&from=2026-01-02",
            &["hamlet/2"],
        ),
    ] {
        assert_eq!(
            objects(&page(&app, &cookie, query).await),
            expected,
            "{query}"
        );
    }
}

#[sqlx::test]
async fn pages_hold_50_entries_newest_first_and_keep_the_filters(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    for i in 0..60 {
        let at = format!("2026-01-01T00:{i:02}:00Z");
        seed(&pool, &at, None, ("rt", &format!("{i:02}")), json!([])).await;
        seed(&pool, &at, None, ("hamlet", "1"), json!([])).await;
    }

    let first = page(&app, &cookie, "?objectType=rt").await;
    let next = first["next"].as_str().unwrap();
    let second = page(&app, &cookie, &format!("?objectType=rt&before={next}")).await;

    let expected: Vec<_> = (0..60).rev().map(|i| format!("rt/{i:02}")).collect();
    assert_eq!(objects(&first), expected[..50]);
    assert_eq!(objects(&second), expected[50..]);
    assert_eq!(second["next"], Value::Null);
}

#[sqlx::test]
async fn sensitive_values_are_masked_without_resident_sensitive(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let admin = app.sign_in("admin", false).await;
    let kades = app.user_with(&pool, "kades", &["audit.read"]).await;
    seed(
        &pool,
        "2026-01-01T00:00:00Z",
        None,
        ("resident", "1"),
        json!([
            { "field": "name", "from": "Budi", "to": "Budi Santoso" },
            { "field": "bloodType", "from": null, "to": "O", "sensitive": true },
        ]),
    )
    .await;
    let changes = |page: Value| page["entries"][0]["changes"].clone();

    assert_eq!(
        changes(page(&app, &kades, "?objectType=resident").await),
        json!([
            { "field": "name", "from": "Budi", "to": "Budi Santoso", "sensitive": false },
            { "field": "bloodType", "from": "•••", "to": "•••", "sensitive": true },
        ])
    );
    // The Administrator holds every Permission, `resident.sensitive` included.
    assert_eq!(
        changes(page(&app, &admin, "?objectType=resident").await)[1],
        json!({ "field": "bloodType", "from": null, "to": "O", "sensitive": true })
    );
}

#[sqlx::test]
async fn the_audit_log_needs_a_session(pool: PgPool) {
    let app = TestApp::new(pool);

    let (status, _) = app.get("/audit-entries").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn provisioning_and_signing_in_and_out_are_audited(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    app.administrator(&pool, "admin").await;
    app.call(
        "POST",
        "/auth/sign-in",
        Some(&json!({ "username": "admin", "password": "wrong password", "remember": false })),
        None,
    )
    .await;
    let cookie = app.sign_in("admin", false).await;
    app.call("POST", "/auth/sign-out", None, Some(&cookie))
        .await;
    let cookie = app.sign_in("admin", true).await;

    let log: Vec<_> = entries(&app, &cookie)
        .await
        .into_iter()
        .rev()
        .map(|entry| {
            (
                entry["action"].clone(),
                entry["actor"].clone(),
                entry["username"].clone(),
            )
        })
        .collect();

    let system = (json!("system"), Value::Null);
    let admin = (json!("user"), json!("admin"));
    let expected = [
        ("village.set_code", &system),
        ("user.create", &system),
        ("access_link.create", &system),
        ("access_link.use", &admin),
        ("session.sign_in_failed", &(json!("anonymous"), Value::Null)),
        ("session.sign_in", &admin),
        ("session.sign_out", &admin),
        ("session.sign_in", &admin),
    ]
    .map(|(action, (actor, username))| (json!(action), actor.clone(), username.clone()));
    assert_eq!(log, expected);
}

#[sqlx::test]
async fn a_failed_change_leaves_no_audit_entry_and_a_successful_one_exactly_one(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;
    let token = pranata::user::create_administrator(&pool, "kedua")
        .await
        .unwrap();
    let uses = || async {
        entries(&app, &cookie)
            .await
            .into_iter()
            .filter(|entry| entry["action"] == "access_link.use" && entry["username"] == "kedua")
            .count()
    };

    let (refused, _) = app
        .post(
            "/auth/access-link",
            &json!({ "token": token, "password": "short" }),
        )
        .await;
    assert_eq!(refused, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(uses().await, 0);

    let (accepted, _) = app
        .post(
            "/auth/access-link",
            &json!({ "token": token, "password": PASSWORD }),
        )
        .await;
    assert_eq!(accepted, StatusCode::NO_CONTENT);
    assert_eq!(uses().await, 1);
}
