mod common;

use axum::http::StatusCode;
use common::TestApp;
use sqlx::PgPool;

#[sqlx::test]
async fn the_seed_fills_the_public_site_once(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let code = pranata::seed::run(&pool, common::ORIGIN, app.files.path())
        .await
        .unwrap();

    let (status, news) = app.get("/news-posts/published").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(news["total"], 10);
    let (_, announcements) = app.get("/announcements/in-effect").await;
    assert_eq!(announcements.as_array().unwrap().len(), 3);
    let (_, events) = app.get("/events/upcoming").await;
    assert_eq!(events.as_array().unwrap().len(), 5);

    let (status, hero) = app.get("/home/hero").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hero["quickLinks"].as_array().unwrap().len(), 4);
    let photo = hero["photo"].as_str().unwrap();
    let (status, _, _) = app.get_raw(&format!("/media/{photo}/2400")).await;
    assert_eq!(status, StatusCode::OK);

    let (status, complaint) = app.get(&format!("/complaints/status/{code}")).await;
    assert_eq!(status, StatusCode::OK, "{complaint}");

    let again = pranata::seed::run(&pool, common::ORIGIN, app.files.path()).await;
    assert!(again.is_err(), "a second seed must refuse");
}
