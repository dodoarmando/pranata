mod common;

use axum::http::StatusCode;
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

/// A Village with one active and one inactive Hamlet; the cookie of a User who manages it and
/// reads the Audit Log.
async fn village(pool: &PgPool) -> (TestApp, String) {
    pranata::village::set_code(pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app
        .user_with(pool, "admin", &["village.manage", "audit.read"])
        .await;
    for (name, active) in [("Krajan", true), ("Kidul", false)] {
        let body = json!({ "kind": "hamlet", "name": name });
        let reply = app
            .call("POST", "/regions/units", Some(&body), Some(&cookie))
            .await;
        assert_eq!(reply.status, StatusCode::CREATED);
        if !active {
            let id = reply.body["id"].as_str().unwrap();
            let body = json!({ "name": name, "active": false });
            let reply = app
                .call(
                    "PUT",
                    &format!("/regions/units/{id}"),
                    Some(&body),
                    Some(&cookie),
                )
                .await;
            assert_eq!(reply.status, StatusCode::NO_CONTENT);
        }
    }
    (app, cookie)
}

/// The id of the Complaint Topic or Hamlet named `name`.
async fn id_of(pool: &PgPool, name: &str) -> String {
    sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM complaint_topics WHERE name = $1
         UNION ALL SELECT id FROM region_units WHERE name = $1",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
    .to_string()
}

/// A proof for a fresh challenge, solved as the browser does.
async fn proof(app: &TestApp) -> Value {
    let (status, challenge) = app.get("/public-form/challenge").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(challenge["algorithm"], "SHA-256");
    let salt = challenge["salt"].as_str().unwrap();
    let max = challenge["maxNumber"].as_u64().unwrap();
    let number = (0..=max)
        .find(|n| {
            let hash = Sha256::digest(format!("{salt}{n}"));
            let hex = hash
                .iter()
                .fold(String::new(), |hex, b| hex + &format!("{b:02x}"));
            hex == challenge["challenge"]
        })
        .unwrap();
    json!({ "salt": salt, "number": number, "signature": challenge["signature"] })
}

async fn send(app: &TestApp, ip: &str, body: &Value) -> Reply {
    app.call_with(
        "POST",
        "/complaints",
        Some(body),
        None,
        &[("x-forwarded-for", ip)],
    )
    .await
}

async fn status_of(app: &TestApp, ip: &str, code: &str) -> Reply {
    app.call_with(
        "GET",
        &format!("/complaints/status/{code}"),
        None,
        None,
        &[("x-forwarded-for", ip)],
    )
    .await
}

async fn complaints(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM complaints")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The answer every sending gets: `201` with a code of four groups of four, and nothing else.
fn code_of(reply: &Reply) -> String {
    assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
    let object = reply.body.as_object().unwrap();
    assert_eq!(object.len(), 1);
    let code = object["code"].as_str().unwrap();
    assert_eq!(code.len(), 19, "{code}");
    assert_eq!(code.matches('-').count(), 3, "{code}");
    code.to_owned()
}

#[sqlx::test]
async fn the_form_offers_active_topics_and_hamlets(pool: PgPool) {
    let (app, _) = village(&pool).await;
    sqlx::query("UPDATE complaint_topics SET active = false WHERE name = 'Keamanan'")
        .execute(&pool)
        .await
        .unwrap();
    let (status, form) = app.get("/complaints/form").await;
    assert_eq!(status, StatusCode::OK);
    let names = |list: &Value| -> Vec<String> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        names(&form["topics"]),
        ["Infrastruktur", "Pelayanan", "Bantuan Sosial", "Lainnya"]
    );
    assert_eq!(names(&form["hamlets"]), ["Krajan"]);
    assert_eq!(form["hamletLabel"], "dusun");
}

#[sqlx::test]
async fn a_complaint_is_received_audited_and_read_by_its_code_without_the_sender(pool: PgPool) {
    let (app, cookie) = village(&pool).await;
    let body = json!({
        "topicId": id_of(&pool, "Infrastruktur").await,
        "text": "  Jalan di RT 02 berlubang.  ",
        "hamletId": id_of(&pool, "Krajan").await,
        "name": "Warga",
        "phone": "0812-3456-7890",
        "website": "",
        "proof": proof(&app).await,
    });
    let code = code_of(&send(&app, "203.0.113.1", &body).await);
    let stored: (String, String, String) =
        sqlx::query_as("SELECT text, name, phone FROM complaints WHERE code = $1")
            .bind(&code)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored,
        (
            "Jalan di RT 02 berlubang.".into(),
            "Warga".into(),
            "0812-3456-7890".into()
        )
    );

    let reply = status_of(&app, "203.0.113.1", &code.to_lowercase()).await;
    assert_eq!(reply.status, StatusCode::OK);
    let status = reply.body.as_object().unwrap();
    assert_eq!(status["topic"], "Infrastruktur");
    assert_eq!(status["state"], "new");
    assert_eq!(status["rejectionReason"], Value::Null);
    assert_eq!(status["responses"], json!([]));
    assert!(status["submittedAt"].is_string());
    let mut keys: Vec<&str> = status.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "rejectionReason",
            "responses",
            "state",
            "submittedAt",
            "topic"
        ],
        "never the name, phone or Hamlet"
    );

    let reply = app
        .call(
            "GET",
            "/audit-entries?objectType=complaint",
            None,
            Some(&cookie),
        )
        .await;
    let entries = reply.body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["action"], "complaint.submit");
    assert_eq!(entries[0]["actor"], "anonymous");
    let entry = entries[0].to_string();
    assert!(
        !entry.contains("Warga") && !entry.contains("0812"),
        "{entry}"
    );
}

#[sqlx::test]
async fn every_stopped_sending_gets_the_answer_of_a_received_one(pool: PgPool) {
    let (app, _) = village(&pool).await;
    let topic = id_of(&pool, "Pelayanan").await;
    let body = |proof: Value| json!({ "topicId": topic, "text": "Antrean lama.", "proof": proof });

    let honeypot = json!({ "topicId": topic, "text": "Antrean lama.", "website": "https://spam.example", "proof": proof(&app).await });
    let no_proof = json!({ "topicId": topic, "text": "Antrean lama." });
    let mut forged = proof(&app).await;
    forged["signature"] = json!("00".repeat(32));
    let spent = proof(&app).await;
    code_of(&send(&app, "203.0.113.2", &body(spent.clone())).await);
    assert_eq!(complaints(&pool).await, 1);

    for (what, sending) in [
        ("honeypot", honeypot),
        ("no proof", no_proof),
        ("forged", body(forged)),
        ("spent", body(spent)),
    ] {
        let code = code_of(&send(&app, "203.0.113.3", &sending).await);
        assert_eq!(complaints(&pool).await, 1, "{what}");
        assert_eq!(
            status_of(&app, "203.0.113.9", &code).await.status,
            StatusCode::NOT_FOUND,
            "{what}"
        );
    }
}

#[sqlx::test]
async fn an_address_sends_at_most_5_complaints_an_hour(pool: PgPool) {
    let (app, _) = village(&pool).await;
    let topic = id_of(&pool, "Lainnya").await;
    for _ in 0..5 {
        let body = json!({ "topicId": topic, "text": "Saran.", "proof": proof(&app).await });
        code_of(&send(&app, "203.0.113.4", &body).await);
    }
    assert_eq!(complaints(&pool).await, 5);
    let body = json!({ "topicId": topic, "text": "Saran.", "proof": proof(&app).await });
    code_of(&send(&app, "203.0.113.4", &body).await);
    assert_eq!(
        complaints(&pool).await,
        5,
        "the sixth is answered but not kept"
    );
    let body = json!({ "topicId": topic, "text": "Saran.", "proof": proof(&app).await });
    code_of(&send(&app, "203.0.113.5", &body).await);
    assert_eq!(complaints(&pool).await, 6, "another address is not limited");
}

#[sqlx::test]
async fn malformed_fields_are_refused_before_the_proof_is_spent(pool: PgPool) {
    let (app, _) = village(&pool).await;
    let topic = id_of(&pool, "Lainnya").await;
    sqlx::query("UPDATE complaint_topics SET active = false WHERE name = 'Keamanan'")
        .execute(&pool)
        .await
        .unwrap();
    let inactive_topic = id_of(&pool, "Keamanan").await;
    let inactive_hamlet = id_of(&pool, "Kidul").await;
    let proof = proof(&app).await;
    let base = json!({ "topicId": topic, "text": "Saran.", "proof": proof });
    let with = |field: &str, value: Value| {
        let mut body = base.clone();
        body[field] = value;
        body
    };
    for body in [
        with("text", json!("   ")),
        with("text", json!("a".repeat(2001))),
        with("topicId", json!(inactive_topic)),
        with("topicId", json!(uuid::Uuid::nil())),
        with("hamletId", json!(inactive_hamlet)),
        with("name", json!("n".repeat(101))),
        with("phone", json!("bukan nomor")),
    ] {
        let reply = send(&app, "203.0.113.6", &body).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    // The same proof still goes through, with the longest text allowed.
    code_of(&send(&app, "203.0.113.6", &with("text", json!("é".repeat(2000)))).await);
    assert_eq!(complaints(&pool).await, 1);
}

#[sqlx::test]
async fn status_lookups_are_throttled_per_address(pool: PgPool) {
    let (app, _) = village(&pool).await;
    for _ in 0..20 {
        let reply = status_of(&app, "203.0.113.7", "0000-0000-0000-0000").await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
    }
    let reply = status_of(&app, "203.0.113.7", "0000-0000-0000-0000").await;
    assert_eq!(reply.status, StatusCode::TOO_MANY_REQUESTS);
    let reply = status_of(&app, "203.0.113.8", "0000-0000-0000-0000").await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn the_status_shows_responses_oldest_first_and_the_rejection_reason(pool: PgPool) {
    let (app, _) = village(&pool).await;
    let topic = id_of(&pool, "Lainnya").await;
    let body = json!({ "topicId": topic, "text": "Saran.", "proof": proof(&app).await });
    let code = code_of(&send(&app, "203.0.113.10", &body).await);
    // Staff answer in the admin area (#57); here straight in the database.
    sqlx::query(
        "WITH complaint AS (SELECT id FROM complaints WHERE code = $1)
         INSERT INTO complaint_responses (complaint_id, user_id, text, created_at)
         SELECT complaint.id, users.id, response.text, response.at
         FROM complaint, users, (VALUES ('Kedua', now()), ('Pertama', now() - interval '1 day'))
             AS response (text, at)",
    )
    .bind(&code)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE complaints SET state = 'rejected', rejection_reason = 'Di luar kewenangan desa' WHERE code = $1")
        .bind(&code)
        .execute(&pool)
        .await
        .unwrap();
    let reply = status_of(&app, "203.0.113.10", &code).await;
    assert_eq!(reply.body["state"], "rejected");
    assert_eq!(reply.body["rejectionReason"], "Di luar kewenangan desa");
    let texts: Vec<&Value> = reply.body["responses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|response| &response["text"])
        .collect();
    assert_eq!(texts, ["Pertama", "Kedua"]);
}
