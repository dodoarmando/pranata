mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::{Value, json};
use sqlx::PgPool;

#[sqlx::test]
async fn village_is_not_found_before_its_code_is_set(pool: PgPool) {
    let app = TestApp::new(pool);

    let (status, _) = app.get("/village").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn a_desa_code_sets_the_name_region_and_time_zone_from_the_national_list(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool);

    let (status, body) = app.get("/village").await;

    assert_eq!(status, StatusCode::OK);
    let name = |code| pranata::village::region_name(code).unwrap();
    assert_eq!(
        body,
        json!({
            "code": "1101012001",
            "name": name("11.01.01.2001"),
            "district": name("11.01.01"),
            "regency": name("11.01"),
            "province": name("11"),
            "timeZone": "WIB",
            "address": null,
            "postalCode": null,
            "phone": null,
            "email": null,
            "latitude": null,
            "longitude": null,
            "logo": null,
            "pseNumber": null,
        })
    );
}

#[sqlx::test]
async fn the_time_zone_follows_the_province(pool: PgPool) {
    let app = TestApp::new(pool.clone());

    for (code, time_zone) in [
        ("1101012001", "WIB"),
        ("5101012001", "WITA"),
        ("9103012004", "WIT"),
    ] {
        pranata::village::set_code(&pool, code).await.unwrap();
        let (_, body) = app.get("/village").await;
        assert_eq!(body["timeZone"], time_zone, "{code}");
    }
}

#[sqlx::test]
async fn a_desa_adat_code_is_accepted(pool: PgPool) {
    pranata::village::set_code(&pool, "9103013007")
        .await
        .unwrap();
    let app = TestApp::new(pool);

    let (status, body) = app.get("/village").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["code"], "9103013007");
}

#[sqlx::test]
async fn kelurahan_malformed_and_unknown_codes_are_refused(pool: PgPool) {
    let kelurahan = pranata::village::set_code(&pool, "1201011001").await;
    assert!(kelurahan.unwrap_err().to_string().contains("kelurahan"));
    for code in [
        "11.01.01.2001", // dotted, not 10 digits
        "110101200",     // 9 digits
        "110101200a",
        "1101015001", // neither desa nor kelurahan
        "1101012999", // not in the national list
    ] {
        assert!(
            pranata::village::set_code(&pool, code).await.is_err(),
            "{code} was accepted"
        );
    }
    let app = TestApp::new(pool);

    let (status, _) = app.get("/village").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn setting_the_code_again_replaces_the_village(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    pranata::village::set_code(&pool, "5101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool);

    let (_, body) = app.get("/village").await;

    assert_eq!(body["code"], "5101012001");
}

#[sqlx::test]
async fn no_endpoint_edits_the_village(pool: PgPool) {
    let app = TestApp::new(pool);

    for method in ["POST", "PUT", "PATCH", "DELETE"] {
        let status = app.request(method, "/village").await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{method}");
    }
}

fn profile() -> Value {
    json!({
        "address": "Jl. Balai Desa No. 1",
        "postalCode": "23111",
        "phone": "(0651) 123-456",
        "email": "kantor@desa.example",
        "latitude": -5.55,
        "longitude": 95.32,
        "logo": null,
        "pseNumber": "000123.01/DJAI.PSE/11/2026",
    })
}

async fn put_profile(app: &TestApp, body: &Value, cookie: &str) -> StatusCode {
    app.call("PUT", "/village/profile", Some(body), Some(cookie))
        .await
        .status
}

#[sqlx::test]
async fn a_village_manager_edits_the_profile_and_the_public_village_shows_it(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app
        .user_with(&pool, "operator", &["village.manage", "audit.read"])
        .await;

    let status = put_profile(&app, &profile(), &cookie).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, village) = app.get("/village").await;
    for (field, value) in profile().as_object().unwrap() {
        assert_eq!(&village[field], value, "{field}");
    }
    assert_eq!(village["code"], "1101012001");
    let audit = app
        .call("GET", "/audit-entries", None, Some(&cookie))
        .await
        .body;
    let entry = &audit["entries"][0];
    assert_eq!(entry["action"], "village.update");
    assert_eq!(entry["username"], "operator");
    assert_eq!(entry["objectType"], "village");
    assert_eq!(entry["objectId"], "1101012001");
    assert!(entry["changes"].as_array().unwrap().contains(
        &json!({ "field": "postalCode", "from": null, "to": "23111", "sensitive": false })
    ));
}

#[sqlx::test]
async fn only_changed_fields_are_audited_and_blank_text_is_cleared(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app
        .user_with(&pool, "operator", &["village.manage", "audit.read"])
        .await;
    put_profile(&app, &profile(), &cookie).await;
    let mut edited = profile();
    edited["phone"] = json!("  ");
    edited["address"] = json!("  Jl. Balai Desa No. 1 ");

    let status = put_profile(&app, &edited, &cookie).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, village) = app.get("/village").await;
    assert_eq!(village["phone"], Value::Null);
    assert_eq!(village["address"], "Jl. Balai Desa No. 1");
    let audit = app
        .call("GET", "/audit-entries", None, Some(&cookie))
        .await
        .body;
    assert_eq!(
        audit["entries"][0]["changes"],
        json!([{ "field": "phone", "from": "(0651) 123-456", "to": null, "sensitive": false }])
    );
}

#[sqlx::test]
async fn an_uploaded_logo_is_saved_into_the_profile(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect width="1" height="1"/></svg>"#;
    let logo = app.upload("/village/logo", svg.into(), &cookie).await.body["id"].clone();
    let mut body = profile();
    body["logo"] = logo.clone();

    let status = put_profile(&app, &body, &cookie).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/village").await.1["logo"], logo);
}

#[sqlx::test]
async fn malformed_fields_and_unknown_logos_are_refused(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;

    for (field, value) in [
        ("postalCode", json!("2311")),
        ("postalCode", json!("2311a")),
        ("phone", json!("call me")),
        ("phone", json!("12")),
        ("email", json!("kantor")),
        ("email", json!("kantor@desa")),
        ("latitude", json!(-91)),
        ("longitude", json!(181)),
        ("longitude", Value::Null),
        ("logo", json!(uuid::Uuid::nil())),
        ("pseNumber", json!("x".repeat(101))),
    ] {
        let mut body = profile();
        body[field] = value.clone();
        let status = put_profile(&app, &body, &cookie).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{field}: {value}");
    }
}

#[sqlx::test]
async fn only_village_managers_edit_the_profile(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "writer", &["content.write"]).await;

    let refused = put_profile(&app, &profile(), &cookie).await;
    let anonymous = app
        .call("PUT", "/village/profile", Some(&profile()), None)
        .await;

    assert_eq!(refused, StatusCode::FORBIDDEN);
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn the_profile_cannot_be_edited_before_the_village_code_is_set(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;

    let status = put_profile(&app, &profile(), &cookie).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn setting_the_code_again_keeps_the_profile(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;
    put_profile(&app, &profile(), &cookie).await;

    pranata::village::set_code(&pool, "5101012001")
        .await
        .unwrap();

    assert_eq!(app.get("/village").await.1["postalCode"], "23111");
}

fn vision_mission() -> Value {
    json!({
        "vision": "Desa yang terbuka, melayani, dan mandiri.",
        "missions": ["Meningkatkan pelayanan.", "Membangun irigasi desa."],
        "startYear": 2025,
        "endYear": 2031,
    })
}

async fn put_vision_mission(app: &TestApp, body: &Value, cookie: &str) -> StatusCode {
    app.call("PUT", "/village/vision-mission", Some(body), Some(cookie))
        .await
        .status
}

#[sqlx::test]
async fn the_vision_and_mission_is_not_found_until_it_is_saved(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool);

    let (status, _) = app.get("/village/vision-mission").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn a_village_manager_saves_the_vision_and_mission_and_the_public_sees_it(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app
        .user_with(&pool, "operator", &["village.manage", "audit.read"])
        .await;

    let status = put_vision_mission(&app, &vision_mission(), &cookie).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/village/vision-mission").await,
        (StatusCode::OK, vision_mission())
    );
    let audit = app
        .call("GET", "/audit-entries", None, Some(&cookie))
        .await
        .body;
    let entry = &audit["entries"][0];
    assert_eq!(entry["action"], "village.update_vision_mission");
    assert_eq!(entry["username"], "operator");
    assert_eq!(entry["objectType"], "village");
    assert_eq!(entry["objectId"], "1101012001");
    assert!(entry["changes"].as_array().unwrap().contains(&json!({
        "field": "missions",
        "from": null,
        "to": ["Meningkatkan pelayanan.", "Membangun irigasi desa."],
        "sensitive": false,
    })));
}

#[sqlx::test]
async fn a_new_version_replaces_the_old_which_lives_on_in_the_audit_log(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app
        .user_with(&pool, "operator", &["village.manage", "audit.read"])
        .await;
    put_vision_mission(&app, &vision_mission(), &cookie).await;
    let mut next = vision_mission();
    next["missions"] = json!(["  Membangun irigasi desa. ", "", "Meningkatkan pelayanan."]);

    let status = put_vision_mission(&app, &next, &cookie).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, current) = app.get("/village/vision-mission").await;
    assert_eq!(
        current["missions"],
        json!(["Membangun irigasi desa.", "Meningkatkan pelayanan."])
    );
    let audit = app
        .call("GET", "/audit-entries", None, Some(&cookie))
        .await
        .body;
    assert_eq!(
        audit["entries"][0]["changes"],
        json!([{
            "field": "missions",
            "from": ["Meningkatkan pelayanan.", "Membangun irigasi desa."],
            "to": ["Membangun irigasi desa.", "Meningkatkan pelayanan."],
            "sensitive": false,
        }])
    );
}

#[sqlx::test]
async fn a_malformed_vision_and_mission_is_refused(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;

    for (field, value) in [
        ("vision", json!("  ")),
        ("vision", json!("x".repeat(1001))),
        ("missions", json!([])),
        ("missions", json!([" "])),
        ("missions", json!(["x".repeat(1001)])),
        ("missions", json!(vec!["Misi"; 21])),
        ("startYear", json!(1999)),
        ("startYear", json!(2031)),
        ("endYear", json!(2101)),
    ] {
        let mut body = vision_mission();
        body[field] = value.clone();
        let status = put_vision_mission(&app, &body, &cookie).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{field}: {value}");
    }
}

#[sqlx::test]
async fn only_village_managers_save_the_vision_and_mission(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "writer", &["content.write"]).await;

    let refused = put_vision_mission(&app, &vision_mission(), &cookie).await;
    let anonymous = app
        .call(
            "PUT",
            "/village/vision-mission",
            Some(&vision_mission()),
            None,
        )
        .await;

    assert_eq!(refused, StatusCode::FORBIDDEN);
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn the_vision_and_mission_cannot_be_saved_before_the_village_code_is_set(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;

    let status = put_vision_mission(&app, &vision_mission(), &cookie).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
