mod common;

use axum::http::StatusCode;
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A Village with a writer (`content.write`), a publisher (`content.publish`, `audit.read`) and
/// an uploaded logo.
struct Office {
    app: TestApp,
    writer: String,
    publisher: String,
    logo: Value,
}

async fn office(pool: &PgPool) -> Office {
    pranata::village::set_code(pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let writer = app.user_with(pool, "penulis", &["content.write"]).await;
    let publisher = app
        .user_with(pool, "redaksi", &["content.publish", "audit.read"])
        .await;
    let upload = app
        .upload("/partners/logo", common::png(64, 32), &writer)
        .await;
    assert_eq!(upload.status, StatusCode::CREATED);
    Office {
        app,
        writer,
        publisher,
        logo: upload.body["id"].clone(),
    }
}

impl Office {
    async fn call(&self, method: &str, path: &str, body: Option<&Value>, cookie: &str) -> Reply {
        self.app.call(method, path, body, Some(cookie)).await
    }

    async fn draft(&self, name: &str) -> String {
        let fields = json!({ "name": name, "logo": self.logo });
        let reply = self
            .call("POST", "/partners", Some(&fields), &self.writer)
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{fields}");
        reply.body["id"].as_str().unwrap().to_owned()
    }

    async fn publish(&self, id: &str) -> StatusCode {
        self.call(
            "POST",
            &format!("/partners/{id}/publish"),
            None,
            &self.publisher,
        )
        .await
        .status
    }

    async fn public_names(&self) -> Vec<String> {
        let (status, partners) = self.app.get("/partners/published").await;
        assert_eq!(status, StatusCode::OK);
        partners
            .as_array()
            .unwrap()
            .iter()
            .map(|partner| partner["name"].as_str().unwrap().to_owned())
            .collect()
    }
}

#[sqlx::test]
async fn a_writer_drafts_a_publisher_publishes_and_the_public_reads_it(pool: PgPool) {
    let office = office(&pool).await;
    let fields = json!({
        "name": " Bank Desa ", "logo": office.logo, "url": " https://bank.example/desa ",
    });
    let created = office
        .call("POST", "/partners", Some(&fields), &office.writer)
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let id = created.body["id"].as_str().unwrap().to_owned();
    assert_eq!(office.app.get("/partners/published").await.1, json!([]));
    let list = office.call("GET", "/partners", None, &office.writer).await;
    assert_eq!(
        list.body,
        json!([{
            "id": id, "name": "Bank Desa", "logo": office.logo,
            "url": "https://bank.example/desa", "published": false,
        }])
    );

    let path = format!("/partners/{id}");
    let refused = office
        .call("POST", &format!("{path}/publish"), None, &office.writer)
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(office.publish(&id).await, StatusCode::NO_CONTENT);
    assert_eq!(office.publish(&id).await, StatusCode::CONFLICT);
    assert_eq!(
        office.app.get("/partners/published").await.1,
        json!([{ "name": "Bank Desa", "logo": office.logo, "url": "https://bank.example/desa" }])
    );

    // A live Partner changes only on a publisher's save; a new logo releases the old one.
    let logo = office
        .app
        .upload("/partners/logo", common::png(32, 32), &office.publisher)
        .await
        .body["id"]
        .clone();
    let fields = json!({ "name": "Bank Desa", "logo": logo });
    let by_writer = office
        .call("PUT", &path, Some(&fields), &office.writer)
        .await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let by_publisher = office
        .call("PUT", &path, Some(&fields), &office.publisher)
        .await;
    assert_eq!(by_publisher.status, StatusCode::NO_CONTENT);
    assert_eq!(
        office.app.get("/partners/published").await.1,
        json!([{ "name": "Bank Desa", "logo": logo, "url": null }])
    );
    let released: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT sweep_after FROM files WHERE id = $1::text::uuid")
            .bind(office.logo.as_str())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(released.is_some(), "the replaced logo is swept later");

    // Only a Draft is deleted, and only by a publisher.
    let deleted = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(deleted.status, StatusCode::CONFLICT);
    let unpublished = office
        .call(
            "POST",
            &format!("{path}/unpublish"),
            None,
            &office.publisher,
        )
        .await;
    assert_eq!(unpublished.status, StatusCode::NO_CONTENT);
    assert_eq!(office.app.get("/partners/published").await.1, json!([]));
    let by_writer = office.call("DELETE", &path, None, &office.writer).await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let deleted = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);

    let entries = office
        .call("GET", "/audit-entries", None, &office.publisher)
        .await
        .body["entries"]
        .clone();
    let actions: Vec<_> = entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["action"].as_str().unwrap())
        .filter(|action| action.starts_with("partner."))
        .collect();
    assert_eq!(
        actions,
        [
            "partner.delete",
            "partner.unpublish",
            "partner.update",
            "partner.publish",
            "partner.create",
        ]
    );
}

#[sqlx::test]
async fn partners_need_a_name_a_known_logo_and_a_web_url(pool: PgPool) {
    let office = office(&pool).await;
    for fields in [
        json!({ "name": " ", "logo": office.logo }),
        json!({ "name": "Bank Desa", "logo": "0190c0de-0000-7000-8000-000000000000" }),
        json!({ "name": "Bank Desa", "logo": office.logo, "url": "javascript:alert(1)" }),
        json!({ "name": "Bank Desa", "logo": office.logo, "url": "bank desa" }),
    ] {
        let reply = office
            .call("POST", "/partners", Some(&fields), &office.writer)
            .await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{fields}");
    }

    // The logo is a PNG of at most 2 MiB; a photo is refused.
    let jpeg = office
        .app
        .upload(
            "/partners/logo",
            b"\xFF\xD8\xFF\xE0".to_vec(),
            &office.writer,
        )
        .await;
    assert_eq!(jpeg.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let huge = office
        .app
        .upload("/partners/logo", vec![0; (2 << 20) + 1], &office.writer)
        .await;
    assert_eq!(huge.status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[sqlx::test]
async fn partners_keep_the_order_publishers_drag_them_into(pool: PgPool) {
    let office = office(&pool).await;
    let bank = office.draft("Bank Desa").await;
    let school = office.draft("SD Negeri 1").await;
    let clinic = office.draft("Klinik Sehat").await;
    for id in [&bank, &school, &clinic] {
        assert_eq!(office.publish(id).await, StatusCode::NO_CONTENT);
    }
    assert_eq!(
        office.public_names().await,
        ["Bank Desa", "SD Negeri 1", "Klinik Sehat"]
    );

    let order = json!({ "ids": [clinic, bank, school] });
    let by_writer = office
        .call("PUT", "/partners/order", Some(&order), &office.writer)
        .await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let partial = json!({ "ids": [clinic, bank] });
    let refused = office
        .call("PUT", "/partners/order", Some(&partial), &office.publisher)
        .await;
    assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY);
    let reordered = office
        .call("PUT", "/partners/order", Some(&order), &office.publisher)
        .await;
    assert_eq!(reordered.status, StatusCode::NO_CONTENT);
    assert_eq!(
        office.public_names().await,
        ["Klinik Sehat", "Bank Desa", "SD Negeri 1"]
    );

    // A new Partner goes last.
    let market = office.draft("Pasar Desa").await;
    let list = office.call("GET", "/partners", None, &office.writer).await;
    let ids: Vec<_> = list
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|partner| partner["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, [clinic, bank, school, market]);
}
