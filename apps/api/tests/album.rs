mod common;

use axum::http::StatusCode;
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A Village with a writer (`content.write`), a publisher (`content.publish`, `audit.read`) and
/// three uploaded photos.
struct Office {
    app: TestApp,
    writer: String,
    publisher: String,
    photos: Vec<Value>,
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
    let mut photos = Vec::new();
    for width in [64, 48, 32] {
        let reply = app
            .upload("/albums/photo", common::png(width, 32), &writer)
            .await;
        assert_eq!(reply.status, StatusCode::CREATED);
        photos.push(reply.body["id"].clone());
    }
    Office {
        app,
        writer,
        publisher,
        photos,
    }
}

impl Office {
    /// Every field publishing requires, with the given photos in order.
    fn complete(title: &str, photos: &[&Value]) -> Value {
        json!({
            "title": title, "occasionStart": "2026-08-17", "description": " Lomba dan upacara. ",
            "photos": photos.iter().map(|&file| json!({
                "file": file, "caption": " Upacara ", "alt": " Warga berbaris di lapangan ",
            })).collect::<Vec<_>>(),
        })
    }

    async fn call(&self, method: &str, path: &str, body: Option<&Value>, cookie: &str) -> Reply {
        self.app.call(method, path, body, Some(cookie)).await
    }

    /// A new Draft's id and slug.
    async fn draft(&self, fields: &Value) -> (String, String) {
        let reply = self
            .call("POST", "/albums", Some(fields), &self.writer)
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{fields}");
        let id = reply.body["id"].as_str().unwrap().to_owned();
        let album = self
            .call("GET", &format!("/albums/{id}"), None, &self.writer)
            .await;
        (id, album.body["slug"].as_str().unwrap().to_owned())
    }

    async fn save(&self, id: &str, fields: &Value, cookie: &str) -> StatusCode {
        self.call("PUT", &format!("/albums/{id}"), Some(fields), cookie)
            .await
            .status
    }

    async fn publish(&self, id: &str) -> StatusCode {
        self.call(
            "POST",
            &format!("/albums/{id}/publish"),
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
async fn a_writer_drafts_a_publisher_publishes_and_the_public_browses_it(pool: PgPool) {
    let office = office(&pool).await;
    let (id, slug) = office.draft(&json!({ "title": "HUT RI ke-81" })).await;
    assert_eq!(slug, "hut-ri-ke-81");
    let public = format!("/albums/published/{slug}");
    assert_eq!(office.app.get(&public).await.0, StatusCode::NOT_FOUND);

    // The autosave fills in the rest; the writer cannot publish.
    let [first, second, _] = &office.photos[..] else {
        unreachable!()
    };
    let mut fields = Office::complete("HUT RI ke-81", &[first, second]);
    fields["occasionEnd"] = json!("2026-08-18");
    assert_eq!(
        office.save(&id, &fields, &office.writer).await,
        StatusCode::NO_CONTENT
    );
    let refused = office
        .call(
            "POST",
            &format!("/albums/{id}/publish"),
            None,
            &office.writer,
        )
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(office.publish(&id).await, StatusCode::NO_CONTENT);

    let (status, album) = office.app.get(&public).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(album["title"], "HUT RI ke-81");
    assert_eq!(album["description"], "Lomba dan upacara.");
    assert_eq!(album["occasionStart"], "2026-08-17");
    assert_eq!(album["occasionEnd"], "2026-08-18");
    // Without a chosen cover, the first photo is the cover.
    assert_eq!(album["cover"], *first);
    assert_eq!(
        album["photos"],
        json!([
            { "file": first, "caption": "Upacara", "alt": "Warga berbaris di lapangan" },
            { "file": second, "caption": "Upacara", "alt": "Warga berbaris di lapangan" },
        ])
    );
    let (status, gallery) = office.app.get("/albums/published").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        gallery,
        json!({ "albums": [{
            "slug": slug, "title": "HUT RI ke-81", "occasionStart": "2026-08-17",
            "occasionEnd": "2026-08-18", "cover": first, "coverAlt": "Warga berbaris di lapangan",
            "photoCount": 2,
        }], "total": 1, "pageSize": 12 })
    );

    // A live Album changes only on a publisher's explicit save.
    fields["title"] = json!("Peringatan HUT RI ke-81");
    assert_eq!(
        office.save(&id, &fields, &office.writer).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        office.save(&id, &fields, &office.publisher).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        office.app.get(&public).await.1["title"],
        "Peringatan HUT RI ke-81"
    );

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
        .filter(|action| action.starts_with("album."))
        .collect();
    assert_eq!(
        actions,
        [
            "album.update",
            "album.publish",
            "album.update",
            "album.create"
        ]
    );
    assert_eq!(
        entries[0]["changes"],
        json!([{
            "field": "title", "from": "HUT RI ke-81", "to": "Peringatan HUT RI ke-81",
            "sensitive": false,
        }])
    );
}

#[sqlx::test]
async fn an_album_without_photos_cannot_be_published(pool: PgPool) {
    let office = office(&pool).await;
    let photo = &office.photos[0];

    let (empty, _) = office.draft(&Office::complete("Kosong", &[])).await;
    assert_eq!(
        office.publish(&empty).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Every photo needs alt text, and the occasion needs its start.
    let mut no_alt = Office::complete("Tanpa Alt", &[photo]);
    no_alt["photos"][0]["alt"] = json!(" ");
    let (id, _) = office.draft(&no_alt).await;
    assert_eq!(office.publish(&id).await, StatusCode::UNPROCESSABLE_ENTITY);
    let mut undated = Office::complete("Tanpa Tanggal", &[photo]);
    undated["occasionStart"] = Value::Null;
    let (id, _) = office.draft(&undated).await;
    assert_eq!(office.publish(&id).await, StatusCode::UNPROCESSABLE_ENTITY);

    // A Published Album cannot lose its last photo or a photo's alt text.
    let (id, _) = office
        .published(&Office::complete("Kerja Bakti", &[photo]))
        .await;
    assert_eq!(
        office
            .save(
                &id,
                &Office::complete("Kerja Bakti", &[]),
                &office.publisher
            )
            .await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut fields = Office::complete("Kerja Bakti", &[photo]);
    fields["photos"][0]["alt"] = Value::Null;
    assert_eq!(
        office.save(&id, &fields, &office.publisher).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let album = office.app.get("/albums/published/kerja-bakti").await.1;
    assert_eq!(album["photos"][0]["file"], *photo);
}

#[sqlx::test]
async fn photos_keep_their_order_and_the_cover_is_chosen(pool: PgPool) {
    let office = office(&pool).await;
    let [a, b, c] = &office.photos[..] else {
        unreachable!()
    };
    let (id, slug) = office
        .published(&Office::complete("Posyandu", &[a, b, c]))
        .await;

    // Reordering and choosing the second photo as the cover.
    let mut fields = Office::complete("Posyandu", &[c, a, b]);
    fields["photos"][1]["caption"] = json!("Penimbangan");
    fields["cover"] = a.clone();
    assert_eq!(
        office.save(&id, &fields, &office.publisher).await,
        StatusCode::NO_CONTENT
    );

    let edited = office
        .call("GET", &format!("/albums/{id}"), None, &office.writer)
        .await
        .body;
    let files = |album: &Value| -> Vec<Value> {
        album["photos"]
            .as_array()
            .unwrap()
            .iter()
            .map(|photo| photo["file"].clone())
            .collect()
    };
    assert_eq!(files(&edited), [c.clone(), a.clone(), b.clone()]);
    assert_eq!(edited["cover"], *a);
    let album = office.app.get(&format!("/albums/published/{slug}")).await.1;
    assert_eq!(files(&album), [c.clone(), a.clone(), b.clone()]);
    assert_eq!(album["photos"][1]["caption"], "Penimbangan");
    assert_eq!(album["cover"], *a);
    let card = office.app.get("/albums/published").await.1["albums"][0].clone();
    assert_eq!(card["cover"], *a);
    assert_eq!(card["photoCount"], 3);

    // The cover must be one of the photos, a photo appears once, and the occasion ends after
    // it starts.
    let mut foreign = Office::complete("Posyandu", &[c]);
    foreign["cover"] = a.clone();
    let twice = Office::complete("Posyandu", &[a, a]);
    let mut backwards = Office::complete("Posyandu", &[a]);
    backwards["occasionEnd"] = json!("2026-08-16");
    let mut unknown = Office::complete("Posyandu", &[a]);
    unknown["photos"][0]["file"] = json!("01900000-0000-7000-8000-000000000000");
    for fields in [foreign, twice, backwards, unknown] {
        assert_eq!(
            office.save(&id, &fields, &office.publisher).await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{fields}"
        );
    }
}

#[sqlx::test]
async fn the_gallery_shows_the_latest_occasion_first(pool: PgPool) {
    let office = office(&pool).await;
    let photo = &office.photos[0];
    for (title, start) in [
        ("Lebaran", "2026-03-20"),
        ("HUT RI", "2026-08-17"),
        ("Tahun Baru", "2026-01-01"),
    ] {
        let mut fields = Office::complete(title, &[photo]);
        fields["occasionStart"] = json!(start);
        office.published(&fields).await;
    }
    office
        .draft(&Office::complete("Belum Terbit", &[photo]))
        .await;

    let gallery = office.app.get("/albums/published").await.1;

    let titles: Vec<_> = gallery["albums"]
        .as_array()
        .unwrap()
        .iter()
        .map(|album| album["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["HUT RI", "Lebaran", "Tahun Baru"]);
    assert_eq!(gallery["total"], 3);
}

#[sqlx::test]
async fn unpublishing_hides_an_album_and_only_a_draft_is_deleted(pool: PgPool) {
    let office = office(&pool).await;
    let (id, slug) = office
        .published(&Office::complete("Musdes", &[&office.photos[0]]))
        .await;
    let path = format!("/albums/{id}");

    let live = office.call("DELETE", &path, None, &office.publisher).await;
    let unpublished = office
        .call(
            "POST",
            &format!("{path}/unpublish"),
            None,
            &office.publisher,
        )
        .await;
    let draft = office.call("GET", &path, None, &office.writer).await.body;
    let hidden = office.app.get(&format!("/albums/published/{slug}")).await.0;
    let listed = office.app.get("/albums/published").await.1;
    let by_writer = office.call("DELETE", &path, None, &office.writer).await;
    let deleted = office.call("DELETE", &path, None, &office.publisher).await;

    assert_eq!(live.status, StatusCode::CONFLICT);
    assert_eq!(unpublished.status, StatusCode::NO_CONTENT);
    assert_eq!(draft["published"], false);
    assert!(draft["firstPublishedAt"].is_string());
    assert_eq!(hidden, StatusCode::NOT_FOUND);
    assert_eq!(listed["total"], 0);
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_eq!(
        office.call("GET", &path, None, &office.writer).await.status,
        StatusCode::NOT_FOUND
    );
    let (_, again) = office
        .draft(&Office::complete("Musdes", &[&office.photos[0]]))
        .await;
    assert_eq!(again, slug);
}

#[sqlx::test]
async fn people_without_a_content_permission_see_no_drafts(pool: PgPool) {
    let office = office(&pool).await;
    office.draft(&json!({ "title": "Rahasia" })).await;
    let manager = office
        .app
        .user_with(&pool, "operator", &["village.manage"])
        .await;

    assert_eq!(
        office.call("GET", "/albums", None, &manager).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        office.app.request("GET", "/albums").await,
        StatusCode::UNAUTHORIZED
    );
    let drafts = office.call("GET", "/albums", None, &office.writer).await;
    assert_eq!(drafts.body[0]["title"], "Rahasia");
    assert_eq!(drafts.body[0]["published"], false);
    assert_eq!(drafts.body[0]["photoCount"], 0);
}

#[sqlx::test]
async fn an_albums_photos_are_kept_and_a_removed_one_is_swept(pool: PgPool) {
    let office = office(&pool).await;
    let [kept, removed, _] = &office.photos[..] else {
        unreachable!()
    };
    let (id, _) = office
        .draft(&Office::complete("Musdes", &[kept, removed]))
        .await;
    // Only the third photo, saved into no Album, goes.
    assert_eq!(sweep_a_day_later(&office, &pool).await, 1);
    assert!(served(&office, kept).await && served(&office, removed).await);

    assert_eq!(
        office
            .save(&id, &Office::complete("Musdes", &[kept]), &office.writer)
            .await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(sweep_a_day_later(&office, &pool).await, 1);
    assert!(served(&office, kept).await);
    assert!(!served(&office, removed).await);
}

async fn served(office: &Office, file: &Value) -> bool {
    let path = format!("/media/{}/480", file.as_str().unwrap());
    office.app.get_raw(&path).await.0 == StatusCode::OK
}

/// Sweeps as if a day had gone by since every upload or release.
async fn sweep_a_day_later(office: &Office, pool: &PgPool) -> usize {
    sqlx::query("UPDATE files SET sweep_after = now() - interval '1 second'")
        .execute(pool)
        .await
        .unwrap();
    pranata::storage::sweep(pool, office.app.files.path())
        .await
        .unwrap()
}
