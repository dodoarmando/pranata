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

async fn create(app: &TestApp, cookie: &str, body: Value) -> Reply {
    app.call("POST", "/regions/units", Some(&body), Some(cookie))
        .await
}

/// Creates a Region Unit that must be accepted; its id.
async fn unit(app: &TestApp, cookie: &str, body: Value) -> String {
    let reply = create(app, cookie, body.clone()).await;
    assert_eq!(reply.status, StatusCode::CREATED, "{body}");
    reply.body["id"].as_str().unwrap().to_owned()
}

async fn hamlet(app: &TestApp, cookie: &str, name: &str) -> String {
    unit(app, cookie, json!({ "kind": "hamlet", "name": name })).await
}

async fn numbered(app: &TestApp, cookie: &str, kind: &str, parent: &str, number: i32) -> String {
    unit(
        app,
        cookie,
        json!({ "kind": kind, "parentId": parent, "number": number }),
    )
    .await
}

async fn update(app: &TestApp, cookie: &str, id: &str, body: Value) -> StatusCode {
    app.call(
        "PUT",
        &format!("/regions/units/{id}"),
        Some(&body),
        Some(cookie),
    )
    .await
    .status
}

async fn set_active(
    app: &TestApp,
    cookie: &str,
    id: &str,
    number: i32,
    active: bool,
) -> StatusCode {
    update(
        app,
        cookie,
        id,
        json!({ "number": number, "active": active }),
    )
    .await
}

async fn settings(app: &TestApp, cookie: &str, label: &str, has_rw: bool) -> StatusCode {
    let body = json!({ "hamletLabel": label, "hasRw": has_rw });
    app.call("PUT", "/regions/settings", Some(&body), Some(cookie))
        .await
        .status
}

async fn tree(app: &TestApp, cookie: &str) -> Value {
    let reply = app.call("GET", "/regions", None, Some(cookie)).await;
    assert_eq!(reply.status, StatusCode::OK);
    reply.body
}

#[sqlx::test]
async fn a_village_manager_builds_the_tree_and_any_user_reads_it(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let reader = app.user_with(&pool, "reader", &[]).await;

    assert_eq!(
        settings(&app, &cookie, " Banjar ", true).await,
        StatusCode::NO_CONTENT
    );
    let krajan = hamlet(&app, &cookie, " Krajan ").await;
    let rw = numbered(&app, &cookie, "rw", &krajan, 1).await;
    let rt = unit(
        &app,
        &cookie,
        json!({
            "kind": "rt", "parentId": rw, "number": 3,
            "headName": " Pak Sastro ", "headPhone": "0812-3456-789",
        }),
    )
    .await;

    assert_eq!(
        tree(&app, &reader).await,
        json!({
            "hamletLabel": "banjar",
            "hasRw": true,
            "units": [
                { "id": krajan, "kind": "hamlet", "parentId": null, "name": "Krajan",
                  "number": null, "headName": null, "headPhone": null, "active": true },
                { "id": rw, "kind": "rw", "parentId": krajan, "name": null,
                  "number": 1, "headName": null, "headPhone": null, "active": true },
                { "id": rt, "kind": "rt", "parentId": rw, "name": null,
                  "number": 3, "headName": "Pak Sastro", "headPhone": "0812-3456-789",
                  "active": true },
            ],
        })
    );
    let audit = app
        .call("GET", "/audit-entries", None, Some(&cookie))
        .await
        .body;
    let entry = &audit["entries"][0];
    assert_eq!(entry["action"], "region.create");
    assert_eq!(entry["objectType"], "region_unit");
    assert_eq!(entry["objectId"], rt.as_str());
    assert!(entry["changes"].as_array().unwrap().contains(
        &json!({ "field": "headName", "from": null, "to": "Pak Sastro", "sensitive": false })
    ));
    assert_eq!(audit["entries"][3]["action"], "region.update_settings");
}

#[sqlx::test]
async fn units_are_renamed_and_renumbered_in_place_and_audited(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;
    let rw = numbered(&app, &cookie, "rw", &krajan, 1).await;

    let renamed = update(
        &app,
        &cookie,
        &krajan,
        json!({ "name": "Krajan Kidul", "active": true }),
    );
    let renumbered = update(
        &app,
        &cookie,
        &rw,
        json!({ "number": 2, "headName": "Bu Siti", "active": true }),
    );

    assert_eq!(renamed.await, StatusCode::NO_CONTENT);
    assert_eq!(renumbered.await, StatusCode::NO_CONTENT);
    let units = tree(&app, &cookie).await["units"].clone();
    assert_eq!(units[0]["name"], "Krajan Kidul");
    assert_eq!(units[1]["number"], 2);
    assert_eq!(units[1]["headName"], "Bu Siti");
    let audit = app
        .call("GET", "/audit-entries", None, Some(&cookie))
        .await
        .body;
    assert_eq!(audit["entries"][0]["action"], "region.update");
    assert_eq!(
        audit["entries"][0]["changes"],
        json!([
            { "field": "headName", "from": null, "to": "Bu Siti", "sensitive": false },
            { "field": "number", "from": 1, "to": 2, "sensitive": false },
        ])
    );
}

#[sqlx::test]
async fn rw_numbers_are_unique_in_the_village_and_rt_numbers_within_their_parent(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;
    let kulon = hamlet(&app, &cookie, "Kulon").await;
    let rw1 = numbered(&app, &cookie, "rw", &krajan, 1).await;
    let rw2 = numbered(&app, &cookie, "rw", &kulon, 2).await;
    numbered(&app, &cookie, "rt", &rw1, 1).await;
    let rt2 = numbered(&app, &cookie, "rt", &rw1, 2).await;

    let rw_taken = create(
        &app,
        &cookie,
        json!({ "kind": "rw", "parentId": kulon, "number": 1 }),
    );
    let rt_taken = create(
        &app,
        &cookie,
        json!({ "kind": "rt", "parentId": rw1, "number": 1 }),
    );
    let renumbered_onto_taken = set_active(&app, &cookie, &rt2, 1, true);

    assert_eq!(rw_taken.await.status, StatusCode::CONFLICT);
    assert_eq!(rt_taken.await.status, StatusCode::CONFLICT);
    assert_eq!(renumbered_onto_taken.await, StatusCode::CONFLICT);
    numbered(&app, &cookie, "rt", &rw2, 1).await;
    // An inactive unit keeps its number, to be reactivated rather than recreated.
    set_active(&app, &cookie, &rt2, 2, false).await;
    let inactive_taken = create(
        &app,
        &cookie,
        json!({ "kind": "rt", "parentId": rw1, "number": 2 }),
    );
    assert_eq!(inactive_taken.await.status, StatusCode::CONFLICT);
}

#[sqlx::test]
async fn rt_numbers_are_unique_within_their_hamlet_without_rws(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    assert_eq!(
        settings(&app, &cookie, "dusun", false).await,
        StatusCode::NO_CONTENT
    );
    let krajan = hamlet(&app, &cookie, "Krajan").await;
    let kulon = hamlet(&app, &cookie, "Kulon").await;
    numbered(&app, &cookie, "rt", &krajan, 1).await;

    let taken = create(
        &app,
        &cookie,
        json!({ "kind": "rt", "parentId": krajan, "number": 1 }),
    );

    assert_eq!(taken.await.status, StatusCode::CONFLICT);
    numbered(&app, &cookie, "rt", &kulon, 1).await;
}

#[sqlx::test]
async fn the_rw_level_is_toggled_only_while_no_rt_exists(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;
    let rw = numbered(&app, &cookie, "rw", &krajan, 1).await;
    // Off only once every RW is inactive, which stay so while the level is off.
    assert_eq!(
        settings(&app, &cookie, "dusun", false).await,
        StatusCode::CONFLICT
    );
    set_active(&app, &cookie, &rw, 1, false).await;
    assert_eq!(
        settings(&app, &cookie, "dusun", false).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        set_active(&app, &cookie, &rw, 1, true).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        settings(&app, &cookie, "dusun", true).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        set_active(&app, &cookie, &rw, 1, true).await,
        StatusCode::NO_CONTENT
    );
    let rt = numbered(&app, &cookie, "rt", &rw, 1).await;

    assert_eq!(
        settings(&app, &cookie, "dusun", false).await,
        StatusCode::CONFLICT
    );
    set_active(&app, &cookie, &rt, 1, false).await;
    assert_eq!(
        settings(&app, &cookie, "dusun", false).await,
        StatusCode::CONFLICT
    );
    // The label alone may still change.
    assert_eq!(
        settings(&app, &cookie, "jorong", true).await,
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test]
async fn the_tree_follows_the_rw_level(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;
    let rw = numbered(&app, &cookie, "rw", &krajan, 1).await;

    let rt_under_hamlet = create(
        &app,
        &cookie,
        json!({ "kind": "rt", "parentId": krajan, "number": 1 }),
    );
    assert_eq!(
        rt_under_hamlet.await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let rw_under_rw = create(
        &app,
        &cookie,
        json!({ "kind": "rw", "parentId": rw, "number": 2 }),
    );
    assert_eq!(rw_under_rw.await.status, StatusCode::UNPROCESSABLE_ENTITY);
    let orphan_rt = create(&app, &cookie, json!({ "kind": "rt", "number": 1 }));
    assert_eq!(orphan_rt.await.status, StatusCode::UNPROCESSABLE_ENTITY);
    let nested_hamlet = create(
        &app,
        &cookie,
        json!({ "kind": "hamlet", "parentId": krajan, "name": "X" }),
    );
    assert_eq!(nested_hamlet.await.status, StatusCode::UNPROCESSABLE_ENTITY);

    set_active(&app, &cookie, &rw, 1, false).await;
    assert_eq!(
        settings(&app, &cookie, "dusun", false).await,
        StatusCode::NO_CONTENT
    );
    let rw_without_level = create(
        &app,
        &cookie,
        json!({ "kind": "rw", "parentId": krajan, "number": 2 }),
    );
    assert_eq!(rw_without_level.await.status, StatusCode::CONFLICT);
    let rt_under_rw = create(
        &app,
        &cookie,
        json!({ "kind": "rt", "parentId": rw, "number": 1 }),
    );
    assert_eq!(rt_under_rw.await.status, StatusCode::UNPROCESSABLE_ENTITY);
    numbered(&app, &cookie, "rt", &krajan, 1).await;
}

#[sqlx::test]
async fn units_are_deactivated_only_when_empty_and_reactivated_under_an_active_parent(
    pool: PgPool,
) {
    let (app, cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;
    let rw = numbered(&app, &cookie, "rw", &krajan, 1).await;
    let rt = numbered(&app, &cookie, "rt", &rw, 1).await;
    let deactivate_hamlet = json!({ "name": "Krajan", "active": false });
    let reactivate_hamlet = json!({ "name": "Krajan", "active": true });

    assert_eq!(
        set_active(&app, &cookie, &rw, 1, false).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        update(&app, &cookie, &krajan, deactivate_hamlet.clone()).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        set_active(&app, &cookie, &rt, 1, false).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        update(&app, &cookie, &krajan, deactivate_hamlet.clone()).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        set_active(&app, &cookie, &rw, 1, false).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        update(&app, &cookie, &krajan, deactivate_hamlet).await,
        StatusCode::NO_CONTENT
    );

    assert_eq!(
        set_active(&app, &cookie, &rt, 1, true).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        set_active(&app, &cookie, &rw, 1, true).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        update(&app, &cookie, &krajan, reactivate_hamlet).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        set_active(&app, &cookie, &rw, 1, true).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        set_active(&app, &cookie, &rt, 1, true).await,
        StatusCode::NO_CONTENT
    );
    let units = tree(&app, &cookie).await["units"].clone();
    assert!(
        units
            .as_array()
            .unwrap()
            .iter()
            .all(|unit| unit["active"] == true)
    );
}

#[sqlx::test]
async fn a_hamlet_without_rws_is_deactivated_once_its_rts_are(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    settings(&app, &cookie, "dusun", false).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;
    let rt = numbered(&app, &cookie, "rt", &krajan, 1).await;
    let deactivate = json!({ "name": "Krajan", "active": false });

    assert_eq!(
        update(&app, &cookie, &krajan, deactivate.clone()).await,
        StatusCode::CONFLICT
    );
    set_active(&app, &cookie, &rt, 1, false).await;
    assert_eq!(
        update(&app, &cookie, &krajan, deactivate).await,
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test]
async fn inactive_units_cannot_be_picked_for_new_units(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;
    let kulon = hamlet(&app, &cookie, "Kulon").await;
    let rw = numbered(&app, &cookie, "rw", &kulon, 1).await;
    update(
        &app,
        &cookie,
        &krajan,
        json!({ "name": "Krajan", "active": false }),
    )
    .await;
    set_active(&app, &cookie, &rw, 1, false).await;

    let rw_under_inactive = create(
        &app,
        &cookie,
        json!({ "kind": "rw", "parentId": krajan, "number": 2 }),
    );
    let rt_under_inactive = create(
        &app,
        &cookie,
        json!({ "kind": "rt", "parentId": rw, "number": 1 }),
    );

    assert_eq!(
        rw_under_inactive.await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        rt_under_inactive.await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test]
async fn hamlets_keep_their_manual_order(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let a = hamlet(&app, &cookie, "A").await;
    let b = hamlet(&app, &cookie, "B").await;
    let c = hamlet(&app, &cookie, "C").await;
    let order = |ids: Value| {
        let body = json!({ "ids": ids });
        let (app, cookie) = (&app, &cookie);
        async move {
            app.call("PUT", "/regions/hamlet-order", Some(&body), Some(cookie))
                .await
                .status
        }
    };

    assert_eq!(order(json!([c, a, b])).await, StatusCode::NO_CONTENT);
    assert_eq!(order(json!([c, a])).await, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        order(json!([c, a, a])).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let unknown = json!([c, a, b, uuid::Uuid::nil()]);
    assert_eq!(order(unknown).await, StatusCode::UNPROCESSABLE_ENTITY);
    let d = hamlet(&app, &cookie, "D").await;
    let ids: Vec<_> = tree(&app, &cookie).await["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|unit| unit["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, [c, a, b, d]);
}

#[sqlx::test]
async fn malformed_units_and_settings_are_refused(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;

    for body in [
        json!({ "kind": "hamlet", "name": "  " }),
        json!({ "kind": "hamlet", "name": "x".repeat(101) }),
        json!({ "kind": "hamlet", "name": "Kulon", "number": 1 }),
        json!({ "kind": "hamlet", "name": "Kulon", "headName": "Pak Lurah" }),
        json!({ "kind": "rw", "parentId": krajan }),
        json!({ "kind": "rw", "parentId": krajan, "number": 0 }),
        json!({ "kind": "rw", "parentId": krajan, "number": 1000 }),
        json!({ "kind": "rw", "parentId": krajan, "number": 1, "name": "Satu" }),
        json!({ "kind": "rw", "parentId": krajan, "number": 1, "headPhone": "call me" }),
        json!({ "kind": "rw", "parentId": krajan, "number": 1, "headName": "x".repeat(101) }),
        json!({ "kind": "rw", "parentId": uuid::Uuid::nil(), "number": 1 }),
    ] {
        let status = create(&app, &cookie, body.clone()).await.status;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    assert_eq!(
        settings(&app, &cookie, " ", true).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        settings(&app, &cookie, &"x".repeat(31), true).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test]
async fn only_village_managers_change_the_tree_and_only_users_read_it(pool: PgPool) {
    let (app, manager_cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &manager_cookie, "Krajan").await;
    let cookie = app.user_with(&pool, "writer", &["content.write"]).await;

    let creating = create(&app, &cookie, json!({ "kind": "hamlet", "name": "Kulon" }));
    let updating = update(
        &app,
        &cookie,
        &krajan,
        json!({ "name": "X", "active": true }),
    );
    let order = json!({ "ids": [krajan] });
    let ordering = app.call("PUT", "/regions/hamlet-order", Some(&order), Some(&cookie));

    assert_eq!(creating.await.status, StatusCode::FORBIDDEN);
    assert_eq!(updating.await, StatusCode::FORBIDDEN);
    assert_eq!(ordering.await.status, StatusCode::FORBIDDEN);
    assert_eq!(
        settings(&app, &cookie, "banjar", true).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(app.get("/regions").await.0, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn unknown_units_are_not_found_and_none_is_ever_deleted(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;
    let nil = uuid::Uuid::nil().to_string();

    let unknown = update(&app, &cookie, &nil, json!({ "name": "X", "active": true }));
    let deleting = app
        .call(
            "DELETE",
            &format!("/regions/units/{krajan}"),
            None,
            Some(&cookie),
        )
        .await;

    assert_eq!(unknown.await, StatusCode::NOT_FOUND);
    assert_eq!(deleting.status, StatusCode::METHOD_NOT_ALLOWED);
    let deleted = sqlx::query("DELETE FROM region_units").execute(&pool).await;
    assert!(deleted.is_err());
}

#[sqlx::test]
async fn the_tree_is_not_found_before_the_village_code_is_set(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;

    let reply = app.call("GET", "/regions", None, Some(&cookie)).await;
    let creating = create(&app, &cookie, json!({ "kind": "hamlet", "name": "Krajan" })).await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(creating.status, StatusCode::NOT_FOUND);
}
