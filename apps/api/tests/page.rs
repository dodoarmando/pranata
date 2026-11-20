mod common;

use axum::http::StatusCode;
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A Village with a writer (`content.write`), a publisher (`content.publish`, `audit.read`) and
/// an uploaded cover.
struct Office {
    app: TestApp,
    writer: String,
    publisher: String,
    cover: Value,
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
    let cover = app
        .upload("/pages/cover", common::png(64, 48), &writer)
        .await
        .body["id"]
        .clone();
    Office {
        app,
        writer,
        publisher,
        cover,
    }
}

fn body() -> Value {
    json!({ "type": "doc", "content": [
        { "type": "paragraph", "content": [{ "type": "text", "text": "Desa berdiri pada 1910." }] },
    ] })
}

impl Office {
    /// Every field publishing requires, under `menu`.
    fn complete(title: &str, menu: &str) -> Value {
        json!({
            "title": title, "menu": menu, "summary": " Asal-usul desa dari masa ke masa. ",
            "body": body(),
        })
    }

    async fn call(&self, method: &str, path: &str, body: Option<&Value>, cookie: &str) -> Reply {
        self.app.call(method, path, body, Some(cookie)).await
    }

    /// A new Draft's id and slug.
    async fn draft(&self, fields: &Value) -> (String, String) {
        let reply = self
            .call("POST", "/pages", Some(fields), &self.writer)
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{fields}");
        let id = reply.body["id"].as_str().unwrap().to_owned();
        let page = self
            .call("GET", &format!("/pages/{id}"), None, &self.writer)
            .await;
        (id, page.body["slug"].as_str().unwrap().to_owned())
    }

    async fn publish(&self, id: &str) -> StatusCode {
        self.call(
            "POST",
            &format!("/pages/{id}/publish"),
            None,
            &self.publisher,
        )
        .await
        .status
    }

    async fn published(&self, fields: &Value) -> (String, String) {
        let (id, slug) = self.draft(fields).await;
        assert_eq!(self.publish(&id).await, StatusCode::NO_CONTENT);
        (id, slug)
    }
}

#[sqlx::test]
async fn a_writer_drafts_a_publisher_publishes_and_the_public_reads_it(pool: PgPool) {
    let office = office(&pool).await;
    let (id, slug) = office
        .draft(&json!({ "title": "Sejarah Desa", "menu": "profile" }))
        .await;
    assert_eq!(slug, "sejarah-desa");
    let public = format!("/pages/published/{slug}");
    assert_eq!(office.app.get(&public).await.0, StatusCode::NOT_FOUND);

    // The autosave fills in the rest; the writer cannot publish.
    let path = format!("/pages/{id}");
    let mut fields = Office::complete("Sejarah Desa", "profile");
    fields["cover"] = office.cover.clone();
    fields["coverAlt"] = json!("Balai desa lama");
    let saved = office
        .call("PUT", &path, Some(&fields), &office.writer)
        .await;
    assert_eq!(saved.status, StatusCode::NO_CONTENT);
    let refused = office
        .call("POST", &format!("{path}/publish"), None, &office.writer)
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(office.publish(&id).await, StatusCode::NO_CONTENT);

    let (status, page) = office.app.get(&public).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["title"], "Sejarah Desa");
    assert_eq!(page["menu"], "profile");
    assert_eq!(page["summary"], "Asal-usul desa dari masa ke masa.");
    assert_eq!(page["cover"], office.cover);
    assert_eq!(page["coverAlt"], "Balai desa lama");
    assert_eq!(page["body"], body());
    let updated = page["updatedAt"].clone();
    assert!(updated.is_string());
    let (_, menus) = office.app.get("/pages/published").await;
    assert_eq!(
        menus,
        json!([{ "menu": "profile", "slug": slug, "title": "Sejarah Desa" }])
    );

    // A live Page changes only on a publisher's explicit save, which updates its date.
    fields["title"] = json!("Sejarah Singkat Desa");
    let by_writer = office
        .call("PUT", &path, Some(&fields), &office.writer)
        .await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let by_publisher = office
        .call("PUT", &path, Some(&fields), &office.publisher)
        .await;
    assert_eq!(by_publisher.status, StatusCode::NO_CONTENT);
    let (_, page) = office.app.get(&public).await;
    assert_eq!(page["title"], "Sejarah Singkat Desa");
    assert_ne!(page["updatedAt"], updated);

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
        .filter(|action| action.starts_with("page."))
        .collect();
    assert_eq!(
        actions,
        ["page.update", "page.publish", "page.update", "page.create"]
    );
    assert_eq!(
        entries[0]["changes"],
        json!([{
            "field": "title", "from": "Sejarah Desa", "to": "Sejarah Singkat Desa",
            "sensitive": false,
        }])
    );
}

#[sqlx::test]
async fn the_public_menus_list_published_pages_in_order(pool: PgPool) {
    let office = office(&pool).await;
    // A new Page goes last in its menu unless given an order.
    for (title, menu) in [
        ("Potensi Desa", "profile"),
        ("Sejarah Desa", "profile"),
        ("Pelayanan KTP", "services"),
        ("BPD", "government"),
    ] {
        office.published(&Office::complete(title, menu)).await;
    }
    let mut first = Office::complete("Geografi", "profile");
    first["order"] = json!(-1);
    office.published(&first).await;
    office
        .draft(&Office::complete("Belum Terbit", "profile"))
        .await;

    let (status, menus) = office.app.get("/pages/published").await;

    assert_eq!(status, StatusCode::OK);
    let titles: Vec<_> = menus
        .as_array()
        .unwrap()
        .iter()
        .map(|page| {
            (
                page["menu"].as_str().unwrap(),
                page["title"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        titles,
        [
            ("profile", "Geografi"),
            ("profile", "Potensi Desa"),
            ("profile", "Sejarah Desa"),
            ("government", "BPD"),
            ("services", "Pelayanan KTP"),
        ]
    );

    // Reordering moves a Page within its menu.
    let list = office.call("GET", "/pages", None, &office.writer).await;
    let sejarah = list
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|page| page["title"] == "Sejarah Desa")
        .unwrap()
        .clone();
    assert_eq!(sejarah["menu"], "profile");
    let mut fields = Office::complete("Sejarah Desa", "profile");
    fields["order"] = json!(-2);
    let reply = office
        .call(
            "PUT",
            &format!("/pages/{}", sejarah["id"].as_str().unwrap()),
            Some(&fields),
            &office.publisher,
        )
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let (_, menus) = office.app.get("/pages/published").await;
    assert_eq!(menus[0]["title"], "Sejarah Desa");

    // A Draft moved to another menu without an order goes last there.
    let mut fields = Office::complete("Pindahan", "services");
    fields["order"] = json!(-5);
    let (id, _) = office.draft(&fields).await;
    fields["menu"] = json!("profile");
    fields["order"] = Value::Null;
    office
        .call(
            "PUT",
            &format!("/pages/{id}"),
            Some(&fields),
            &office.writer,
        )
        .await;
    office.publish(&id).await;
    let (_, menus) = office.app.get("/pages/published").await;
    assert_eq!(
        menus[3],
        json!({ "menu": "profile", "slug": "pindahan", "title": "Pindahan" })
    );
}

#[sqlx::test]
async fn publishing_needs_a_summary_and_a_cover_needs_alt_text(pool: PgPool) {
    let office = office(&pool).await;
    let (id, _) = office
        .draft(&json!({ "title": "Profil", "menu": "profile" }))
        .await;
    assert_eq!(office.publish(&id).await, StatusCode::UNPROCESSABLE_ENTITY);
    let mut fields = Office::complete("Profil", "profile");
    fields["cover"] = office.cover.clone();
    let (id, _) = office.draft(&fields).await;
    assert_eq!(office.publish(&id).await, StatusCode::UNPROCESSABLE_ENTITY);

    // Without a cover, a summary is enough.
    let (id, _) = office
        .published(&Office::complete("Profil", "profile"))
        .await;
    let mut fields = Office::complete("Profil", "profile");
    fields["summary"] = json!("");
    let reply = office
        .call(
            "PUT",
            &format!("/pages/{id}"),
            Some(&fields),
            &office.publisher,
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn a_page_needs_a_known_menu(pool: PgPool) {
    let office = office(&pool).await;
    for fields in [
        json!({ "title": "Tanpa Menu" }),
        json!({ "title": "Menu Asing", "menu": "berita" }),
    ] {
        let reply = office
            .call("POST", "/pages", Some(&fields), &office.writer)
            .await;

        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{fields}");
    }
}

#[sqlx::test]
async fn the_slug_and_menu_lock_at_first_publish(pool: PgPool) {
    let office = office(&pool).await;
    let (id, slug) = office
        .draft(&Office::complete("Layanan KTP", "services"))
        .await;
    let (_, second) = office
        .draft(&Office::complete("Layanan KTP", "government"))
        .await;
    assert_eq!(second, "layanan-ktp-2");

    // Both are editable while a Draft.
    let path = format!("/pages/{id}");
    let mut fields = Office::complete("Layanan KTP", "profile");
    fields["slug"] = json!("ktp");
    let moved = office
        .call("PUT", &path, Some(&fields), &office.writer)
        .await;
    assert_eq!(moved.status, StatusCode::NO_CONTENT);
    let page = office.call("GET", &path, None, &office.writer).await;
    assert_eq!(page.body["slug"], "ktp");
    assert_eq!(page.body["menu"], "profile");

    office.publish(&id).await;
    office
        .call(
            "POST",
            &format!("{path}/unpublish"),
            None,
            &office.publisher,
        )
        .await;
    for (field, value) in [("slug", json!(slug)), ("menu", json!("services"))] {
        let mut changed = fields.clone();
        changed[field] = value;
        let reply = office
            .call("PUT", &path, Some(&changed), &office.writer)
            .await;
        assert_eq!(reply.status, StatusCode::CONFLICT, "{field}");
    }
}

#[sqlx::test]
async fn unpublishing_hides_a_page_and_only_a_draft_is_deleted(pool: PgPool) {
    let office = office(&pool).await;
    let (id, slug) = office
        .published(&Office::complete("Sejarah Desa", "profile"))
        .await;
    let path = format!("/pages/{id}");
    let public = format!("/pages/published/{slug}");

    let live = office.call("DELETE", &path, None, &office.publisher).await;
    let unpublished = office
        .call(
            "POST",
            &format!("{path}/unpublish"),
            None,
            &office.publisher,
        )
        .await;
    let hidden = office.app.get(&public).await.0;
    let listed = office.app.get("/pages/published").await.1;
    let by_writer = office.call("DELETE", &path, None, &office.writer).await;
    let deleted = office.call("DELETE", &path, None, &office.publisher).await;

    assert_eq!(live.status, StatusCode::CONFLICT);
    assert_eq!(unpublished.status, StatusCode::NO_CONTENT);
    assert_eq!(hidden, StatusCode::NOT_FOUND);
    assert_eq!(listed, json!([]));
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_eq!(
        office.call("GET", &path, None, &office.writer).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test]
async fn people_without_a_content_permission_see_no_drafts(pool: PgPool) {
    let office = office(&pool).await;
    office
        .draft(&json!({ "title": "Rahasia", "menu": "profile" }))
        .await;
    let manager = office
        .app
        .user_with(&pool, "operator", &["village.manage"])
        .await;

    assert_eq!(
        office.call("GET", "/pages", None, &manager).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        office.app.request("GET", "/pages").await,
        StatusCode::UNAUTHORIZED
    );
    let drafts = office.call("GET", "/pages", None, &office.writer).await;
    assert_eq!(drafts.body[0]["title"], "Rahasia");
    assert_eq!(drafts.body[0]["published"], false);
}
