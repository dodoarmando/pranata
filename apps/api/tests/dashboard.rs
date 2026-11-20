mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn the_dashboard_counts_active_users_for_any_user(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    app.administrator(&pool, "leaver").await;
    let cookie = app
        .user_with(&pool, "kontributor", &["content.write"])
        .await;
    sqlx::query("UPDATE users SET active = false WHERE username = 'leaver'")
        .execute(&pool)
        .await
        .unwrap();

    let reply = app.call("GET", "/dashboard", None, Some(&cookie)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["activeUsers"], 2);
}

#[sqlx::test]
async fn the_dashboard_needs_a_session(pool: PgPool) {
    let app = TestApp::new(pool);

    let (status, _) = app.get("/dashboard").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

async fn missing(app: &TestApp, cookie: &str) -> serde_json::Value {
    let reply = app.call("GET", "/dashboard", None, Some(cookie)).await;
    assert_eq!(reply.status, StatusCode::OK);
    reply.body["missingVillageData"].clone()
}

#[sqlx::test]
async fn the_dashboard_lists_missing_village_data(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;
    for name in ["Krajan", "Kidul"] {
        let body = json!({ "kind": "hamlet", "name": name });
        let reply = app
            .call("POST", "/regions/units", Some(&body), Some(&cookie))
            .await;
        assert_eq!(reply.status, StatusCode::CREATED);
    }
    let positions = app.call("GET", "/positions", None, Some(&cookie)).await;
    let krajan = positions.body.as_array().unwrap()[8]["id"].clone();
    let tenure = json!({
        "positionId": krajan, "name": "Pak Sastro", "kind": "definitive",
        "startDate": "2020-01-01", "decreeNumber": "141/01/2020", "decreeDate": "2020-01-01",
    });
    let reply = app
        .call("POST", "/tenures", Some(&tenure), Some(&cookie))
        .await;
    assert_eq!(reply.status, StatusCode::CREATED);

    assert_eq!(
        missing(&app, &cookie).await,
        json!({ "logo": true, "visionMission": true, "unfilledPositions": ["Kepala Dusun Kidul"] })
    );

    let vision = json!({
        "vision": "Desa maju", "missions": ["Melayani"], "startYear": 2025, "endYear": 2031,
    });
    let reply = app
        .call(
            "PUT",
            "/village/vision-mission",
            Some(&vision),
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(missing(&app, &cookie).await["visionMission"], false);
}
