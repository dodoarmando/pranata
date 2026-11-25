mod common;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::{test_state, test_state_with_web};
use http_body_util::BodyExt;
use pranata::{AppState, auth, jobs, region_codes::VillageCode, village};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpListener};
use tower::ServiceExt;
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery";

struct Reply {
    status: StatusCode,
    body: Value,
}

struct Harness {
    pool: PgPool,
    state: AppState,
    app: Router,
    cookie: String,
    csrf: String,
    user_id: Uuid,
}

impl Harness {
    async fn new(pool: PgPool) -> Self {
        Self::with_state(pool.clone(), test_state(pool)).await
    }

    async fn with_state(pool: PgPool, state: AppState) -> Self {
        village::init(&pool, &VillageCode::parse("14.05.03.2002").unwrap())
            .await
            .unwrap();
        let app = pranata::app(state.clone());
        let hash = auth::password::hash_password(PASSWORD).unwrap();
        let role = common::administrator_role(&pool).await;
        let user_id = sqlx::query_scalar!(
            "INSERT INTO users (username, display_name, password_hash, role_id)
             VALUES ('budi', 'Budi Santoso', $1, $2) RETURNING id",
            hash,
            role.0
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let login = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"username": "budi", "password": PASSWORD}).to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(login).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let csrf = serde_json::from_slice::<Value>(&bytes).unwrap()["csrf_token"]
            .as_str()
            .unwrap()
            .to_owned();
        Self {
            pool,
            state,
            app,
            cookie,
            csrf,
            user_id,
        }
    }

    async fn send(&self, request: Request<Body>) -> Reply {
        let response = self.app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        Reply {
            status: parts.status,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        }
    }

    async fn get_profile(&self) -> Reply {
        self.send(
            Request::builder()
                .uri("/api/admin/village/profile")
                .header(header::COOKIE, &self.cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    async fn put_profile(&self, body: &Value) -> Reply {
        self.send(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/admin/village/profile")
                .header(header::COOKIE, &self.cookie)
                .header("x-csrf-token", &self.csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
    }

    async fn public_village(&self) -> Reply {
        self.send(
            Request::builder()
                .uri("/api/public/village")
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    /// Insert an image or document asset with one 480 px variant when ready.
    async fn asset(&self, kind: &str, status: &str) -> Uuid {
        let sha = Uuid::new_v4().as_bytes().repeat(2);
        let id = sqlx::query_scalar!(
            "INSERT INTO media_assets (sha256, kind, mime, original_name, bytes, status, created_by)
             VALUES ($1, $2, 'image/jpeg', 'a.jpg', 10, $3, $4) RETURNING id",
            sha,
            kind,
            status,
            self.user_id,
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        if status == "ready" && kind == "image" {
            let path = format!("public/ab/{id}-480.webp");
            sqlx::query!(
                "INSERT INTO media_variants (asset_id, width, path, bytes) VALUES ($1, 480, $2, 10)",
                id,
                path
            )
            .execute(&self.pool)
            .await
            .unwrap();
        }
        id
    }
}

fn doc(text: &str) -> Value {
    json!({"type": "doc", "content": [
        {"type": "paragraph", "content": [{"type": "text", "text": text}]}
    ]})
}

async fn job_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM jobs WHERE kind = 'purge_public_cache'"#)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn put_profile_updates_and_audits(pool: PgPool) {
    let h = Harness::new(pool).await;
    let hero = h.asset("image", "ready").await;
    let current = h.get_profile().await;
    assert_eq!(current.status, StatusCode::OK, "{}", current.body);
    assert_eq!(current.body["overview"], Value::Null);
    let updated_at = current.body["updated_at"].as_str().unwrap().to_owned();

    let put = h
        .put_profile(&json!({
            "overview": doc("Selamat datang"),
            "hero_asset_id": hero,
            "updated_at": updated_at,
        }))
        .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    assert_eq!(put.body["overview"], doc("Selamat datang"));
    assert_eq!(put.body["hero"]["id"], hero.to_string());
    assert_ne!(put.body["updated_at"], json!(updated_at));

    let again = h.get_profile().await;
    assert_eq!(again.body["overview"], doc("Selamat datang"));
    assert_eq!(again.body["updated_at"], put.body["updated_at"]);

    let entry = sqlx::query!(
        r#"SELECT actor, action, object_type, object_id, changes FROM audit_entries
           WHERE object_type = 'village' AND action = 'update'"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(entry.actor, Some(h.user_id));
    assert_eq!(entry.object_id, "1");
    let fields: Vec<&str> = entry
        .changes
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["field"].as_str().unwrap())
        .collect();
    assert_eq!(fields, ["hero_asset_id", "overview"]);
}

#[sqlx::test]
async fn put_profile_enqueues_purge_in_same_tx(pool: PgPool) {
    let h = Harness::new(pool).await;
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let put = h
        .put_profile(
            &json!({"overview": doc("Satu"), "hero_asset_id": null, "updated_at": updated_at}),
        )
        .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    assert_eq!(job_count(&h.pool).await, 1);

    // A failing job insert rolls back the update and the audit entry.
    sqlx::raw_sql(
        "CREATE FUNCTION fail_jobs() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN RAISE EXCEPTION 'no jobs'; END $$;
         CREATE TRIGGER fail_jobs BEFORE INSERT ON jobs
         FOR EACH ROW EXECUTE FUNCTION fail_jobs();",
    )
    .execute(&h.pool)
    .await
    .unwrap();
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let failed = h
        .put_profile(
            &json!({"overview": doc("Zwei"), "hero_asset_id": null, "updated_at": updated_at}),
        )
        .await;
    assert_eq!(failed.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(job_count(&h.pool).await, 1);
    let after = h.get_profile().await;
    assert_eq!(after.body["overview"], doc("Satu"));
    let audits = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM audit_entries WHERE object_type = 'village' AND action = 'update'"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);
}

#[sqlx::test]
async fn stale_update_is_409(pool: PgPool) {
    let h = Harness::new(pool).await;
    let first = h.get_profile().await.body["updated_at"].clone();
    let ok = h
        .put_profile(&json!({"overview": doc("A"), "hero_asset_id": null, "updated_at": first}))
        .await;
    assert_eq!(ok.status, StatusCode::OK);
    let stale = h
        .put_profile(&json!({"overview": doc("B"), "hero_asset_id": null, "updated_at": first}))
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(stale.body["code"], "stale_write");
    assert_eq!(h.get_profile().await.body["overview"], doc("A"));
    assert_eq!(job_count(&h.pool).await, 1);
}

#[sqlx::test]
async fn unusable_heroes_are_422(pool: PgPool) {
    let h = Harness::new(pool).await;
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let candidates = [
        h.asset("image", "pending").await,
        h.asset("image", "failed").await,
        h.asset("document", "ready").await,
        Uuid::new_v4(),
    ];
    for hero in candidates {
        let reply = h
            .put_profile(
                &json!({"overview": null, "hero_asset_id": hero, "updated_at": updated_at}),
            )
            .await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{hero}");
        assert_eq!(reply.body["code"], "hero_not_ready");
    }
    assert_eq!(job_count(&h.pool).await, 0);
}

#[sqlx::test]
async fn pending_hero_is_422(pool: PgPool) {
    let h = Harness::new(pool).await;
    let hero = h.asset("image", "pending").await;
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let reply = h
        .put_profile(&json!({"overview": null, "hero_asset_id": hero, "updated_at": updated_at}))
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.body["code"], "hero_not_ready");
}

#[sqlx::test]
async fn invalid_overview_is_a_field_error(pool: PgPool) {
    let h = Harness::new(pool).await;
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let reply = h
        .put_profile(&json!({"overview": {"type": "paragraph"}, "hero_asset_id": null, "updated_at": updated_at}))
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.body["code"], "validation_failed");
    assert_eq!(
        reply.body["errors"],
        json!([{"field": "overview", "code": "overview_invalid"}])
    );

    let mut deep = json!({"type": "text", "text": "x"});
    for _ in 0..40 {
        deep = json!({"type": "blockquote", "content": [deep]});
    }
    let reply = h
        .put_profile(&json!({"overview": {"type": "doc", "content": [deep]}, "hero_asset_id": null, "updated_at": updated_at}))
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        reply.body["errors"],
        json!([{"field": "overview", "code": "overview_too_large"}])
    );
}

#[sqlx::test]
async fn null_overview_clears_it(pool: PgPool) {
    let h = Harness::new(pool).await;
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let put = h
        .put_profile(
            &json!({"overview": doc("A"), "hero_asset_id": null, "updated_at": updated_at}),
        )
        .await;
    let cleared = h
        .put_profile(
            &json!({"overview": null, "hero_asset_id": null, "updated_at": put.body["updated_at"]}),
        )
        .await;
    assert_eq!(cleared.status, StatusCode::OK, "{}", cleared.body);
    assert_eq!(cleared.body["overview"], Value::Null);
}

#[sqlx::test]
async fn profile_routes_need_a_session_and_csrf(pool: PgPool) {
    let h = Harness::new(pool).await;
    let anonymous = h
        .send(
            Request::builder()
                .uri("/api/admin/village/profile")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    let no_csrf = h
        .send(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/admin/village/profile")
                .header(header::COOKIE, &h.cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_eq!(no_csrf.status, StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn public_village_returns_rendered_html(pool: PgPool) {
    let h = Harness::new(pool).await;
    let before = h.public_village().await;
    assert_eq!(before.status, StatusCode::OK);
    assert_eq!(before.body["overview_html"], Value::Null);
    assert_eq!(before.body["hero"], Value::Null);

    let hero = h.asset("image", "ready").await;
    let inline = h.asset("image", "ready").await;
    let overview = json!({"type": "doc", "content": [
        {"type": "paragraph", "content": [{"type": "text", "text": "Desa <kami>"}]},
        {"type": "image", "attrs": {"assetId": inline.to_string(), "alt": "Balai"}},
    ]});
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let put = h
        .put_profile(
            &json!({"overview": overview, "hero_asset_id": hero, "updated_at": updated_at}),
        )
        .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);

    let public = h.public_village().await;
    assert_eq!(public.status, StatusCode::OK);
    let html = public.body["overview_html"].as_str().unwrap();
    assert!(html.contains("<p>Desa &lt;kami&gt;</p>"), "{html}");
    assert!(
        html.contains(&format!("/media/ab/{inline}-480.webp")),
        "{html}"
    );
    assert!(html.contains("alt=\"Balai\""), "{html}");
    assert_eq!(public.body["hero"]["id"], hero.to_string());
    assert_eq!(
        public.body["hero"]["variants"][0]["url"],
        format!("/media/ab/{hero}-480.webp")
    );
    assert_eq!(public.body["name"], "Sorek II");
}

/// A one-shot web server: answers one request with `status` and reports the
/// raw request text.
async fn fake_web(status: &str) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let status = status.to_owned();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0_u8; 4096];
        let n = socket.read(&mut buf).await.unwrap();
        socket
            .write_all(
                format!("HTTP/1.1 {status}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        String::from_utf8_lossy(&buf[..n]).into_owned()
    });
    (url, task)
}

/// Queue a purge job directly, as the save does, without the inline purge.
async fn enqueue_purge(h: &Harness) {
    let mut conn = h.pool.acquire().await.unwrap();
    jobs::enqueue(
        &mut conn,
        jobs::Job::PurgePublicCache,
        time::OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
}

#[sqlx::test]
async fn profile_save_purges_before_responding(pool: PgPool) {
    let (url, request) = fake_web("204 No Content").await;
    let h = Harness::with_state(pool.clone(), test_state_with_web(pool, &url)).await;
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let put = h
        .put_profile(
            &json!({"overview": doc("A"), "hero_asset_id": null, "updated_at": updated_at}),
        )
        .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);

    // The stand-in web server has the purge by the time the PUT has returned
    // (no worker was run), and the redundant job is marked done.
    let request = tokio::time::timeout(std::time::Duration::from_secs(1), request)
        .await
        .expect("the purge was not sent before the answer")
        .unwrap()
        .to_lowercase();
    assert!(
        request.starts_with("post /_internal/cache/purge "),
        "{request}"
    );
    let done = sqlx::query_scalar!(
        r#"SELECT done_at IS NOT NULL AS "done!" FROM jobs WHERE kind = 'purge_public_cache'"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert!(done);
}

#[sqlx::test]
async fn profile_save_only_completes_its_own_purge_job(pool: PgPool) {
    let (url, _request) = fake_web("204 No Content").await;
    let h = Harness::with_state(pool.clone(), test_state_with_web(pool, &url)).await;
    // Another save's purge is still waiting for its retry.
    enqueue_purge(&h).await;
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let put = h
        .put_profile(
            &json!({"overview": doc("A"), "hero_asset_id": null, "updated_at": updated_at}),
        )
        .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    let pending = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM jobs WHERE kind = 'purge_public_cache' AND done_at IS NULL"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(pending, 1, "the other job must stay pending");
}

#[sqlx::test]
async fn profile_save_gives_up_on_a_silent_web_server_after_two_seconds(pool: PgPool) {
    // Accepts the connection and never answers.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let _held = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        drop(socket);
    });
    let h = Harness::with_state(pool.clone(), test_state_with_web(pool, &url)).await;
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let started = std::time::Instant::now();
    let put = h
        .put_profile(
            &json!({"overview": doc("A"), "hero_asset_id": null, "updated_at": updated_at}),
        )
        .await;
    let took = started.elapsed();
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    assert!(
        took >= std::time::Duration::from_secs(2) && took < std::time::Duration::from_secs(4),
        "{took:?}"
    );
    let pending = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM jobs WHERE kind = 'purge_public_cache' AND done_at IS NULL"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(pending, 1);
}

#[sqlx::test]
async fn profile_save_keeps_the_purge_job_when_the_inline_purge_fails(pool: PgPool) {
    let (url, _request) = fake_web("503 Service Unavailable").await;
    let h = Harness::with_state(pool.clone(), test_state_with_web(pool, &url)).await;
    let updated_at = h.get_profile().await.body["updated_at"].clone();
    let put = h
        .put_profile(
            &json!({"overview": doc("A"), "hero_asset_id": null, "updated_at": updated_at}),
        )
        .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    let pending = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM jobs WHERE kind = 'purge_public_cache' AND done_at IS NULL"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(pending, 1);
}

#[sqlx::test]
async fn purge_job_posts_the_token_and_completes(pool: PgPool) {
    let (url, request) = fake_web("204 No Content").await;
    let h = Harness::with_state(pool.clone(), test_state_with_web(pool, &url)).await;
    enqueue_purge(&h).await;
    assert!(jobs::claim_and_run(&h.state).await.unwrap());

    let request = request.await.unwrap().to_lowercase();
    assert!(
        request.starts_with("post /_internal/cache/purge "),
        "{request}"
    );
    assert!(
        request.contains("x-pranata-purge-token: test-purge-token"),
        "{request}"
    );
    let done = sqlx::query_scalar!(
        r#"SELECT done_at IS NOT NULL AS "done!" FROM jobs WHERE kind = 'purge_public_cache'"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert!(done);
}

#[sqlx::test]
async fn purge_job_retries_when_web_is_down(pool: PgPool) {
    // Bind then drop, so the port is closed.
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let h = Harness::with_state(pool.clone(), test_state_with_web(pool, &url)).await;
    enqueue_purge(&h).await;
    assert!(jobs::claim_and_run(&h.state).await.unwrap());

    let job = sqlx::query!(
        r#"SELECT attempts, done_at, failed_at, run_at > now() AS "later!", last_error
           FROM jobs WHERE kind = 'purge_public_cache'"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(job.attempts, 1);
    assert!(job.done_at.is_none() && job.failed_at.is_none());
    assert!(job.later);
    assert!(!job.last_error.unwrap().contains("test-purge-token"));
}

#[sqlx::test]
async fn purge_job_retries_on_a_non_2xx_answer(pool: PgPool) {
    let (url, _request) = fake_web("503 Service Unavailable").await;
    let h = Harness::with_state(pool.clone(), test_state_with_web(pool, &url)).await;
    enqueue_purge(&h).await;
    assert!(jobs::claim_and_run(&h.state).await.unwrap());
    let job = sqlx::query!(
        r#"SELECT attempts, done_at, run_at > now() AS "later!" FROM jobs WHERE kind = 'purge_public_cache'"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(job.attempts, 1);
    assert!(job.done_at.is_none() && job.later);
}
