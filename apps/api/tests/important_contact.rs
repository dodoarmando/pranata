mod common;

use axum::http::StatusCode;
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A Village with a writer (`content.write`) and a publisher (`content.publish`, `audit.read`).
struct Office {
    app: TestApp,
    writer: String,
    publisher: String,
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
    Office {
        app,
        writer,
        publisher,
    }
}

impl Office {
    async fn call(&self, method: &str, path: &str, body: Option<&Value>, cookie: &str) -> Reply {
        self.app.call(method, path, body, Some(cookie)).await
    }

    async fn category(&self, name: &str) -> String {
        let reply = self
            .call(
                "POST",
                "/contact-categories",
                Some(&json!({ "name": name })),
                &self.publisher,
            )
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{name}");
        reply.body["id"].as_str().unwrap().to_owned()
    }

    async fn draft(&self, fields: &Value) -> String {
        let reply = self
            .call("POST", "/important-contacts", Some(fields), &self.writer)
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{fields}");
        reply.body["id"].as_str().unwrap().to_owned()
    }

    async fn publish(&self, id: &str) -> StatusCode {
        self.call(
            "POST",
            &format!("/important-contacts/{id}/publish"),
            None,
            &self.publisher,
        )
        .await
        .status
    }

    async fn published(&self, name: &str, category: &str) -> String {
        let id = self
            .draft(&json!({ "name": name, "category": category, "phone": "112" }))
            .await;
        assert_eq!(self.publish(&id).await, StatusCode::NO_CONTENT);
        id
    }

    /// The public listing as `category: [names]` pairs.
    async fn public_names(&self) -> Vec<(String, Vec<String>)> {
        let (status, groups) = self.app.get("/important-contacts/published").await;
        assert_eq!(status, StatusCode::OK);
        groups
            .as_array()
            .unwrap()
            .iter()
            .map(|group| {
                let names = group["contacts"].as_array().unwrap();
                (
                    group["category"].as_str().unwrap().to_owned(),
                    names
                        .iter()
                        .map(|contact| contact["name"].as_str().unwrap().to_owned())
                        .collect(),
                )
            })
            .collect()
    }
}

#[sqlx::test]
async fn a_writer_drafts_a_publisher_publishes_and_the_public_reads_it(pool: PgPool) {
    let office = office(&pool).await;
    let health = office.category("Kesehatan").await;
    let id = office
        .draft(&json!({
            "name": " Puskesmas Kecamatan ", "category": health, "phone": "(0651) 123-456",
            "note": " Buka 24 jam ",
        }))
        .await;
    assert_eq!(
        office.app.get("/important-contacts/published").await.1,
        json!([])
    );
    let (_, list) = office.app.get("/important-contacts").await;
    assert_eq!(list, Value::Null, "the admin list needs a session");
    let list = office
        .call("GET", "/important-contacts", None, &office.writer)
        .await;
    assert_eq!(
        list.body,
        json!([{
            "id": id, "name": "Puskesmas Kecamatan", "category": health,
            "phone": "(0651) 123-456", "note": "Buka 24 jam", "order": 1, "published": false,
        }])
    );

    // The writer may not publish.
    let path = format!("/important-contacts/{id}");
    let refused = office
        .call("POST", &format!("{path}/publish"), None, &office.writer)
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(office.publish(&id).await, StatusCode::NO_CONTENT);
    assert_eq!(office.publish(&id).await, StatusCode::CONFLICT);
    let (_, groups) = office.app.get("/important-contacts/published").await;
    assert_eq!(
        groups,
        json!([{ "category": "Kesehatan", "contacts": [
            { "name": "Puskesmas Kecamatan", "phone": "(0651) 123-456", "note": "Buka 24 jam" },
        ] }])
    );

    // A live contact changes only on a publisher's save.
    let fields = json!({ "name": "Puskesmas", "category": health, "phone": "0651 123456" });
    let by_writer = office
        .call("PUT", &path, Some(&fields), &office.writer)
        .await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let by_publisher = office
        .call("PUT", &path, Some(&fields), &office.publisher)
        .await;
    assert_eq!(by_publisher.status, StatusCode::NO_CONTENT);
    assert_eq!(
        office.public_names().await,
        [("Kesehatan".to_owned(), vec!["Puskesmas".to_owned()])]
    );

    // Only a Draft is deleted, and only by a publisher.
    let deleted = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(deleted.status, StatusCode::CONFLICT);
    let by_writer = office
        .call("POST", &format!("{path}/unpublish"), None, &office.writer)
        .await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let unpublished = office
        .call(
            "POST",
            &format!("{path}/unpublish"),
            None,
            &office.publisher,
        )
        .await;
    assert_eq!(unpublished.status, StatusCode::NO_CONTENT);
    assert_eq!(
        office.app.get("/important-contacts/published").await.1,
        json!([])
    );
    let by_writer = office.call("DELETE", &path, None, &office.writer).await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let deleted = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let gone = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);

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
        .filter(|action| action.starts_with("important_contact."))
        .collect();
    assert_eq!(
        actions,
        [
            "important_contact.delete",
            "important_contact.unpublish",
            "important_contact.update",
            "important_contact.publish",
            "important_contact.create",
        ]
    );
}

#[sqlx::test]
async fn contacts_need_a_name_a_known_category_and_a_phone(pool: PgPool) {
    let office = office(&pool).await;
    let health = office.category("Kesehatan").await;
    for fields in [
        json!({ "name": " ", "category": health, "phone": "112" }),
        json!({ "name": "Polsek", "category": health, "phone": "call us" }),
        json!({ "name": "Polsek", "category": health, "phone": "112", "note": "x".repeat(151) }),
        json!({ "name": "Polsek", "category": "0190c0de-0000-7000-8000-000000000000", "phone": "112" }),
    ] {
        let reply = office
            .call("POST", "/important-contacts", Some(&fields), &office.writer)
            .await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{fields}");
    }
}

#[sqlx::test]
async fn categories_and_contacts_keep_the_order_staff_give(pool: PgPool) {
    let office = office(&pool).await;
    let health = office.category("Kesehatan").await;
    let safety = office.category("Keamanan").await;
    office.published("Puskesmas", &health).await;
    let ambulance = office.published("Ambulans Desa", &health).await;
    office.published("Polsek", &safety).await;
    assert_eq!(
        office.public_names().await,
        [
            (
                "Kesehatan".to_owned(),
                vec!["Puskesmas".to_owned(), "Ambulans Desa".to_owned()]
            ),
            ("Keamanan".to_owned(), vec!["Polsek".to_owned()]),
        ]
    );

    // An explicit order puts a contact first; a move without one puts it last in its new category.
    let path = format!("/important-contacts/{ambulance}");
    let first = json!({ "name": "Ambulans Desa", "category": health, "phone": "112", "order": 0 });
    let saved = office
        .call("PUT", &path, Some(&first), &office.publisher)
        .await;
    assert_eq!(saved.status, StatusCode::NO_CONTENT);
    assert_eq!(
        office.public_names().await[0].1,
        ["Ambulans Desa", "Puskesmas"]
    );
    let moved = json!({ "name": "Ambulans Desa", "category": safety, "phone": "112" });
    let saved = office
        .call("PUT", &path, Some(&moved), &office.publisher)
        .await;
    assert_eq!(saved.status, StatusCode::NO_CONTENT);
    assert_eq!(
        office.public_names().await[1].1,
        ["Polsek", "Ambulans Desa"]
    );

    // Reordering the categories needs every one exactly once, and a publisher.
    let order = json!({ "ids": [safety, health] });
    let by_writer = office
        .call(
            "PUT",
            "/contact-categories/order",
            Some(&order),
            &office.writer,
        )
        .await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let partial = json!({ "ids": [safety] });
    let refused = office
        .call(
            "PUT",
            "/contact-categories/order",
            Some(&partial),
            &office.publisher,
        )
        .await;
    assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY);
    let reordered = office
        .call(
            "PUT",
            "/contact-categories/order",
            Some(&order),
            &office.publisher,
        )
        .await;
    assert_eq!(reordered.status, StatusCode::NO_CONTENT);
    let (_, categories) = office.app.get("/contact-categories").await;
    assert_eq!(
        categories,
        json!([
            { "id": safety, "name": "Keamanan" },
            { "id": health, "name": "Kesehatan" },
        ])
    );
    let names: Vec<String> = office
        .public_names()
        .await
        .into_iter()
        .map(|(category, _)| category)
        .collect();
    assert_eq!(names, ["Keamanan", "Kesehatan"]);
}

#[sqlx::test]
async fn publishers_manage_contact_categories(pool: PgPool) {
    let office = office(&pool).await;
    let by_writer = office
        .call(
            "POST",
            "/contact-categories",
            Some(&json!({ "name": "Kesehatan" })),
            &office.writer,
        )
        .await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let health = office.category("Kesehatan").await;
    let taken = office
        .call(
            "POST",
            "/contact-categories",
            Some(&json!({ "name": "KESEHATAN" })),
            &office.publisher,
        )
        .await;
    assert_eq!(taken.status, StatusCode::CONFLICT);
    let path = format!("/contact-categories/{health}");
    let renamed = office
        .call(
            "PUT",
            &path,
            Some(&json!({ "name": "Layanan Kesehatan" })),
            &office.publisher,
        )
        .await;
    assert_eq!(renamed.status, StatusCode::NO_CONTENT);

    // A category a contact, even a Draft, is filed under is kept.
    let draft = office
        .draft(&json!({ "name": "Puskesmas", "category": health, "phone": "112" }))
        .await;
    let in_use = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(in_use.status, StatusCode::CONFLICT);
    let deleted = office
        .call(
            "DELETE",
            &format!("/important-contacts/{draft}"),
            None,
            &office.publisher,
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let deleted = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let (_, categories) = office.app.get("/contact-categories").await;
    assert_eq!(categories, json!([]));
}
