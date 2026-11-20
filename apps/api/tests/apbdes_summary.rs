mod common;

use axum::http::StatusCode;
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A Village with a writer (`content.write`) and a publisher (`content.publish`).
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
    let publisher = app.user_with(pool, "redaksi", &["content.publish"]).await;
    Office {
        app,
        writer,
        publisher,
    }
}

fn lines(revenue: i64) -> Value {
    json!({
        "revenue": revenue, "governance": 792_000_000, "development": 1_020_000_000,
        "community": 160_000_000, "empowerment": 300_000_000, "emergencies": 105_000_000,
        "financingReceipts": 45_000_000, "financingExpenditures": 0,
    })
}

fn summary(fiscal_year: i32) -> Value {
    json!({
        "fiscalYear": fiscal_year, "version": "original", "versionDate": format!("{fiscal_year}-01-02"),
        "budget": lines(2_420_000_000), "realisation": null,
    })
}

impl Office {
    async fn call(&self, method: &str, path: &str, body: Option<&Value>, cookie: &str) -> Reply {
        self.app.call(method, path, body, Some(cookie)).await
    }

    async fn draft(&self, fields: &Value) -> String {
        let reply = self
            .call("POST", "/apbdes-summaries", Some(fields), &self.writer)
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{fields}");
        reply.body["id"].as_str().unwrap().to_owned()
    }

    async fn publish(&self, path: &str, id: &str) {
        let reply = self
            .call(
                "POST",
                &format!("/{path}/{id}/publish"),
                None,
                &self.publisher,
            )
            .await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT);
    }

    async fn document(&self, file: &Value, title: &str, document_type: &str, fiscal_year: i32) {
        let fields = json!({
            "title": title, "documentType": document_type, "fiscalYear": fiscal_year,
            "file": file,
        });
        let reply = self
            .call(
                "POST",
                "/transparency-documents",
                Some(&fields),
                &self.writer,
            )
            .await;
        assert_eq!(reply.status, StatusCode::CREATED);
        if !title.starts_with("Draf") {
            self.publish("transparency-documents", reply.body["id"].as_str().unwrap())
                .await;
        }
    }
}

#[sqlx::test]
async fn a_writer_drafts_a_publisher_publishes_and_the_public_reads_it(pool: PgPool) {
    let office = office(&pool).await;
    let mut fields = summary(2026);
    fields["version"] = json!("amendment");
    fields["versionDate"] = json!("2026-09-02");
    fields["realisation"] = json!({ "asOf": "2026-06-30" });
    fields["realisation"]
        .as_object_mut()
        .unwrap()
        .extend(lines(1_355_200_000).as_object().unwrap().clone());
    let id = office.draft(&fields).await;
    assert_eq!(
        office.app.get("/apbdes-summaries/published").await.0,
        StatusCode::NOT_FOUND
    );
    let list = office
        .call("GET", "/apbdes-summaries", None, &office.writer)
        .await;
    let mut listed = fields.clone();
    listed["id"] = json!(id);
    listed["published"] = json!(false);
    assert_eq!(list.body, json!([listed]));

    let path = format!("/apbdes-summaries/{id}");
    let refused = office
        .call("POST", &format!("{path}/publish"), None, &office.writer)
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    office.publish("apbdes-summaries", &id).await;
    let mut public = fields.clone();
    public["documents"] = json!([]);
    public["fiscalYears"] = json!([2026]);
    assert_eq!(
        office.app.get("/apbdes-summaries/published").await,
        (StatusCode::OK, public.clone())
    );
    assert_eq!(
        office
            .app
            .get("/apbdes-summaries/published?fiscalYear=2026")
            .await
            .1,
        public
    );

    // A live summary changes only on a publisher's save, and is deleted only as a Draft.
    fields["budget"]["revenue"] = json!(2_500_000_000_i64);
    let by_writer = office
        .call("PUT", &path, Some(&fields), &office.writer)
        .await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let by_publisher = office
        .call("PUT", &path, Some(&fields), &office.publisher)
        .await;
    assert_eq!(by_publisher.status, StatusCode::NO_CONTENT);
    let live = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(live.status, StatusCode::CONFLICT);
    let unpublished = office
        .call(
            "POST",
            &format!("{path}/unpublish"),
            None,
            &office.publisher,
        )
        .await;
    assert_eq!(unpublished.status, StatusCode::NO_CONTENT);
    let deleted = office.call("DELETE", &path, None, &office.publisher).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let list = office
        .call("GET", "/apbdes-summaries", None, &office.writer)
        .await;
    assert_eq!(list.body, json!([]));
}

#[sqlx::test]
async fn a_fiscal_year_holds_one_summary(pool: PgPool) {
    let office = office(&pool).await;
    office.draft(&summary(2026)).await;
    let other = office.draft(&summary(2025)).await;
    let twice = office
        .call(
            "POST",
            "/apbdes-summaries",
            Some(&summary(2026)),
            &office.writer,
        )
        .await;
    assert_eq!(twice.status, StatusCode::CONFLICT);
    let moved = office
        .call(
            "PUT",
            &format!("/apbdes-summaries/{other}"),
            Some(&summary(2026)),
            &office.writer,
        )
        .await;
    assert_eq!(moved.status, StatusCode::CONFLICT);
}

#[sqlx::test]
async fn malformed_figures_are_refused(pool: PgPool) {
    let office = office(&pool).await;
    let mut negative = summary(2026);
    negative["budget"]["governance"] = json!(-1);
    let mut fractional = summary(2026);
    fractional["budget"]["revenue"] = json!(1.5);
    let mut huge = summary(2026);
    huge["budget"]["revenue"] = json!(1_000_000_000_000_000_i64);
    let mut partial = summary(2026);
    partial["realisation"] = json!({ "asOf": "2026-06-30", "revenue": 1 });
    for fields in [summary(1999), negative, fractional, huge, partial] {
        let reply = office
            .call("POST", "/apbdes-summaries", Some(&fields), &office.writer)
            .await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{fields}");
    }
}

#[sqlx::test]
async fn the_public_reads_the_latest_published_year_with_its_budget_documents(pool: PgPool) {
    let office = office(&pool).await;
    for year in [2024, 2025] {
        let id = office.draft(&summary(year)).await;
        office.publish("apbdes-summaries", &id).await;
    }
    office.draft(&summary(2026)).await;
    let upload = office
        .app
        .upload(
            "/transparency-documents/file",
            b"%PDF-1.7\n".to_vec(),
            &office.writer,
        )
        .await;
    let file = upload.body["id"].clone();
    office.document(&file, "APBDes 2025", "apbdes", 2025).await;
    office
        .document(&file, "Perubahan APBDes 2025", "apbdesAmendment", 2025)
        .await;
    office
        .document(&file, "Realisasi 2025", "realisationReport", 2025)
        .await;
    office.document(&file, "RKPDes 2025", "rkpdes", 2025).await;
    office
        .document(&file, "Draf APBDes 2025", "apbdes", 2025)
        .await;
    office.document(&file, "APBDes 2024", "apbdes", 2024).await;

    // The Draft year is not public, so the latest is the newest Published year.
    let (status, latest) = office.app.get("/apbdes-summaries/published").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(latest["fiscalYear"], 2025);
    assert_eq!(latest["fiscalYears"], json!([2025, 2024]));
    let titles: Vec<&str> = latest["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|document| document["title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        ["Realisasi 2025", "Perubahan APBDes 2025", "APBDes 2025"]
    );
    let (_, older) = office
        .app
        .get("/apbdes-summaries/published?fiscalYear=2024")
        .await;
    assert_eq!(older["documents"][0]["title"], "APBDes 2024");
    assert_eq!(
        office
            .app
            .get("/apbdes-summaries/published?fiscalYear=2026")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
