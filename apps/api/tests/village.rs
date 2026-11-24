mod common;

use pranata::region_codes::{TimeZone, VillageCode, VillageCodeError, lookup};

fn code(s: &str) -> VillageCode {
    VillageCode::parse(s).expect("valid code")
}

#[test]
fn parse_accepts_dotted_and_plain() {
    assert_eq!(code("14.05.03.2002"), code("1405032002"));
    assert_eq!(code("1405032002").as_str(), "14.05.03.2002");
}

#[test]
fn parse_rejects_nine_digits() {
    assert_eq!(
        VillageCode::parse("140503200"),
        Err(VillageCodeError::Format)
    );
    assert_eq!(
        VillageCode::parse("14.05.03.20x2"),
        Err(VillageCodeError::Format)
    );
}

#[test]
fn lookup_finds_a_riau_village() {
    let record = lookup(&code("14.05.03.2002")).expect("village exists");
    assert_eq!(record.village, "Sorek II");
    assert_eq!(record.district, "Pangkalan Kuras");
    assert_eq!(record.regency, "Kabupaten Pelalawan");
    assert_eq!(record.province, "Riau");
    assert_eq!(record.time_zone, TimeZone::Wib);
}

#[test]
fn lookup_maps_papua_to_wit() {
    let record = lookup(&code("91.03.01.1001")).expect("village exists");
    assert_eq!(record.province, "Papua");
    assert_eq!(record.time_zone, TimeZone::Wit);
    assert_eq!(record.time_zone.iana_name(), "Asia/Jayapura");
}

#[test]
fn lookup_returns_none_for_unknown_code() {
    assert!(lookup(&code("99.99.99.9999")).is_none());
}

#[test]
fn time_zone_follows_province_code() {
    assert_eq!(TimeZone::from_province_code("51"), TimeZone::Wita);
    assert_eq!(TimeZone::from_province_code("76"), TimeZone::Wita);
    assert_eq!(TimeZone::from_province_code("81"), TimeZone::Wit);
    assert_eq!(TimeZone::from_province_code("97"), TimeZone::Wit);
    assert_eq!(TimeZone::from_province_code("14"), TimeZone::Wib);
    assert_eq!(TimeZone::Wita.iana_name(), "Asia/Makassar");
    assert_eq!(TimeZone::Wib.iana_name(), "Asia/Jakarta");
}

mod api {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use pranata::village::{self, InitError};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;

    use super::code;
    use crate::common;

    async fn get_village(pool: PgPool) -> (StatusCode, Value) {
        let response = pranata::app(common::test_state(pool))
            .oneshot(
                Request::builder()
                    .uri("/api/public/village")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[sqlx::test]
    async fn public_village_404_before_init(pool: PgPool) {
        let (status, body) = get_village(pool).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "village_not_initialized");
    }

    #[sqlx::test]
    async fn init_then_public_village_returns_profile(pool: PgPool) {
        village::init(&pool, &code("14.05.03.2002")).await.unwrap();
        let (status, body) = get_village(pool).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({
                "code": "14.05.03.2002",
                "name": "Sorek II",
                "district": "Pangkalan Kuras",
                "regency": "Kabupaten Pelalawan",
                "province": "Riau",
                "time_zone": "Asia/Jakarta",
            })
        );
    }

    #[sqlx::test]
    async fn init_uses_the_province_time_zone(pool: PgPool) {
        let profile = village::init(&pool, &code("91.03.01.1001")).await.unwrap();
        assert_eq!(profile.time_zone, "Asia/Jayapura");
    }

    #[sqlx::test]
    async fn init_twice_fails(pool: PgPool) {
        village::init(&pool, &code("14.05.03.2002")).await.unwrap();
        let second = village::init(&pool, &code("14.05.03.2003")).await;
        assert!(matches!(second, Err(InitError::AlreadyInitialised)));
        let (_, body) = get_village(pool).await;
        assert_eq!(body["code"], "14.05.03.2002");
    }

    #[sqlx::test]
    async fn init_rejects_a_code_missing_from_the_dataset(pool: PgPool) {
        let result = village::init(&pool, &code("99.99.99.9999")).await;
        assert!(matches!(result, Err(InitError::UnknownCode)));
    }

    #[sqlx::test]
    async fn init_writes_an_audit_entry(pool: PgPool) {
        village::init(&pool, &code("14.05.03.2002")).await.unwrap();
        let row = sqlx::query!(
            "SELECT action, object_type, object_id, summary, actor FROM audit_entries"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.action, "create");
        assert_eq!(row.object_type, "village");
        assert_eq!(row.object_id, "14.05.03.2002");
        assert!(row.summary.contains("14.05.03.2002"));
        assert!(row.actor.is_none());
    }
}
