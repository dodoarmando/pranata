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

fn indeks_desa(year: i64) -> Value {
    json!({
        "year": year, "kind": "indeks_desa", "score": 72.48, "status": "advanced",
        "basicServices": 81, "social": 74.5, "economic": 63.25,
    })
}

fn idm(year: i64) -> Value {
    json!({ "year": year, "kind": "idm", "score": 0.7142, "status": "advanced" })
}

async fn create(app: &TestApp, cookie: &str, body: &Value) -> Reply {
    app.call("POST", "/index-editions", Some(body), Some(cookie))
        .await
}

async fn audit(app: &TestApp, cookie: &str) -> Value {
    app.call("GET", "/audit-entries", None, Some(cookie))
        .await
        .body["entries"]
        .clone()
}

#[sqlx::test]
async fn a_village_manager_records_editions_and_the_public_lists_the_newest_first(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;

    let old = create(&app, &cookie, &idm(2023)).await;
    let new = create(&app, &cookie, &indeks_desa(2024)).await;

    assert_eq!(old.status, StatusCode::CREATED);
    assert_eq!(new.status, StatusCode::CREATED);
    let (status, list) = app.get("/index-editions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        list,
        json!([
            {
                "id": new.body["id"], "year": 2024, "kind": "indeks_desa", "score": 72.48,
                "status": "advanced", "basicServices": 81.0, "social": 74.5,
                "economic": 63.25, "environmental": null, "accessibility": null,
                "governance": null,
            },
            {
                "id": old.body["id"], "year": 2023, "kind": "idm", "score": 0.7142,
                "status": "advanced", "basicServices": null, "social": null,
                "economic": null, "environmental": null, "accessibility": null,
                "governance": null,
            },
        ])
    );
    let entry = &audit(&app, &cookie).await[0];
    assert_eq!(entry["action"], "index_edition.create");
    assert_eq!(entry["objectType"], "index_edition");
    assert_eq!(entry["objectId"], new.body["id"]);
    assert!(
        entry["changes"]
            .as_array()
            .unwrap()
            .contains(&json!({ "field": "score", "from": null, "to": 72.48, "sensitive": false }))
    );
}

#[sqlx::test]
async fn scores_outside_their_kinds_range_or_precision_are_refused(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;

    for (base, field, value) in [
        (indeks_desa(2024), "score", json!(100.01)),
        (indeks_desa(2024), "score", json!(-0.01)),
        (indeks_desa(2024), "score", json!(72.485)),
        (indeks_desa(2024), "governance", json!(100.5)),
        (indeks_desa(2024), "social", json!(70.001)),
        (indeks_desa(2024), "economic", json!(-1)),
        (indeks_desa(2024), "year", json!(1999)),
        (indeks_desa(2024), "status", json!("maju")),
        (idm(2023), "score", json!(1.0001)),
        (idm(2023), "score", json!(-0.0001)),
        (idm(2023), "score", json!(0.71425)),
        (idm(2023), "score", json!(72.48)),
        (idm(2023), "economic", json!(63)),
    ] {
        let mut bad = base;
        bad[field] = value.clone();
        let status = create(&app, &cookie, &bad).await.status;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{field} {value}");
    }

    // The bounds themselves are scores.
    let mut top = indeks_desa(2024);
    top["score"] = json!(100);
    top["governance"] = json!(0);
    assert_eq!(
        create(&app, &cookie, &top).await.status,
        StatusCode::CREATED
    );
    let mut top = idm(2023);
    top["score"] = json!(1);
    assert_eq!(
        create(&app, &cookie, &top).await.status,
        StatusCode::CREATED
    );
}

#[sqlx::test]
async fn a_year_holds_one_edition(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    create(&app, &cookie, &idm(2023)).await;
    let id = create(&app, &cookie, &indeks_desa(2024)).await.body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = format!("/index-editions/{id}");

    let again = create(&app, &cookie, &idm(2024)).await;
    let moved = app
        .call("PUT", &path, Some(&indeks_desa(2023)), Some(&cookie))
        .await;
    let kept = app
        .call("PUT", &path, Some(&indeks_desa(2024)), Some(&cookie))
        .await;

    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(moved.status, StatusCode::CONFLICT);
    assert_eq!(kept.status, StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn an_edition_is_changed_and_deleted_with_only_changes_audited(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let id = create(&app, &cookie, &indeks_desa(2024)).await.body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = format!("/index-editions/{id}");
    let mut edit = indeks_desa(2024);
    edit["status"] = json!("developing");
    edit["governance"] = json!(69);

    let reply = app.call("PUT", &path, Some(&edit), Some(&cookie)).await;

    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let (_, list) = app.get("/index-editions").await;
    assert_eq!(list[0]["status"], "developing");
    let entry = &audit(&app, &cookie).await[0];
    assert_eq!(entry["action"], "index_edition.update");
    assert_eq!(
        entry["changes"],
        json!([
            { "field": "governance", "from": null, "to": 69.0, "sensitive": false },
            { "field": "status", "from": "advanced", "to": "developing", "sensitive": false },
        ])
    );

    // Saving it unchanged records nothing.
    let before = audit(&app, &cookie).await;
    app.call("PUT", &path, Some(&edit), Some(&cookie)).await;
    assert_eq!(audit(&app, &cookie).await, before);

    let reply = app.call("DELETE", &path, None, Some(&cookie)).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/index-editions").await.1, json!([]));
    assert_eq!(
        audit(&app, &cookie).await[0]["action"],
        "index_edition.delete"
    );
    for (method, body) in [("PUT", Some(&edit)), ("DELETE", None)] {
        let reply = app.call(method, &path, body, Some(&cookie)).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{method}");
    }
}

#[sqlx::test]
async fn only_a_village_manager_changes_editions(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let reader = app.user_with(&pool, "reader", &[]).await;
    let id = create(&app, &cookie, &idm(2023)).await.body["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = format!("/index-editions/{id}");

    assert_eq!(
        create(&app, &reader, &idm(2022)).await.status,
        StatusCode::FORBIDDEN
    );
    for method in ["PUT", "DELETE"] {
        let reply = app
            .call(method, &path, Some(&idm(2023)), Some(&reader))
            .await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{method}");
    }
    let (status, _) = app.post("/index-editions", &idm(2022)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
