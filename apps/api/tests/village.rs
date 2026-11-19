mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;
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
