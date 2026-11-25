mod common;

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use common::{FixedClock, test_state_with_clock};
use http_body_util::BodyExt;
use pranata::{
    auth,
    ids::{RoleId, UserId},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use tower::ServiceExt;
use url::Url;
use webauthn_authenticator_rs::{
    AuthenticatorBackend, WebauthnAuthenticator, error::WebauthnCError, softpasskey::SoftPasskey,
};
use webauthn_rs_proto::{
    AllowCredentials, CreationChallengeResponse, PublicKeyCredential,
    PublicKeyCredentialCreationOptions, PublicKeyCredentialRequestOptions,
    RegisterPublicKeyCredential, RequestChallengeResponse,
};

const PASSWORD: &str = "correct horse battery";
const ORIGIN: &str = "http://localhost:3000";

/// A software authenticator that keeps discoverable credentials.
///
/// `SoftPasskey` keeps every key it creates, but it refuses a request for a
/// resident key and never returns a user handle. This wrapper does what a
/// platform authenticator does with a discoverable credential: it accepts
/// the request, remembers the user handle, offers its credentials when the
/// Relying Party names none (username-less sign-in), and returns the user
/// handle with the assertion. The signature is still `SoftPasskey`'s own; the
/// user handle is not signed in WebAuthn either.
struct Authenticator {
    inner: SoftPasskey,
    user_handles: HashMap<Vec<u8>, Vec<u8>>,
}

impl Authenticator {
    fn new() -> Self {
        Self {
            inner: SoftPasskey::new(true),
            user_handles: HashMap::new(),
        }
    }

    fn origin() -> Url {
        Url::parse(ORIGIN).unwrap()
    }

    /// Answer registration options as a browser would.
    fn create(&mut self, options: &Value) -> RegisterPublicKeyCredential {
        let options: CreationChallengeResponse = serde_json::from_value(options.clone()).unwrap();
        self.do_registration(Self::origin(), options).unwrap()
    }

    /// Answer authentication options as a browser would.
    fn get(&mut self, options: &Value) -> PublicKeyCredential {
        let options: RequestChallengeResponse = serde_json::from_value(options.clone()).unwrap();
        self.do_authentication(Self::origin(), options).unwrap()
    }
}

impl AuthenticatorBackend for Authenticator {
    fn perform_register(
        &mut self,
        origin: Url,
        mut options: PublicKeyCredentialCreationOptions,
        timeout_ms: u32,
    ) -> Result<RegisterPublicKeyCredential, WebauthnCError> {
        if let Some(selection) = options.authenticator_selection.as_mut() {
            selection.require_resident_key = false;
        }
        let user_handle = options.user.id.clone();
        let credential =
            AuthenticatorBackend::perform_register(&mut self.inner, origin, options, timeout_ms)?;
        self.user_handles
            .insert(credential.raw_id.clone(), user_handle);
        Ok(credential)
    }

    fn perform_auth(
        &mut self,
        origin: Url,
        mut options: PublicKeyCredentialRequestOptions,
        timeout_ms: u32,
    ) -> Result<PublicKeyCredential, WebauthnCError> {
        if options.allow_credentials.is_empty() {
            options.allow_credentials = self
                .user_handles
                .keys()
                .map(|id| AllowCredentials {
                    type_: "public-key".to_owned(),
                    id: id.clone(),
                    transports: None,
                })
                .collect();
        }
        let mut credential =
            AuthenticatorBackend::perform_auth(&mut self.inner, origin, options, timeout_ms)?;
        credential.response.user_handle = self.user_handles.get(&credential.raw_id).cloned();
        Ok(credential)
    }
}

struct Harness {
    pool: PgPool,
    clock: Arc<FixedClock>,
    app: Router,
    admin_role: RoleId,
    next_ip: AtomicU32,
}

#[derive(Debug)]
struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

impl Reply {
    fn cookie(&self) -> Option<String> {
        self.headers.get(header::SET_COOKIE).map(|value| {
            value
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned()
        })
    }
}

/// A signed-in browser: the session cookie and its CSRF token.
struct Session {
    cookie: String,
    csrf: String,
}

impl Harness {
    async fn new(pool: PgPool) -> Self {
        let clock = FixedClock::new();
        let app = pranata::app(test_state_with_clock(pool.clone(), clock.clone()));
        let admin_role = common::administrator_role(&pool).await;
        Self {
            pool,
            clock,
            app,
            admin_role,
            next_ip: AtomicU32::new(0),
        }
    }

    /// A client address nobody used before, so the per-IP limit never
    /// interferes.
    fn fresh_ip(&self) -> String {
        let n = self.next_ip.fetch_add(1, Ordering::Relaxed);
        format!("10.0.{}.{}", (n >> 8) & 255, n & 255)
    }

    async fn role(&self, name: &str, permissions: &[&str]) -> RoleId {
        let id = sqlx::query_scalar!(
            r#"INSERT INTO roles (name) VALUES ($1) RETURNING id AS "id: RoleId""#,
            name
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        for permission in permissions {
            sqlx::query!(
                "INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2)",
                id.0,
                permission
            )
            .execute(&self.pool)
            .await
            .unwrap();
        }
        id
    }

    /// A User with a password, so they can sign in.
    async fn user(&self, username: &str, role: RoleId) -> UserId {
        let hash = auth::password::hash_password(PASSWORD).unwrap();
        sqlx::query_scalar!(
            r#"INSERT INTO users (username, display_name, password_hash, role_id)
               VALUES ($1, $1, $2, $3) RETURNING id AS "id: UserId""#,
            username,
            hash,
            role.0
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// A User without a password, with a session made the way sign-in makes
    /// one (so in Sudo Mode).
    async fn passwordless(&self, username: &str, role: RoleId) -> (UserId, Session) {
        let id = sqlx::query_scalar!(
            r#"INSERT INTO users (username, display_name, role_id)
               VALUES ($1, $1, $2) RETURNING id AS "id: UserId""#,
            username,
            role.0
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let mut conn = self.pool.acquire().await.unwrap();
        let new = auth::session::create(&mut conn, id, false, self.clock.now_for_test())
            .await
            .unwrap();
        let session = Session {
            cookie: format!("pranata_session={}", new.token),
            csrf: new.csrf_token,
        };
        (id, session)
    }

    async fn admin(&self, username: &str) -> (UserId, Session) {
        let id = self.user(username, self.admin_role).await;
        (id, self.login(username).await)
    }

    async fn request(
        &self,
        method: Method,
        uri: &str,
        session: Option<&Session>,
        csrf: bool,
        body: Option<Value>,
    ) -> Reply {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("x-forwarded-for", self.fresh_ip());
        if let Some(session) = session {
            builder = builder.header(header::COOKIE, &session.cookie);
            if csrf {
                builder = builder.header("x-csrf-token", &session.csrf);
            }
        }
        let request = match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => builder.body(Body::empty()),
        }
        .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        Reply {
            status: parts.status,
            headers: parts.headers,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        }
    }

    async fn call(
        &self,
        method: Method,
        uri: &str,
        session: &Session,
        body: Option<Value>,
    ) -> Reply {
        self.request(method, uri, Some(session), true, body).await
    }

    async fn try_login(&self, username: &str, password: &str) -> Reply {
        self.request(
            Method::POST,
            "/api/auth/login",
            None,
            false,
            Some(json!({"username": username, "password": password})),
        )
        .await
    }

    async fn login(&self, username: &str) -> Session {
        let reply = self.try_login(username, PASSWORD).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        Session {
            cookie: reply.cookie().unwrap(),
            csrf: reply.body["csrf_token"].as_str().unwrap().to_owned(),
        }
    }

    async fn me(&self, cookie: &str) -> Reply {
        let session = Session {
            cookie: cookie.to_owned(),
            csrf: String::new(),
        };
        self.request(Method::GET, "/api/auth/me", Some(&session), false, None)
            .await
    }

    async fn start_registration(&self, session: &Session) -> Reply {
        self.call(
            Method::POST,
            "/api/auth/passkeys/registration/start",
            session,
            None,
        )
        .await
    }

    async fn finish_registration(
        &self,
        session: &Session,
        challenge_id: &Value,
        name: &str,
        credential: &RegisterPublicKeyCredential,
    ) -> Reply {
        self.call(
            Method::POST,
            "/api/auth/passkeys/registration/finish",
            session,
            Some(json!({"challenge_id": challenge_id, "name": name, "credential": credential})),
        )
        .await
    }

    /// Register a passkey on `key` and return the finish reply.
    async fn register(&self, session: &Session, key: &mut Authenticator, name: &str) -> Reply {
        let start = self.start_registration(session).await;
        assert_eq!(start.status, StatusCode::OK, "{}", start.body);
        let credential = key.create(&start.body["options"]);
        self.finish_registration(session, &start.body["challenge_id"], name, &credential)
            .await
    }

    /// Register a passkey and return its id.
    async fn add_passkey(&self, session: &Session, key: &mut Authenticator) -> String {
        let reply = self.register(session, key, "Laptop kantor").await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
        reply.body["id"].as_str().unwrap().to_owned()
    }

    async fn start_sign_in(&self) -> Reply {
        let reply = self
            .request(
                Method::POST,
                "/api/auth/passkey-login/start",
                None,
                false,
                None,
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        reply
    }

    async fn finish_sign_in(
        &self,
        challenge_id: &Value,
        credential: &PublicKeyCredential,
    ) -> Reply {
        self.request(
            Method::POST,
            "/api/auth/passkey-login/finish",
            None,
            false,
            Some(
                json!({"challenge_id": challenge_id, "credential": credential, "remember": false}),
            ),
        )
        .await
    }

    async fn passkey_sign_in(&self, key: &mut Authenticator) -> Reply {
        let start = self.start_sign_in().await;
        let credential = key.get(&start.body["options"]);
        self.finish_sign_in(&start.body["challenge_id"], &credential)
            .await
    }

    async fn passkey_sudo(&self, session: &Session, key: &mut Authenticator) -> Reply {
        let start = self
            .call(Method::POST, "/api/auth/sudo/passkey/start", session, None)
            .await;
        assert_eq!(start.status, StatusCode::OK, "{}", start.body);
        let credential = key.get(&start.body["options"]);
        self.call(
            Method::POST,
            "/api/auth/sudo/passkey/finish",
            session,
            Some(json!({"challenge_id": start.body["challenge_id"], "credential": credential})),
        )
        .await
    }

    async fn challenge_ids(&self) -> Vec<uuid::Uuid> {
        sqlx::query_scalar!("SELECT id FROM webauthn_challenges ORDER BY id")
            .fetch_all(&self.pool)
            .await
            .unwrap()
    }

    async fn audit_count(&self, action: &str, object_type: &str) -> i64 {
        sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM audit_entries
               WHERE action = $1 AND object_type = $2"#,
            action,
            object_type
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn create_user(&self, session: &Session, username: &str) -> Reply {
        self.call(
            Method::POST,
            "/api/admin/users",
            session,
            Some(
                json!({"username": username, "display_name": "Siti", "role_id": self.admin_role.0}),
            ),
        )
        .await
    }
}

fn assert_problem(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert_eq!(reply.body["code"], code, "{}", reply.body);
}

fn instant(value: &Value) -> OffsetDateTime {
    OffsetDateTime::parse(value.as_str().unwrap(), &Rfc3339).unwrap()
}

#[sqlx::test]
async fn register_then_sign_in_without_username(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (budi, session) = h.admin("budi").await;
    let mut key = Authenticator::new();

    let registered = h.register(&session, &mut key, "  Laptop kantor  ").await;
    assert_eq!(
        registered.status,
        StatusCode::CREATED,
        "{}",
        registered.body
    );
    assert_eq!(registered.body["name"], "Laptop kantor");
    assert_eq!(
        instant(&registered.body["created_at"]),
        h.clock.now_for_test()
    );
    assert_eq!(registered.body["last_used_at"], Value::Null);
    let passkey_id = registered.body["id"].as_str().unwrap().to_owned();
    let listed = h
        .call(Method::GET, "/api/auth/passkeys", &session, None)
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.body);
    assert_eq!(listed.body, json!([registered.body]));

    let out = h
        .call(Method::POST, "/api/auth/logout", &session, None)
        .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    h.clock.advance(Duration::minutes(1));

    let signed = h.passkey_sign_in(&mut key).await;
    assert_eq!(signed.status, StatusCode::OK, "{}", signed.body);
    assert_eq!(signed.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(signed.body["user"]["username"], "budi");
    assert!(signed.body["csrf_token"].is_string());
    let cookie = signed.cookie().unwrap();
    let me = h.me(&cookie).await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.body);
    assert_eq!(me.body["user"]["username"], "budi");

    let used = sqlx::query_scalar!(
        "SELECT last_used_at FROM passkeys WHERE id = $1",
        passkey_id.parse::<uuid::Uuid>().unwrap()
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(used, Some(h.clock.now_for_test()));

    let created = sqlx::query!(
        "SELECT actor, object_id FROM audit_entries WHERE action = 'create' AND object_type = 'passkey'"
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(created.actor, Some(budi.0));
    assert_eq!(created.object_id, passkey_id);
    let sign_in = sqlx::query!(
        r#"SELECT actor, object_type, summary FROM audit_entries
           WHERE action = 'sign_in' AND summary LIKE '%passkey'"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(sign_in.actor, Some(budi.0));
    assert_eq!(sign_in.object_type, "user");
    assert_eq!(sign_in.summary, "budi signed in with a passkey");
}

#[sqlx::test]
async fn registration_requires_sudo_and_csrf(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("budi").await;

    let unprotected = h
        .request(
            Method::POST,
            "/api/auth/passkeys/registration/start",
            Some(&session),
            false,
            None,
        )
        .await;
    assert_problem(&unprotected, StatusCode::FORBIDDEN, "csrf_mismatch");

    h.clock.advance(Duration::minutes(11));
    let start = h.start_registration(&session).await;
    assert_problem(&start, StatusCode::FORBIDDEN, "sudo_required");
}

#[sqlx::test]
async fn registration_options_require_a_resident_key_and_exclude_known_credentials(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("budi").await;
    let mut key = Authenticator::new();

    let first = h.start_registration(&session).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    assert_eq!(first.headers[header::CACHE_CONTROL], "no-store");
    let credential = key.create(&first.body["options"]);
    let finished = h
        .finish_registration(&session, &first.body["challenge_id"], "Ponsel", &credential)
        .await;
    assert_eq!(finished.status, StatusCode::CREATED, "{}", finished.body);

    let second = h.start_registration(&session).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.body);
    let options = &second.body["options"]["publicKey"];
    assert_eq!(
        options["authenticatorSelection"]["residentKey"], "required",
        "{options}"
    );
    assert_eq!(
        options["authenticatorSelection"]["requireResidentKey"], true,
        "{options}"
    );
    let excluded: Vec<&str> = options["excludeCredentials"]
        .as_array()
        .unwrap()
        .iter()
        .map(|descriptor| descriptor["id"].as_str().unwrap())
        .collect();
    assert_eq!(excluded, [credential.id.as_str()]);
}

#[sqlx::test]
async fn registration_validates_the_name_and_refuses_a_known_credential(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("budi").await;
    let other = h.user("siti", h.admin_role).await;
    let mut key = Authenticator::new();

    let start = h.start_registration(&session).await;
    let credential = key.create(&start.body["options"]);
    for name in ["   ", &"x".repeat(51)] {
        let reply = h
            .finish_registration(&session, &start.body["challenge_id"], name, &credential)
            .await;
        assert_problem(
            &reply,
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_failed",
        );
        assert_eq!(
            reply.body["errors"],
            json!([{"field": "name", "code": "name_length"}])
        );
    }

    // Somebody else holds this credential id already.
    sqlx::query!(
        "INSERT INTO passkeys (user_id, credential_id, name, passkey, created_at)
         VALUES ($1, $2, 'Lain', '{}', now())",
        other.0,
        credential.raw_id,
    )
    .execute(&h.pool)
    .await
    .unwrap();
    let reply = h
        .finish_registration(&session, &start.body["challenge_id"], "Ponsel", &credential)
        .await;
    assert_problem(&reply, StatusCode::CONFLICT, "passkey_exists");
    assert_eq!(h.audit_count("create", "passkey").await, 0);
}

#[sqlx::test]
async fn challenges_are_bound_to_kind_and_user(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, andi) = h.admin("andi").await;
    let (_, budi) = h.admin("budi").await;
    let mut key = Authenticator::new();

    // A sign-in challenge at the registration finish.
    let sign_in = h.start_sign_in().await;
    let registration = h.start_registration(&andi).await;
    let credential = key.create(&registration.body["options"]);
    let reply = h
        .finish_registration(&andi, &sign_in.body["challenge_id"], "Ponsel", &credential)
        .await;
    assert_problem(&reply, StatusCode::GONE, "passkey_challenge_expired");
    let sign_in_id: uuid::Uuid = sign_in.body["challenge_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(h.challenge_ids().await.contains(&sign_in_id));

    // Andi's registration challenge, finished by Budi.
    let reply = h
        .finish_registration(
            &budi,
            &registration.body["challenge_id"],
            "Ponsel",
            &credential,
        )
        .await;
    assert_problem(&reply, StatusCode::GONE, "passkey_challenge_expired");
    assert_eq!(h.challenge_ids().await.len(), 2);

    // Andi can still finish it.
    let reply = h
        .finish_registration(
            &andi,
            &registration.body["challenge_id"],
            "Ponsel",
            &credential,
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);

    // An unknown challenge id.
    let reply = h
        .finish_registration(&andi, &json!(uuid::Uuid::nil()), "Ponsel", &credential)
        .await;
    assert_problem(&reply, StatusCode::GONE, "passkey_challenge_expired");
}

#[sqlx::test]
async fn expired_challenges_are_pruned_on_insert(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.start_sign_in().await;
    h.clock.advance(Duration::minutes(6));
    let second = h.start_sign_in().await;
    let id: uuid::Uuid = second.body["challenge_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(h.challenge_ids().await, [id]);
}

#[sqlx::test]
async fn challenge_is_single_use_and_expires(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("budi").await;
    let mut key = Authenticator::new();

    let start = h.start_registration(&session).await;
    let credential = key.create(&start.body["options"]);
    let first = h
        .finish_registration(&session, &start.body["challenge_id"], "Ponsel", &credential)
        .await;
    assert_eq!(first.status, StatusCode::CREATED, "{}", first.body);
    let again = h
        .finish_registration(&session, &start.body["challenge_id"], "Ponsel", &credential)
        .await;
    assert_problem(&again, StatusCode::GONE, "passkey_challenge_expired");

    // A replayed sign-in assertion.
    let sign_in = h.start_sign_in().await;
    let assertion = key.get(&sign_in.body["options"]);
    let first = h
        .finish_sign_in(&sign_in.body["challenge_id"], &assertion)
        .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    let again = h
        .finish_sign_in(&sign_in.body["challenge_id"], &assertion)
        .await;
    assert_problem(&again, StatusCode::GONE, "passkey_challenge_expired");

    // Five minutes after the start, the challenge is gone.
    let start = h.start_registration(&session).await;
    let credential = key.create(&start.body["options"]);
    h.clock.advance(Duration::minutes(5));
    let late = h
        .finish_registration(&session, &start.body["challenge_id"], "Ponsel", &credential)
        .await;
    assert_problem(&late, StatusCode::GONE, "passkey_challenge_expired");
}

#[sqlx::test]
async fn inactive_user_cannot_use_a_passkey(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (budi, session) = h.admin("budi").await;
    let mut key = Authenticator::new();
    h.add_passkey(&session, &mut key).await;

    sqlx::query!("UPDATE users SET is_active = false WHERE id = $1", budi.0)
        .execute(&h.pool)
        .await
        .unwrap();
    let reply = h.passkey_sign_in(&mut key).await;
    assert_problem(&reply, StatusCode::UNAUTHORIZED, "passkey_failed");
    assert!(reply.cookie().is_none());
}

#[sqlx::test]
async fn unknown_credential_cannot_sign_in(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("budi").await;
    let mut key = Authenticator::new();
    let passkey = h.add_passkey(&session, &mut key).await;
    sqlx::query!(
        "DELETE FROM passkeys WHERE id = $1",
        passkey.parse::<uuid::Uuid>().unwrap()
    )
    .execute(&h.pool)
    .await
    .unwrap();

    let reply = h.passkey_sign_in(&mut key).await;
    assert_problem(&reply, StatusCode::UNAUTHORIZED, "passkey_failed");
}

#[sqlx::test]
async fn passkey_sign_in_ignores_password_throttle(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("budi").await;
    let mut key = Authenticator::new();
    h.add_passkey(&session, &mut key).await;

    for _ in 0..10 {
        let reply = h.try_login("budi", "wrong password").await;
        assert_problem(&reply, StatusCode::UNAUTHORIZED, "invalid_credentials");
    }
    let locked = h.try_login("budi", PASSWORD).await;
    assert_problem(&locked, StatusCode::TOO_MANY_REQUESTS, "too_many_attempts");

    let reply = h.passkey_sign_in(&mut key).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
}

#[sqlx::test]
async fn passkey_reopens_sudo(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (budi, session) = h.admin("budi").await;
    let mut key = Authenticator::new();
    let passkey = h.add_passkey(&session, &mut key).await;
    h.clock.advance(Duration::minutes(11));

    let refused = h.create_user(&session, "siti").await;
    assert_problem(&refused, StatusCode::FORBIDDEN, "sudo_required");

    let reply = h.passkey_sudo(&session, &mut key).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT, "{}", reply.body);
    let created = h.create_user(&session, "siti").await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let me = h.me(&session.cookie).await;
    assert_eq!(
        instant(&me.body["sudo_until"]),
        h.clock.now_for_test() + Duration::minutes(10)
    );

    let entry =
        sqlx::query!("SELECT actor, object_id FROM audit_entries WHERE action = 'reauthenticate'")
            .fetch_one(&h.pool)
            .await
            .unwrap();
    assert_eq!(entry.actor, Some(budi.0));
    assert_eq!(entry.object_id, budi.0.to_string());
    let used = sqlx::query_scalar!(
        "SELECT last_used_at FROM passkeys WHERE id = $1",
        passkey.parse::<uuid::Uuid>().unwrap()
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(used, Some(h.clock.now_for_test()));
}

#[sqlx::test]
async fn sudo_accepts_only_the_signed_in_users_passkeys(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, andi) = h.admin("andi").await;
    let (_, budi) = h.admin("budi").await;
    let mut andi_key = Authenticator::new();
    let mut budi_key = Authenticator::new();
    h.add_passkey(&andi, &mut andi_key).await;
    h.add_passkey(&budi, &mut budi_key).await;
    h.clock.advance(Duration::minutes(11));

    // Budi answers Andi's Sudo challenge with Budi's passkey.
    let start = h
        .call(Method::POST, "/api/auth/sudo/passkey/start", &andi, None)
        .await;
    assert_eq!(start.status, StatusCode::OK, "{}", start.body);
    let mut options = start.body["options"].clone();
    options["publicKey"]["allowCredentials"] = json!([]);
    let credential = budi_key.get(&options);
    let reply = h
        .call(
            Method::POST,
            "/api/auth/sudo/passkey/finish",
            &andi,
            Some(json!({"challenge_id": start.body["challenge_id"], "credential": credential})),
        )
        .await;
    assert_problem(&reply, StatusCode::UNAUTHORIZED, "passkey_failed");
    assert_eq!(h.me(&andi.cookie).await.body["sudo_until"], Value::Null);

    // A Sudo challenge is not a sign-in challenge.
    let start = h
        .call(Method::POST, "/api/auth/sudo/passkey/start", &budi, None)
        .await;
    let credential = budi_key.get(&start.body["options"]);
    let reply = h
        .finish_sign_in(&start.body["challenge_id"], &credential)
        .await;
    assert_problem(&reply, StatusCode::GONE, "passkey_challenge_expired");
}

#[sqlx::test]
async fn sudo_with_a_passkey_needs_one(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("budi").await;
    let start = h
        .call(Method::POST, "/api/auth/sudo/passkey/start", &session, None)
        .await;
    assert_problem(&start, StatusCode::NOT_FOUND, "passkey_not_found");
    let unprotected = h
        .request(
            Method::POST,
            "/api/auth/sudo/passkey/start",
            Some(&session),
            false,
            None,
        )
        .await;
    assert_problem(&unprotected, StatusCode::FORBIDDEN, "csrf_mismatch");
}

#[sqlx::test]
async fn only_the_owner_deletes_a_passkey(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (andi, andi_session) = h.admin("andi").await;
    let (_, budi_session) = h.admin("budi").await;
    let mut key = Authenticator::new();
    let passkey = h.add_passkey(&andi_session, &mut key).await;

    let listed = h
        .call(Method::GET, "/api/auth/passkeys", &budi_session, None)
        .await;
    assert_eq!(listed.body, json!([]));
    let reply = h
        .call(
            Method::DELETE,
            &format!("/api/auth/passkeys/{passkey}"),
            &budi_session,
            None,
        )
        .await;
    assert_problem(&reply, StatusCode::NOT_FOUND, "passkey_not_found");
    let reply = h
        .call(
            Method::DELETE,
            "/api/auth/passkeys/not-an-id",
            &budi_session,
            None,
        )
        .await;
    assert_problem(&reply, StatusCode::NOT_FOUND, "passkey_not_found");
    assert_eq!(h.passkey_sign_in(&mut key).await.status, StatusCode::OK);

    let reply = h
        .call(
            Method::DELETE,
            &format!("/api/auth/passkeys/{passkey}"),
            &andi_session,
            None,
        )
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT, "{}", reply.body);
    let entry = sqlx::query!(
        "SELECT actor, object_id FROM audit_entries WHERE action = 'delete' AND object_type = 'passkey'"
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(entry.actor, Some(andi.0));
    assert_eq!(entry.object_id, passkey);
    let reply = h.passkey_sign_in(&mut key).await;
    assert_problem(&reply, StatusCode::UNAUTHORIZED, "passkey_failed");
}

#[sqlx::test]
async fn administrator_removes_a_users_passkeys(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (admin, admin_session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let budi = h.user("budi", writer).await;
    let budi_session = h.login("budi").await;
    let mut first = Authenticator::new();
    let mut second = Authenticator::new();
    h.add_passkey(&budi_session, &mut first).await;
    h.add_passkey(&budi_session, &mut second).await;

    let reply = h
        .call(
            Method::DELETE,
            &format!("/api/admin/users/{}/passkeys", budi.0),
            &admin_session,
            None,
        )
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT, "{}", reply.body);
    for key in [&mut first, &mut second] {
        let reply = h.passkey_sign_in(key).await;
        assert_problem(&reply, StatusCode::UNAUTHORIZED, "passkey_failed");
    }
    let entries = sqlx::query!(
        "SELECT actor, object_id, summary FROM audit_entries WHERE action = 'delete' AND object_type = 'passkey'"
    )
    .fetch_all(&h.pool)
    .await
    .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].actor, Some(admin.0));
    assert_eq!(entries[0].object_id, budi.0.to_string());
    assert_eq!(entries[0].summary, "Removed all passkeys of budi");

    let managers = h.role("Pengelola", &["user.manage"]).await;
    h.user("manajer", managers).await;
    let manager = h.login("manajer").await;
    let reply = h
        .call(
            Method::DELETE,
            &format!("/api/admin/users/{}/passkeys", admin.0),
            &manager,
            None,
        )
        .await;
    assert_problem(&reply, StatusCode::FORBIDDEN, "administrator_only");
    let reply = h
        .call(
            Method::DELETE,
            &format!("/api/admin/users/{}/passkeys", budi.0),
            &budi_session,
            None,
        )
        .await;
    assert_problem(&reply, StatusCode::FORBIDDEN, "permission_denied");
    let reply = h
        .call(
            Method::DELETE,
            &format!("/api/admin/users/{}/passkeys", uuid::Uuid::nil()),
            &admin_session,
            None,
        )
        .await;
    assert_problem(&reply, StatusCode::NOT_FOUND, "user_not_found");
}

#[sqlx::test]
async fn user_summary_counts_passkeys(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("budi").await;
    let mut key = Authenticator::new();
    h.add_passkey(&session, &mut key).await;

    let listed = h
        .call(Method::GET, "/api/admin/users", &session, None)
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.body);
    assert_eq!(listed.body[0]["username"], "budi");
    assert_eq!(listed.body[0]["passkey_count"], 1);
}

/// Wait until `count` connections to this test's database wait for a lock.
async fn wait_for_lock_waits(pool: &PgPool, count: i64) {
    for _ in 0..500 {
        let waiting = sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM pg_stat_activity
               WHERE datname = current_database() AND wait_event_type = 'Lock'"#
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting >= count {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("fewer than {count} requests waited for a lock");
}

/// Andi and Budi demote each other at the same time while `holder` keeps
/// Dewi's row locked; once both requests wait for it, `holder` commits.
/// Holding the lock makes both requests read their sessions (as
/// Administrators) before either demotion commits.
async fn demote_each_other(
    h: &Harness,
    (a, a_session): &(UserId, Session),
    (b, b_session): &(UserId, Session),
    holder: sqlx::Transaction<'static, sqlx::Postgres>,
) -> (Reply, Reply) {
    let writer = h.role("Penulis", &["content.write"]).await;
    let (a_uri, b_uri) = (
        format!("/api/admin/users/{}", a.0),
        format!("/api/admin/users/{}", b.0),
    );
    let demote = json!({"role_id": writer.0});
    let (first, second, ()) = tokio::join!(
        h.call(Method::PATCH, &b_uri, a_session, Some(demote.clone())),
        h.call(Method::PATCH, &a_uri, b_session, Some(demote.clone())),
        async {
            wait_for_lock_waits(&h.pool, 2).await;
            holder.commit().await.unwrap();
        },
    );
    (first, second)
}

async fn lock_user(h: &Harness, user: UserId) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let mut holder = h.pool.begin().await.unwrap();
    sqlx::query!(
        "SELECT id FROM users WHERE id = $1 FOR NO KEY UPDATE",
        user.0
    )
    .fetch_one(&mut *holder)
    .await
    .unwrap();
    holder
}

#[sqlx::test]
async fn passkey_only_administrator_counts(pool: PgPool) {
    let h = Harness::new(pool).await;
    let andi = h.admin("andi").await;
    let budi = h.admin("budi").await;
    let (dewi, dewi_session) = h.passwordless("dewi", h.admin_role).await;
    let mut key = Authenticator::new();
    h.add_passkey(&dewi_session, &mut key).await;

    let holder = lock_user(&h, dewi).await;
    let (first, second) = demote_each_other(&h, &andi, &budi, holder).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    assert_eq!(second.status, StatusCode::OK, "{}", second.body);
}

#[sqlx::test]
async fn a_passkey_removed_during_a_demotion_no_longer_counts(pool: PgPool) {
    let h = Harness::new(pool).await;
    let andi = h.admin("andi").await;
    let budi = h.admin("budi").await;
    let (dewi, dewi_session) = h.passwordless("dewi", h.admin_role).await;
    let mut key = Authenticator::new();
    h.add_passkey(&dewi_session, &mut key).await;

    // Like every removal, this one locks Dewi first. Both demotions wait for
    // it and must see that Dewi can no longer sign in.
    let mut holder = lock_user(&h, dewi).await;
    sqlx::query!("DELETE FROM passkeys WHERE user_id = $1", dewi.0)
        .execute(&mut *holder)
        .await
        .unwrap();
    let (first, second) = demote_each_other(&h, &andi, &budi, holder).await;
    let mut statuses = [first.status, second.status];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::CONFLICT],
        "{} / {}",
        first.body,
        second.body
    );
}

#[sqlx::test]
async fn the_last_administrator_keeps_their_only_passkey(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (dewi, session) = h.passwordless("dewi", h.admin_role).await;
    let mut key = Authenticator::new();
    let passkey = h.add_passkey(&session, &mut key).await;

    let own = h
        .call(
            Method::DELETE,
            &format!("/api/auth/passkeys/{passkey}"),
            &session,
            None,
        )
        .await;
    assert_problem(&own, StatusCode::CONFLICT, "last_administrator");
    let all = h
        .call(
            Method::DELETE,
            &format!("/api/admin/users/{}/passkeys", dewi.0),
            &session,
            None,
        )
        .await;
    assert_problem(&all, StatusCode::CONFLICT, "last_administrator");
    assert_eq!(h.passkey_sign_in(&mut key).await.status, StatusCode::OK);

    // With a second key, the first one may go.
    let mut spare = Authenticator::new();
    h.add_passkey(&session, &mut spare).await;
    let own = h
        .call(
            Method::DELETE,
            &format!("/api/auth/passkeys/{passkey}"),
            &session,
            None,
        )
        .await;
    assert_eq!(own.status, StatusCode::NO_CONTENT, "{}", own.body);

    // With another Administrator who can sign in, every one may go.
    h.user("andi", h.admin_role).await;
    let all = h
        .call(
            Method::DELETE,
            &format!("/api/admin/users/{}/passkeys", dewi.0),
            &session,
            None,
        )
        .await;
    assert_eq!(all.status, StatusCode::NO_CONTENT, "{}", all.body);
}
