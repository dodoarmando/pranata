mod common;

use axum::http::StatusCode;
use common::{PASSWORD, TestApp, authenticator::Authenticator};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

async fn role_id(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM roles WHERE name = $1")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn admin(app: &TestApp, pool: &PgPool) -> String {
    app.administrator(pool, "admin").await;
    app.sign_in("admin", false).await
}

/// Creates a User through the API and returns `(id, Access Link token)`.
async fn create_user(app: &TestApp, cookie: &str, username: &str, role: Uuid) -> (String, String) {
    let reply = app
        .call(
            "POST",
            "/users",
            Some(&json!({ "username": username, "roleId": role })),
            Some(cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    (
        reply.body["userId"].as_str().unwrap().to_owned(),
        reply.body["token"].as_str().unwrap().to_owned(),
    )
}

async fn use_link(app: &TestApp, token: &str) -> StatusCode {
    app.post(
        "/auth/access-link",
        &json!({ "token": token, "password": PASSWORD }),
    )
    .await
    .0
}

async fn actions(pool: &PgPool, object_id: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT action FROM audit_entries WHERE object_id = $1 ORDER BY id")
        .bind(object_id)
        .fetch_all(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn the_administrator_creates_a_user_who_sets_a_password_from_the_access_link(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let operator = role_id(&pool, "Operator").await;

    let (id, token) = create_user(&app, &cookie, "kaur.umum", operator).await;

    assert!(token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(use_link(&app, &token).await, StatusCode::NO_CONTENT);
    let kaur = app.sign_in("kaur.umum", false).await;
    let me = app.call("GET", "/auth/me", None, Some(&kaur)).await;
    assert_eq!(me.body["role"], "Operator");
    let users = app.call("GET", "/users", None, Some(&cookie)).await;
    assert_eq!(users.status, StatusCode::OK);
    let kaur = users
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == id.as_str())
        .unwrap();
    assert_eq!(kaur["username"], "kaur.umum");
    assert_eq!(kaur["roleId"], operator.to_string());
    assert_eq!(kaur["role"], "Operator");
    assert_eq!(
        actions(&pool, &id).await,
        [
            "user.create",
            "access_link.create",
            "access_link.use",
            "session.sign_in"
        ]
    );
    let details: Value = sqlx::query_scalar(
        "SELECT details FROM audit_entries WHERE object_id = $1 AND action = 'user.create'",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        details,
        json!({ "username": "kaur.umum", "role": "Operator" })
    );
}

#[sqlx::test]
async fn a_new_user_needs_a_valid_free_username_and_an_existing_role(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let operator = role_id(&pool, "Operator").await;

    for (username, role, status) in [
        ("Kaur Umum", operator, StatusCode::UNPROCESSABLE_ENTITY),
        ("admin", operator, StatusCode::CONFLICT),
        ("kaur", Uuid::nil(), StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let reply = app
            .call(
                "POST",
                "/users",
                Some(&json!({ "username": username, "roleId": role })),
                Some(&cookie),
            )
            .await;
        assert_eq!(reply.status, status, "{username}");
    }
}

#[sqlx::test]
async fn a_new_access_link_voids_the_previous_and_revokes_the_sessions(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let (id, first) = create_user(&app, &cookie, "kaur", role_id(&pool, "Operator").await).await;
    let (_, unused) = create_user(&app, &cookie, "kasi", role_id(&pool, "Operator").await).await;
    assert_eq!(use_link(&app, &first).await, StatusCode::NO_CONTENT);
    let kaur = app.sign_in("kaur", false).await;

    let reply = app
        .call(
            "POST",
            &format!("/users/{id}/access-link"),
            None,
            Some(&cookie),
        )
        .await;

    assert_eq!(reply.status, StatusCode::CREATED);
    assert_eq!(reply.body["userId"], id.as_str());
    let me = app.call("GET", "/auth/me", None, Some(&kaur)).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    assert_eq!(use_link(&app, &first).await, StatusCode::NOT_FOUND);
    assert_eq!(
        use_link(&app, reply.body["token"].as_str().unwrap()).await,
        StatusCode::NO_CONTENT
    );
    // Another User's link is untouched.
    assert_eq!(use_link(&app, &unused).await, StatusCode::NO_CONTENT);
    let resets: Vec<_> = actions(&pool, &id)
        .await
        .into_iter()
        .skip(4)
        .take(2)
        .collect();
    assert_eq!(resets, ["user.sessions_revoke", "access_link.create"]);
}

#[sqlx::test]
async fn the_administrator_revokes_all_of_a_users_sessions(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let (id, token) = create_user(&app, &cookie, "kaur", role_id(&pool, "Operator").await).await;
    use_link(&app, &token).await;
    let phone = app.sign_in("kaur", false).await;
    let laptop = app.sign_in("kaur", true).await;

    let reply = app
        .call(
            "DELETE",
            &format!("/users/{id}/sessions"),
            None,
            Some(&cookie),
        )
        .await;

    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    for session in [phone, laptop] {
        let me = app.call("GET", "/auth/me", None, Some(&session)).await;
        assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        actions(&pool, &id).await.last().unwrap(),
        "user.sessions_revoke"
    );
    // The Administrator's own session lives on.
    let me = app.call("GET", "/auth/me", None, Some(&cookie)).await;
    assert_eq!(me.status, StatusCode::OK);
}

#[sqlx::test]
async fn the_administrator_removes_all_of_a_users_passkeys(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let (id, token) = create_user(&app, &cookie, "kaur", role_id(&pool, "Operator").await).await;
    use_link(&app, &token).await;
    let kaur = app.sign_in("kaur", false).await;
    app.add_passkey(&kaur, "Laptop", &Authenticator::new(1))
        .await;
    app.add_passkey(&kaur, "Ponsel", &Authenticator::new(2))
        .await;
    app.add_passkey(&cookie, "Milik admin", &Authenticator::new(3))
        .await;
    let listed = app.call("GET", "/users", None, Some(&cookie)).await;
    let counts: Vec<_> = listed
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["passkeys"].clone())
        .collect();
    assert_eq!(counts, [json!(1), json!(2)]);

    let reply = app
        .call(
            "DELETE",
            &format!("/users/{id}/passkeys"),
            None,
            Some(&cookie),
        )
        .await;

    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let own = app.call("GET", "/auth/passkeys", None, Some(&kaur)).await;
    assert_eq!(own.body, json!([]));
    let details: Value = sqlx::query_scalar(
        "SELECT details FROM audit_entries WHERE action = 'user.passkeys_remove' AND object_id = $1",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(details, json!({ "passkeysRemoved": 2 }));
    // The Administrator's own passkey stays.
    let mine = app.call("GET", "/auth/passkeys", None, Some(&cookie)).await;
    assert_eq!(mine.body.as_array().unwrap().len(), 1);
}

#[sqlx::test]
async fn acting_on_an_unknown_user_is_not_found(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let id = Uuid::nil();

    for (method, path) in [
        ("POST", format!("/users/{id}/access-link")),
        ("DELETE", format!("/users/{id}/sessions")),
        ("DELETE", format!("/users/{id}/passkeys")),
    ] {
        let reply = app.call(method, &path, None, Some(&cookie)).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{method} {path}");
    }
}

#[sqlx::test]
async fn only_the_administrator_role_manages_users(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let all = [
        "village.manage",
        "content.write",
        "content.publish",
        "complaint.read",
        "complaint.handle",
        "audit.read",
    ];
    let cookie = app.user_with(&pool, "kades", &all).await;
    let id = Uuid::nil();
    let body = json!({ "username": "kaur", "roleId": role_id(&pool, "Operator").await });

    for (method, path, body) in [
        ("GET", "/users".to_owned(), None),
        ("POST", "/users".to_owned(), Some(&body)),
        ("POST", format!("/users/{id}/access-link"), None),
        ("DELETE", format!("/users/{id}/sessions"), None),
        ("DELETE", format!("/users/{id}/passkeys"), None),
    ] {
        let reply = app.call(method, &path, body, Some(&cookie)).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{method} {path}");
    }
}

#[sqlx::test]
async fn actions_on_users_need_sudo_mode(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let (id, _) = create_user(&app, &cookie, "kaur", role_id(&pool, "Operator").await).await;
    sqlx::query("UPDATE sessions SET authenticated_at = now() - interval '16 minutes'")
        .execute(&pool)
        .await
        .unwrap();
    let body = json!({ "username": "kasi", "roleId": role_id(&pool, "Operator").await });

    for (method, path, body) in [
        ("POST", "/users".to_owned(), Some(&body)),
        ("POST", format!("/users/{id}/access-link"), None),
        ("DELETE", format!("/users/{id}/sessions"), None),
        ("DELETE", format!("/users/{id}/passkeys"), None),
    ] {
        let reply = app.call(method, &path, body, Some(&cookie)).await;
        assert_eq!(
            reply.status,
            StatusCode::PRECONDITION_REQUIRED,
            "{method} {path}"
        );
    }
    let users = app.call("GET", "/users", None, Some(&cookie)).await;
    assert_eq!(users.status, StatusCode::OK);
}

async fn patch(app: &TestApp, cookie: &str, id: &str, body: Value) -> StatusCode {
    app.call("PATCH", &format!("/users/{id}"), Some(&body), Some(cookie))
        .await
        .status
}

async fn last_changes(pool: &PgPool, id: &str) -> (String, Value) {
    sqlx::query_as(
        "SELECT action, changes FROM audit_entries WHERE object_id = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn deactivating_a_user_signs_them_out_at_once_until_reactivated(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let (id, token) = create_user(&app, &cookie, "kaur", role_id(&pool, "Operator").await).await;
    use_link(&app, &token).await;
    let kaur = app.sign_in("kaur", false).await;

    assert_eq!(
        patch(&app, &cookie, &id, json!({ "active": false })).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        last_changes(&pool, &id).await,
        (
            "user.update".to_owned(),
            json!([{ "field": "active", "from": true, "to": false }])
        )
    );

    let me = app.call("GET", "/auth/me", None, Some(&kaur)).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    let sign_in = app
        .call(
            "POST",
            "/auth/sign-in",
            Some(&json!({ "username": "kaur", "password": PASSWORD, "remember": false })),
            None,
        )
        .await;
    assert_eq!(sign_in.status, StatusCode::UNAUTHORIZED);
    let users = app.call("GET", "/users", None, Some(&cookie)).await.body;
    let kaur = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == id.as_str())
        .unwrap();
    assert_eq!(kaur["active"], false);

    assert_eq!(
        patch(&app, &cookie, &id, json!({ "active": true })).await,
        StatusCode::NO_CONTENT
    );
    app.sign_in("kaur", false).await;
}

#[sqlx::test]
async fn a_new_role_applies_on_the_users_next_request(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let (id, token) = create_user(&app, &cookie, "kaur", role_id(&pool, "Operator").await).await;
    use_link(&app, &token).await;
    let kaur = app.sign_in("kaur", false).await;
    let kontributor = role_id(&pool, "Kontributor Web").await;

    assert_eq!(
        patch(&app, &cookie, &id, json!({ "roleId": kontributor })).await,
        StatusCode::NO_CONTENT
    );

    let me = app.call("GET", "/auth/me", None, Some(&kaur)).await;
    assert_eq!(me.body["role"], "Kontributor Web");
    assert_eq!(
        last_changes(&pool, &id).await,
        (
            "user.update".to_owned(),
            json!([{ "field": "role", "from": "Operator", "to": "Kontributor Web" }])
        )
    );
}

#[sqlx::test]
async fn the_last_active_administrator_is_never_deactivated_or_demoted(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let me = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE username = 'admin'")
        .fetch_one(&pool)
        .await
        .unwrap()
        .to_string();
    let administrator = role_id(&pool, "Administrator").await;
    let operator = role_id(&pool, "Operator").await;
    let (other, _) = create_user(&app, &cookie, "admin2", administrator).await;

    // An inactive Administrator does not count.
    assert_eq!(
        patch(&app, &cookie, &other, json!({ "active": false })).await,
        StatusCode::NO_CONTENT
    );
    for body in [json!({ "active": false }), json!({ "roleId": operator })] {
        assert_eq!(
            patch(&app, &cookie, &me, body.clone()).await,
            StatusCode::CONFLICT,
            "{body}"
        );
    }
    let still: bool =
        sqlx::query_scalar("SELECT active AND role_id = $1 FROM users WHERE username = 'admin'")
            .bind(administrator)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(still);

    // With another active Administrator, stepping down is allowed.
    assert_eq!(
        patch(&app, &cookie, &other, json!({ "active": true })).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        patch(&app, &cookie, &me, json!({ "roleId": operator })).await,
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test]
async fn changing_a_user_needs_a_known_user_and_role(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let (id, _) = create_user(&app, &cookie, "kaur", role_id(&pool, "Operator").await).await;

    let unknown_user = patch(
        &app,
        &cookie,
        &Uuid::nil().to_string(),
        json!({ "active": false }),
    )
    .await;
    let unknown_role = patch(&app, &cookie, &id, json!({ "roleId": Uuid::nil() })).await;

    assert_eq!(unknown_user, StatusCode::NOT_FOUND);
    assert_eq!(unknown_role, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn changing_a_user_is_for_the_administrator_in_sudo_mode(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let (id, _) = create_user(&app, &cookie, "kaur", role_id(&pool, "Operator").await).await;
    let kades = app.user_with(&pool, "kades", &["audit.read"]).await;

    assert_eq!(
        patch(&app, &kades, &id, json!({ "active": false })).await,
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE sessions SET authenticated_at = now() - interval '16 minutes'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        patch(&app, &cookie, &id, json!({ "active": false })).await,
        StatusCode::PRECONDITION_REQUIRED
    );
}

#[sqlx::test]
async fn a_change_that_changes_nothing_writes_no_audit_entry(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let operator = role_id(&pool, "Operator").await;
    let (id, _) = create_user(&app, &cookie, "kaur", operator).await;

    for body in [
        json!({ "active": true }),
        json!({ "roleId": operator }),
        json!({}),
    ] {
        assert_eq!(
            patch(&app, &cookie, &id, body).await,
            StatusCode::NO_CONTENT
        );
    }

    assert_eq!(
        actions(&pool, &id).await,
        ["user.create", "access_link.create"]
    );
}

#[sqlx::test]
async fn an_inactive_user_gets_no_access_link_until_reactivated(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let (id, token) = create_user(&app, &cookie, "kaur", role_id(&pool, "Operator").await).await;
    patch(&app, &cookie, &id, json!({ "active": false })).await;
    let path = format!("/users/{id}/access-link");
    let reset = || app.call("POST", &path, None, Some(&cookie));

    assert_eq!(use_link(&app, &token).await, StatusCode::NOT_FOUND);
    assert_eq!(reset().await.status, StatusCode::CONFLICT);
    patch(&app, &cookie, &id, json!({ "active": true })).await;
    assert_eq!(reset().await.status, StatusCode::CREATED);
}
