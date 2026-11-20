mod common;

use axum::http::{StatusCode, header};
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// The smallest bytes that pass as a PDF.
fn pdf(len: usize) -> Vec<u8> {
    let mut bytes = b"%PDF-1.7\n".to_vec();
    bytes.resize(len.max(bytes.len()), b' ');
    bytes
}

/// A Village with a writer (`content.write`), a publisher (`content.publish`) and an uploaded
/// PDF.
struct Office {
    app: TestApp,
    writer: String,
    publisher: String,
    file: Value,
}

async fn office(pool: &PgPool) -> Office {
    pranata::village::set_code(pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let writer = app.user_with(pool, "penulis", &["content.write"]).await;
    let publisher = app.user_with(pool, "redaksi", &["content.publish"]).await;
    let upload = app
        .upload("/transparency-documents/file", pdf(2048), &writer)
        .await;
    assert_eq!(upload.status, StatusCode::CREATED);
    Office {
        app,
        writer,
        publisher,
        file: upload.body["id"].clone(),
    }
}

impl Office {
    async fn call(&self, method: &str, path: &str, body: Option<&Value>, cookie: &str) -> Reply {
        self.app.call(method, path, body, Some(cookie)).await
    }

    async fn published(&self, title: &str, document_type: &str, fiscal_year: i32) -> String {
        let fields = json!({
            "title": title, "documentType": document_type, "fiscalYear": fiscal_year,
            "file": self.file,
        });
        let reply = self
            .call(
                "POST",
                "/transparency-documents",
                Some(&fields),
                &self.writer,
            )
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{fields}");
        let id = reply.body["id"].as_str().unwrap().to_owned();
        let path = format!("/transparency-documents/{id}/publish");
        let published = self.call("POST", &path, None, &self.publisher).await;
        assert_eq!(published.status, StatusCode::NO_CONTENT);
        id
    }

    async fn public_titles(&self, query: &str) -> Vec<String> {
        let (status, page) = self
            .app
            .get(&format!("/transparency-documents/published{query}"))
            .await;
        assert_eq!(status, StatusCode::OK);
        page["documents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|document| document["title"].as_str().unwrap().to_owned())
            .collect()
    }
}

#[sqlx::test]
async fn a_writer_drafts_a_publisher_publishes_and_the_public_downloads_it(pool: PgPool) {
    let office = office(&pool).await;
    let fields = json!({
        "title": " Perdes Nomor 1 Tahun 2026 tentang APBDes ", "documentType": "apbdes",
        "fiscalYear": 2026, "file": office.file,
    });
    let created = office
        .call(
            "POST",
            "/transparency-documents",
            Some(&fields),
            &office.writer,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let id = created.body["id"].as_str().unwrap().to_owned();
    assert_eq!(office.public_titles("").await, Vec::<String>::new());
    let list = office
        .call("GET", "/transparency-documents", None, &office.writer)
        .await;
    assert_eq!(
        list.body,
        json!([{
            "id": id, "title": "Perdes Nomor 1 Tahun 2026 tentang APBDes",
            "documentType": "apbdes", "fiscalYear": 2026, "file": office.file,
            "byteSize": 2048, "published": false,
        }])
    );

    let path = format!("/transparency-documents/{id}");
    let refused = office
        .call("POST", &format!("{path}/publish"), None, &office.writer)
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    let published = office
        .call("POST", &format!("{path}/publish"), None, &office.publisher)
        .await;
    assert_eq!(published.status, StatusCode::NO_CONTENT);
    let again = office
        .call("POST", &format!("{path}/publish"), None, &office.publisher)
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(
        office.app.get("/transparency-documents/published").await.1,
        json!({
            "documents": [{
                "title": "Perdes Nomor 1 Tahun 2026 tentang APBDes", "documentType": "apbdes",
                "fiscalYear": 2026, "file": office.file, "byteSize": 2048,
            }],
            "total": 1, "pageSize": 10, "fiscalYears": [2026],
        })
    );

    // The PDF is served so that a browser neither sniffs nor runs it.
    let file = office.file.as_str().unwrap();
    let (status, headers, bytes) = office.app.get_raw(&format!("/media/{file}/pdf")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(headers[header::CONTENT_SECURITY_POLICY], "sandbox");
    assert_eq!(bytes, pdf(2048));
    let (status, _, _) = office.app.get_raw(&format!("/media/{file}/480")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A live document changes only on a publisher's save; a new PDF releases the old one.
    let replacement = office
        .app
        .upload("/transparency-documents/file", pdf(4096), &office.publisher)
        .await
        .body["id"]
        .clone();
    let fields = json!({
        "title": "Perdes Nomor 4 Tahun 2026 tentang Perubahan APBDes",
        "documentType": "apbdesAmendment", "fiscalYear": 2026, "file": replacement,
    });
    let by_writer = office
        .call("PUT", &path, Some(&fields), &office.writer)
        .await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let by_publisher = office
        .call("PUT", &path, Some(&fields), &office.publisher)
        .await;
    assert_eq!(by_publisher.status, StatusCode::NO_CONTENT);
    let page = office.app.get("/transparency-documents/published").await.1;
    assert_eq!(page["documents"][0]["documentType"], "apbdesAmendment");
    assert_eq!(page["documents"][0]["byteSize"], 4096);
    let released: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT sweep_after FROM files WHERE id = $1::text::uuid")
            .bind(file)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(released.is_some(), "the replaced PDF is swept later");

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
    assert_eq!(office.public_titles("").await, Vec::<String>::new());
    let by_writer = office.call("DELETE", &path, None, &office.writer).await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let deleted = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let gone = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);

    let audited: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_entries WHERE action LIKE 'transparency_document.%'
         ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        audited,
        [
            "transparency_document.create",
            "transparency_document.publish",
            "transparency_document.update",
            "transparency_document.unpublish",
            "transparency_document.delete",
        ]
    );
}

#[sqlx::test]
async fn the_public_filters_by_type_group_and_fiscal_year(pool: PgPool) {
    let office = office(&pool).await;
    office.published("APBDes 2025", "apbdes", 2025).await;
    office
        .published("Perubahan APBDes 2026", "apbdesAmendment", 2026)
        .await;
    office
        .published("Realisasi 2026", "realisationReport", 2026)
        .await;
    office.published("RKPDes 2027", "rkpdes", 2027).await;
    office.published("RPJMDes 2025–2031", "rpjmdes", 2025).await;
    office
        .published("Perdes Pungutan", "villageRegulation", 2025)
        .await;
    office.published("Notulen", "other", 2026).await;

    // The latest fiscal year first.
    assert_eq!(
        office.public_titles("").await,
        [
            "RKPDes 2027",
            "Notulen",
            "Realisasi 2026",
            "Perubahan APBDes 2026",
            "Perdes Pungutan",
            "RPJMDes 2025–2031",
            "APBDes 2025",
        ]
    );
    assert_eq!(
        office.public_titles("?group=budget").await,
        ["Perubahan APBDes 2026", "APBDes 2025"]
    );
    assert_eq!(
        office.public_titles("?group=realisation").await,
        ["Realisasi 2026"]
    );
    assert_eq!(
        office.public_titles("?group=planning").await,
        ["RKPDes 2027", "RPJMDes 2025–2031"]
    );
    assert_eq!(
        office.public_titles("?group=regulation").await,
        ["Perdes Pungutan"]
    );
    assert_eq!(
        office.public_titles("?group=budget&fiscalYear=2025").await,
        ["APBDes 2025"]
    );
    let (_, page) = office
        .app
        .get("/transparency-documents/published?fiscalYear=2026")
        .await;
    assert_eq!(page["total"], 3);
    assert_eq!(page["fiscalYears"], json!([2027, 2026, 2025]));
    let (status, _) = office
        .app
        .get("/transparency-documents/published?group=minutes")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn a_document_needs_a_title_a_catalog_type_a_fiscal_year_and_a_pdf(pool: PgPool) {
    let office = office(&pool).await;
    let photo = office
        .app
        .upload("/partners/logo", common::png(8, 8), &office.writer)
        .await
        .body["id"]
        .clone();
    let valid = json!({
        "title": "RKPDes 2027", "documentType": "rkpdes", "fiscalYear": 2027,
        "file": office.file,
    });
    for (field, value) in [
        ("title", json!(" ")),
        ("title", json!("x".repeat(201))),
        ("documentType", json!("minutes")),
        ("documentType", json!("RKPDes")),
        ("fiscalYear", json!(1999)),
        ("fiscalYear", json!(2101)),
        ("file", photo.clone()),
        ("file", json!("01900000-0000-7000-8000-000000000000")),
    ] {
        let mut fields = valid.clone();
        fields[field] = value;
        let reply = office
            .call(
                "POST",
                "/transparency-documents",
                Some(&fields),
                &office.writer,
            )
            .await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{fields}");
    }
    let reply = office
        .call(
            "POST",
            "/transparency-documents",
            Some(&valid),
            &office.writer,
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED);
}

#[sqlx::test]
async fn the_file_must_be_a_pdf_of_at_most_25_mib(pool: PgPool) {
    let office = office(&pool).await;
    for (upload, expected) in [
        (pdf(25 << 20), StatusCode::CREATED),
        (pdf((25 << 20) + 1), StatusCode::PAYLOAD_TOO_LARGE),
        (common::png(8, 8), StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (
            b"<html><script>alert(1)</script>".to_vec(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
    ] {
        let reply = office
            .app
            .upload("/transparency-documents/file", upload, &office.writer)
            .await;
        assert_eq!(reply.status, expected);
    }
    let reader = office
        .app
        .user_with(&pool, "pembaca", &["audit.read"])
        .await;
    let reply = office
        .app
        .upload("/transparency-documents/file", pdf(64), &reader)
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
}
