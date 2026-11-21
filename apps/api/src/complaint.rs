//! Complaints: what residents send from the public site, anonymously or with a name and phone,
//! and follow with their Complaint Code; staff answer them with Complaint Responses, then close
//! or reject them, and keep the Complaint Topics.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    ApiError, AppState, Clock,
    audit::{self, Actor},
    auth::{Session, client_ip},
    category::{label, unique_name},
    government::Created,
    home::{required, text},
    order::Order,
    public_form::{Proof, PublicForm},
    role::Permission,
    village::{diff, phone_ok},
};

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_form))
        .routes(routes!(submit))
        .routes(routes!(get_status))
        .routes(routes!(list))
        .routes(routes!(get))
        .routes(routes!(respond))
        .routes(routes!(change_state))
        .routes(routes!(list_topics, create_topic))
        .routes(routes!(update_topic, delete_topic))
        .routes(routes!(order_topics))
}

/// Where a Complaint stands.
#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "complaint_state", rename_all = "snake_case")]
pub enum ComplaintState {
    New,
    InProgress,
    /// Final, after a last Complaint Response.
    Closed,
    /// Final, with a reason.
    Rejected,
}

/// Something a Complaint is filed under, or the Hamlet it is about.
#[derive(Serialize, ToSchema)]
pub struct Choice {
    id: Uuid,
    name: String,
}

/// What the public Complaint form offers.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComplaintForm {
    /// The active Complaint Topics in order.
    topics: Vec<Choice>,
    /// The local name for a Hamlet, such as dusun.
    hamlet_label: String,
    /// The active Hamlets in their manual order.
    hamlets: Vec<Choice>,
}

/// What a resident sends.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewComplaint {
    /// An active Complaint Topic.
    topic_id: Uuid,
    /// 1–2,000 characters.
    text: String,
    /// An active Hamlet.
    hamlet_id: Option<Uuid>,
    /// At most 100 characters.
    name: Option<String>,
    /// At most 20 characters: digits, spaces and `+-()`.
    phone: Option<String>,
    /// The honeypot, which people leave empty.
    website: Option<String>,
    proof: Option<Proof>,
}

/// The code a sender follows a Complaint with.
#[derive(Serialize, ToSchema)]
pub struct ComplaintCode {
    /// Such as `7KQ2-M9XD-4TRB-HC1N`.
    code: String,
}

/// One Complaint Response.
#[derive(Serialize, ToSchema)]
pub struct ComplaintResponse {
    at: DateTime<Utc>,
    text: String,
}

/// A Complaint as its sender sees it; never the name, phone or Hamlet.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComplaintStatus {
    topic: String,
    submitted_at: DateTime<Utc>,
    state: ComplaintState,
    rejection_reason: Option<String>,
    /// Oldest first.
    responses: Vec<ComplaintResponse>,
}

/// 16 random characters of Crockford's base32, 80 bits, in groups of four.
fn new_code() -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).expect("the OS random number generator is unavailable");
    let chars: Vec<char> = bytes
        .iter()
        .map(|byte| char::from(ALPHABET[usize::from(byte % 32)]))
        .collect();
    chars
        .chunks(4)
        .map(|group| group.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// The active Complaint Topics and Hamlets, for the public form.
///
/// # Errors
/// `404` before the Village Code is set.
#[utoipa::path(
    get,
    path = "/complaints/form",
    operation_id = "getComplaintForm",
    tag = "complaints",
    responses(
        (status = OK, description = "What the form offers", body = ComplaintForm),
        (status = NOT_FOUND, description = "The Village Code is not set yet"),
    ),
)]
pub(crate) async fn get_form(State(pool): State<PgPool>) -> Result<Json<ComplaintForm>, ApiError> {
    let hamlet_label = sqlx::query_scalar!("SELECT hamlet_label FROM village")
        .fetch_optional(&pool)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let topics = sqlx::query_as!(
        Choice,
        "SELECT id, name FROM complaint_topics WHERE active ORDER BY sort_order, id"
    )
    .fetch_all(&pool)
    .await?;
    let hamlets = sqlx::query_as!(
        Choice,
        r#"SELECT id, name AS "name!" FROM region_units WHERE kind = 'hamlet' AND active
           ORDER BY sort_order"#
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(ComplaintForm {
        topics,
        hamlet_label,
        hamlets,
    }))
}

/// Sends a Complaint. A filled honeypot, a missing, wrong or spent proof of work, or a sixth
/// sending within the hour from one address is answered the same as a Complaint received, with
/// a code that leads nowhere.
///
/// # Errors
/// `422` for blank or overlong text, a Topic or Hamlet that is unknown or inactive, an
/// overlong name or a malformed phone.
#[utoipa::path(
    post,
    path = "/complaints",
    operation_id = "submitComplaint",
    tag = "complaints",
    request_body = NewComplaint,
    responses(
        (status = CREATED, description = "The Complaint's code", body = ComplaintCode),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed, or the Topic or Hamlet cannot take it"),
    ),
)]
pub(crate) async fn submit(
    State(pool): State<PgPool>,
    State(form): State<Arc<PublicForm>>,
    State(clock): State<Clock>,
    headers: HeaderMap,
    Json(body): Json<NewComplaint>,
) -> Result<(StatusCode, Json<ComplaintCode>), ApiError> {
    let unprocessable = ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY);
    let complaint_text = required(&body.text, 2000)?;
    let name = text(body.name.as_deref(), 100)?;
    let phone = text(body.phone.as_deref(), 20)?;
    if phone.as_deref().is_some_and(|phone| !phone_ok(phone)) {
        return Err(unprocessable);
    }
    let topic_active = sqlx::query_scalar!(
        "SELECT active FROM complaint_topics WHERE id = $1",
        body.topic_id
    )
    .fetch_optional(&pool)
    .await?;
    if topic_active != Some(true) {
        return Err(unprocessable);
    }
    if let Some(hamlet) = body.hamlet_id {
        let active = sqlx::query_scalar!(
            "SELECT active FROM region_units WHERE id = $1 AND kind = 'hamlet'",
            hamlet
        )
        .fetch_optional(&pool)
        .await?;
        if active != Some(true) {
            return Err(unprocessable);
        }
    }

    let code = new_code();
    let admitted = form.admit(
        body.website.as_deref(),
        body.proof.as_ref(),
        client_ip(&headers),
        clock.now(),
    );
    if admitted {
        let mut tx = pool.begin().await?;
        let id = sqlx::query_scalar!(
            "INSERT INTO complaints (code, topic_id, text, hamlet_id, name, phone)
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
            code,
            body.topic_id,
            complaint_text,
            body.hamlet_id,
            name,
            phone,
        )
        .fetch_one(&mut *tx)
        .await?;
        audit::record(
            &mut tx,
            Actor::Anonymous,
            "complaint.submit",
            ("complaint", Some(&id.to_string())),
            json!({}),
        )
        .await?;
        tx.commit().await?;
    }
    Ok((StatusCode::CREATED, Json(ComplaintCode { code })))
}

/// A Complaint's topic, date, state, rejection reason and Responses, read by its code.
///
/// # Errors
/// `404` for an unknown code; `429` without looking, after 20 lookups a minute from the address.
#[utoipa::path(
    get,
    path = "/complaints/status/{code}",
    operation_id = "getComplaintStatus",
    tag = "complaints",
    params(("code" = String, Path, description = "The Complaint Code, in any case")),
    responses(
        (status = OK, description = "The Complaint", body = ComplaintStatus),
        (status = NOT_FOUND, description = "No Complaint has the code"),
        (status = TOO_MANY_REQUESTS, description = "Too many lookups from the address; try again later"),
    ),
)]
pub(crate) async fn get_status(
    State(pool): State<PgPool>,
    State(form): State<Arc<PublicForm>>,
    headers: HeaderMap,
    Path(code): Path<String>,
) -> Result<Json<ComplaintStatus>, ApiError> {
    if !form.admit_lookup(client_ip(&headers)) {
        return Err(ApiError::Status(StatusCode::TOO_MANY_REQUESTS));
    }
    let complaint = sqlx::query!(
        r#"SELECT complaints.id, complaint_topics.name AS topic, created_at,
           state AS "state: ComplaintState", rejection_reason
           FROM complaints JOIN complaint_topics ON complaint_topics.id = topic_id
           WHERE code = $1"#,
        code.trim().to_uppercase()
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let responses = sqlx::query_as!(
        ComplaintResponse,
        r#"SELECT created_at AS at, text FROM complaint_responses WHERE complaint_id = $1
           ORDER BY created_at, id"#,
        complaint.id
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(ComplaintStatus {
        topic: complaint.topic,
        submitted_at: complaint.created_at,
        state: complaint.state,
        rejection_reason: complaint.rejection_reason,
        responses,
    }))
}

/// Refuses with `403` unless the User's Role holds `permission`.
fn ensure(session: &Session, permission: Permission) -> Result<(), ApiError> {
    if session.holds(permission) {
        Ok(())
    } else {
        Err(ApiError::Status(StatusCode::FORBIDDEN))
    }
}

/// How many Complaints are New, for the menu badge and the dashboard; `None` without
/// `complaint.read`.
pub(crate) async fn count_new(pool: &PgPool, session: &Session) -> sqlx::Result<Option<i64>> {
    if !session.holds(Permission::ComplaintRead) {
        return Ok(None);
    }
    sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM complaints WHERE state = 'new'"#)
        .fetch_one(pool)
        .await
        .map(Some)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ComplaintFilter {
    /// Only Complaints in this state.
    #[param(inline)]
    state: Option<ComplaintState>,
}

/// A Complaint in the staff queue.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComplaintSummary {
    id: Uuid,
    topic: String,
    text: String,
    state: ComplaintState,
    submitted_at: DateTime<Utc>,
}

/// One Complaint Response as staff see it.
#[derive(Serialize, ToSchema)]
pub struct StaffComplaintResponse {
    at: DateTime<Utc>,
    text: String,
    /// The username of the User who posted it.
    author: String,
}

/// A Complaint as staff see it, with what the sender left to be reached by.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComplaintDetail {
    id: Uuid,
    code: String,
    topic: String,
    text: String,
    submitted_at: DateTime<Utc>,
    state: ComplaintState,
    rejection_reason: Option<String>,
    /// The name of the sender's Hamlet.
    hamlet: Option<String>,
    name: Option<String>,
    phone: Option<String>,
    /// Oldest first.
    responses: Vec<StaffComplaintResponse>,
    /// Whether the reading User holds `complaint.handle`, to answer, close or reject it.
    can_handle: bool,
}

/// A Complaint Response to post.
#[derive(Deserialize, ToSchema)]
pub struct NewComplaintResponse {
    /// 1–2,000 characters, shown to the sender on the status page.
    text: String,
}

/// Where to move a Complaint.
#[derive(Deserialize, ToSchema)]
pub struct ComplaintStateChange {
    /// `in_progress` from `new`; `closed` or `rejected` from either.
    state: ComplaintState,
    /// For `closed`, the final Complaint Response; for `rejected`, the reason; 1–2,000
    /// characters either way.
    text: Option<String>,
}

/// Every Complaint, or those in one state, the latest first.
///
/// # Errors
/// `401` without a live session, `403` without `complaint.read`.
#[utoipa::path(
    get,
    path = "/complaints",
    operation_id = "listComplaints",
    tag = "complaints",
    params(ComplaintFilter),
    responses(
        (status = OK, description = "The Complaints", body = Vec<ComplaintSummary>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not read Complaints"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    session: Session,
    Query(filter): Query<ComplaintFilter>,
) -> Result<Json<Vec<ComplaintSummary>>, ApiError> {
    ensure(&session, Permission::ComplaintRead)?;
    // shortcut: no paging; a Village gets few Complaints, page once a queue grows past hundreds.
    let complaints = sqlx::query_as!(
        ComplaintSummary,
        r#"SELECT complaints.id, complaint_topics.name AS topic, text,
           state AS "state: ComplaintState", created_at AS submitted_at
           FROM complaints JOIN complaint_topics ON complaint_topics.id = topic_id
           WHERE $1::complaint_state IS NULL OR state = $1
           ORDER BY created_at DESC, complaints.id DESC"#,
        filter.state as Option<ComplaintState>,
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(complaints))
}

/// One Complaint with its sender's Hamlet, name and phone and every Response.
///
/// # Errors
/// `401` without a live session, `403` without `complaint.read`, `404` for an unknown
/// Complaint.
#[utoipa::path(
    get,
    path = "/complaints/{id}",
    operation_id = "getComplaint",
    tag = "complaints",
    params(("id" = Uuid, Path, description = "The Complaint")),
    responses(
        (status = OK, description = "The Complaint", body = ComplaintDetail),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not read Complaints"),
        (status = NOT_FOUND, description = "There is no such Complaint"),
    ),
)]
pub(crate) async fn get(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Json<ComplaintDetail>, ApiError> {
    ensure(&session, Permission::ComplaintRead)?;
    let complaint = sqlx::query!(
        r#"SELECT code, complaint_topics.name AS topic, text, created_at,
           state AS "state: ComplaintState", rejection_reason,
           region_units.name AS "hamlet?", complaints.name, phone
           FROM complaints JOIN complaint_topics ON complaint_topics.id = topic_id
           LEFT JOIN region_units ON region_units.id = hamlet_id
           WHERE complaints.id = $1"#,
        id
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let responses = sqlx::query_as!(
        StaffComplaintResponse,
        r#"SELECT complaint_responses.created_at AS at, complaint_responses.text,
           username AS author
           FROM complaint_responses JOIN users ON users.id = user_id
           WHERE complaint_id = $1 ORDER BY complaint_responses.created_at, complaint_responses.id"#,
        id
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(ComplaintDetail {
        id,
        code: complaint.code,
        topic: complaint.topic,
        text: complaint.text,
        submitted_at: complaint.created_at,
        state: complaint.state,
        rejection_reason: complaint.rejection_reason,
        hamlet: complaint.hamlet,
        name: complaint.name,
        phone: complaint.phone,
        responses,
        can_handle: session.holds(Permission::ComplaintHandle),
    }))
}

/// Locks a Complaint that is still open and returns its state: `404` for an unknown one,
/// `409` once Closed or Rejected.
async fn lock_open(tx: &mut PgConnection, id: Uuid) -> Result<ComplaintState, ApiError> {
    let state = sqlx::query_scalar!(
        r#"SELECT state AS "state: ComplaintState" FROM complaints WHERE id = $1 FOR UPDATE"#,
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    match state {
        ComplaintState::New | ComplaintState::InProgress => Ok(state),
        ComplaintState::Closed | ComplaintState::Rejected => {
            Err(ApiError::Status(StatusCode::CONFLICT))
        }
    }
}

/// Posts a Complaint Response and audits it.
async fn post_response(
    tx: &mut PgConnection,
    session: &Session,
    id: Uuid,
    text: &str,
) -> Result<Uuid, ApiError> {
    let response = sqlx::query_scalar!(
        "INSERT INTO complaint_responses (complaint_id, user_id, text) VALUES ($1, $2, $3)
         RETURNING id",
        id,
        session.user_id,
        text,
    )
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        tx,
        Actor::User(session.user_id),
        "complaint.respond",
        ("complaint", Some(&id.to_string())),
        json!({ "response": response }),
    )
    .await?;
    Ok(response)
}

/// Moves a Complaint and audits it; a rejection takes its reason.
async fn set_state(
    tx: &mut PgConnection,
    session: &Session,
    id: Uuid,
    (from, to): (ComplaintState, ComplaintState),
    rejection_reason: Option<&str>,
) -> Result<(), ApiError> {
    sqlx::query!(
        "UPDATE complaints SET state = $1, rejection_reason = $2 WHERE id = $3",
        to as ComplaintState,
        rejection_reason,
        id,
    )
    .execute(&mut *tx)
    .await?;
    let mut changes = vec![json!({ "field": "state", "from": from, "to": to })];
    if let Some(reason) = rejection_reason {
        changes.push(json!({ "field": "rejectionReason", "from": null, "to": reason }));
    }
    audit::record_change(
        tx,
        Actor::User(session.user_id),
        "complaint.state",
        ("complaint", Some(&id.to_string())),
        json!(changes),
    )
    .await?;
    Ok(())
}

/// Posts a Complaint Response, which the sender sees on the status page and nobody edits; a New
/// Complaint moves to `InProgress` with it.
///
/// # Errors
/// `401` without a live session, `403` without `complaint.handle`, `404` for an unknown
/// Complaint, `409` once it is Closed or Rejected, `422` for a blank or overlong text.
#[utoipa::path(
    post,
    path = "/complaints/{id}/responses",
    operation_id = "respondToComplaint",
    tag = "complaints",
    params(("id" = Uuid, Path, description = "The Complaint")),
    request_body = NewComplaintResponse,
    responses(
        (status = CREATED, description = "The new Complaint Response", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not handle Complaints"),
        (status = NOT_FOUND, description = "There is no such Complaint"),
        (status = CONFLICT, description = "The Complaint is Closed or Rejected"),
        (status = UNPROCESSABLE_ENTITY, description = "The text is blank or too long"),
    ),
)]
pub(crate) async fn respond(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<NewComplaintResponse>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure(&session, Permission::ComplaintHandle)?;
    let text = required(&body.text, 2000)?;
    let mut tx = pool.begin().await?;
    let state = lock_open(&mut tx, id).await?;
    let response = post_response(&mut tx, &session, id, &text).await?;
    if state == ComplaintState::New {
        set_state(
            &mut tx,
            &session,
            id,
            (state, ComplaintState::InProgress),
            None,
        )
        .await?;
    }
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id: response })))
}

/// Moves a Complaint: a New one to `InProgress`; an open one to Closed with a final Complaint
/// Response, or to Rejected with a reason. Closed and Rejected are final.
///
/// # Errors
/// `401` without a live session, `403` without `complaint.handle`, `404` for an unknown
/// Complaint, `409` for a move its state does not allow, `422` for `new`, or a missing, blank
/// or overlong text.
#[utoipa::path(
    put,
    path = "/complaints/{id}/state",
    operation_id = "changeComplaintState",
    tag = "complaints",
    params(("id" = Uuid, Path, description = "The Complaint")),
    request_body = ComplaintStateChange,
    responses(
        (status = NO_CONTENT, description = "The Complaint is moved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not handle Complaints"),
        (status = NOT_FOUND, description = "There is no such Complaint"),
        (status = CONFLICT, description = "The Complaint's state does not allow the move"),
        (status = UNPROCESSABLE_ENTITY, description = "The state is new, or the text is missing, blank or too long"),
    ),
)]
pub(crate) async fn change_state(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<ComplaintStateChange>,
) -> Result<StatusCode, ApiError> {
    ensure(&session, Permission::ComplaintHandle)?;
    let to = body.state;
    let text = match to {
        ComplaintState::New => return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY)),
        ComplaintState::InProgress => None,
        ComplaintState::Closed | ComplaintState::Rejected => {
            Some(required(body.text.as_deref().unwrap_or_default(), 2000)?)
        }
    };
    let mut tx = pool.begin().await?;
    let from = lock_open(&mut tx, id).await?;
    if to == ComplaintState::InProgress && from != ComplaintState::New {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    if to == ComplaintState::Closed {
        post_response(&mut tx, &session, id, text.as_deref().unwrap_or_default()).await?;
    }
    let reason = text.as_deref().filter(|_| to == ComplaintState::Rejected);
    set_state(&mut tx, &session, id, (from, to), reason).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A Complaint Topic as staff keep it.
#[derive(Serialize, ToSchema)]
pub struct ComplaintTopic {
    id: Uuid,
    name: String,
    /// Whether the public form offers it.
    active: bool,
    /// Whether any Complaint is filed under it, which keeps it from being deleted.
    used: bool,
}

/// What staff set on a Complaint Topic.
#[derive(Deserialize, ToSchema)]
pub struct ComplaintTopicFields {
    /// 1–50 characters, unique whatever the case.
    name: String,
    active: bool,
}

/// Locks the active Topics, so concurrent deactivations and deletes wait on each other and
/// cannot both pass [`ensure_active_topic`].
async fn lock_active_topics(tx: &mut PgConnection) -> Result<(), ApiError> {
    sqlx::query!("SELECT id FROM complaint_topics WHERE active FOR UPDATE")
        .fetch_all(&mut *tx)
        .await?;
    Ok(())
}

/// Refuses with `409` when no active Topic is left, which would leave the public form empty.
async fn ensure_active_topic(tx: &mut PgConnection) -> Result<(), ApiError> {
    let any = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM complaint_topics WHERE active) AS "any!""#
    )
    .fetch_one(&mut *tx)
    .await?;
    if any {
        Ok(())
    } else {
        Err(ApiError::Status(StatusCode::CONFLICT))
    }
}

/// Every Complaint Topic in order, active or not.
///
/// # Errors
/// `401` without a live session, `403` without `complaint.handle`.
#[utoipa::path(
    get,
    path = "/complaints/topics",
    operation_id = "listComplaintTopics",
    tag = "complaints",
    responses(
        (status = OK, description = "Every Complaint Topic", body = Vec<ComplaintTopic>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not handle Complaints"),
    ),
)]
pub(crate) async fn list_topics(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<ComplaintTopic>>, ApiError> {
    ensure(&session, Permission::ComplaintHandle)?;
    let topics = sqlx::query_as!(
        ComplaintTopic,
        r#"SELECT id, name, active,
           EXISTS (SELECT 1 FROM complaints WHERE topic_id = complaint_topics.id) AS "used!"
           FROM complaint_topics ORDER BY sort_order, id"#
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(topics))
}

/// Adds a Complaint Topic at the end of the list.
///
/// # Errors
/// `401` without a live session, `403` without `complaint.handle`, `409` for a name in use or
/// when no active Topic would be left, `422` for a blank name or one over 50 characters.
#[utoipa::path(
    post,
    path = "/complaints/topics",
    operation_id = "createComplaintTopic",
    tag = "complaints",
    request_body = ComplaintTopicFields,
    responses(
        (status = CREATED, description = "The new Complaint Topic", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not handle Complaints"),
        (status = CONFLICT, description = "Another Topic has the name, or no active Topic would be left"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is blank or too long"),
    ),
)]
pub(crate) async fn create_topic(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<ComplaintTopicFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure(&session, Permission::ComplaintHandle)?;
    let name = label(&body.name)?;
    let mut tx = pool.begin().await?;
    let id = unique_name(
        sqlx::query_scalar!(
            "INSERT INTO complaint_topics (name, active, sort_order)
             SELECT $1, $2, coalesce(max(sort_order), 0) + 1 FROM complaint_topics
             RETURNING id",
            name,
            body.active,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    ensure_active_topic(&mut tx).await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "complaint_topic.create",
        ("complaint_topic", Some(&id.to_string())),
        json!([
            { "field": "name", "from": null, "to": name },
            { "field": "active", "from": null, "to": body.active },
        ]),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Renames a Complaint Topic or (de)activates it; an inactive Topic keeps its Complaints but
/// takes no new ones.
///
/// # Errors
/// `401` without a live session, `403` without `complaint.handle`, `404` for an unknown Topic,
/// `409` for a name in use or when no active Topic would be left, `422` for a blank name or one
/// over 50 characters.
#[utoipa::path(
    put,
    path = "/complaints/topics/{id}",
    operation_id = "updateComplaintTopic",
    tag = "complaints",
    params(("id" = Uuid, Path, description = "The Complaint Topic")),
    request_body = ComplaintTopicFields,
    responses(
        (status = NO_CONTENT, description = "The Complaint Topic is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not handle Complaints"),
        (status = NOT_FOUND, description = "There is no such Complaint Topic"),
        (status = CONFLICT, description = "Another Topic has the name, or no active Topic would be left"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is blank or too long"),
    ),
)]
pub(crate) async fn update_topic(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<ComplaintTopicFields>,
) -> Result<StatusCode, ApiError> {
    ensure(&session, Permission::ComplaintHandle)?;
    let name = label(&body.name)?;
    let mut tx = pool.begin().await?;
    lock_active_topics(&mut tx).await?;
    let old = sqlx::query!(
        "SELECT name, active FROM complaint_topics WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let changes = diff(
        &json!({ "name": old.name, "active": old.active }),
        &json!({ "name": name, "active": body.active }),
    );
    if !changes.is_empty() {
        unique_name(
            sqlx::query!(
                "UPDATE complaint_topics SET name = $1, active = $2 WHERE id = $3",
                name,
                body.active,
                id
            )
            .execute(&mut *tx)
            .await,
        )?;
        ensure_active_topic(&mut tx).await?;
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "complaint_topic.update",
            ("complaint_topic", Some(&id.to_string())),
            json!(changes),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Complaint Topic no Complaint is filed under; a used one can only be deactivated.
///
/// # Errors
/// `401` without a live session, `403` without `complaint.handle`, `404` for an unknown Topic,
/// `409` while a Complaint is filed under it or when no active Topic would be left.
#[utoipa::path(
    delete,
    path = "/complaints/topics/{id}",
    operation_id = "deleteComplaintTopic",
    tag = "complaints",
    params(("id" = Uuid, Path, description = "The Complaint Topic")),
    responses(
        (status = NO_CONTENT, description = "The Complaint Topic is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not handle Complaints"),
        (status = NOT_FOUND, description = "There is no such Complaint Topic"),
        (status = CONFLICT, description = "A Complaint is filed under the Topic, or no active Topic would be left"),
    ),
)]
pub(crate) async fn delete_topic(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure(&session, Permission::ComplaintHandle)?;
    let mut tx = pool.begin().await?;
    lock_active_topics(&mut tx).await?;
    // Complaints reference their Topic, so the delete itself refuses a used one.
    let name = match sqlx::query_scalar!(
        "DELETE FROM complaint_topics WHERE id = $1 RETURNING name",
        id
    )
    .fetch_optional(&mut *tx)
    .await
    {
        Err(sqlx::Error::Database(error)) if error.is_foreign_key_violation() => {
            return Err(ApiError::Status(StatusCode::CONFLICT));
        }
        name => name?.ok_or(ApiError::Status(StatusCode::NOT_FOUND))?,
    };
    ensure_active_topic(&mut tx).await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "complaint_topic.delete",
        ("complaint_topic", Some(&id.to_string())),
        json!([{ "field": "name", "from": name, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts every Complaint Topic, active or not, in a new order.
///
/// # Errors
/// `401` without a live session, `403` without `complaint.handle`, `422` unless the list holds
/// every Topic exactly once.
#[utoipa::path(
    put,
    path = "/complaints/topics/order",
    operation_id = "orderComplaintTopics",
    tag = "complaints",
    request_body = Order,
    responses(
        (status = NO_CONTENT, description = "The Complaint Topics are reordered"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not handle Complaints"),
        (status = UNPROCESSABLE_ENTITY, description = "The list is not every Topic exactly once"),
    ),
)]
pub(crate) async fn order_topics(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<Order>,
) -> Result<StatusCode, ApiError> {
    ensure(&session, Permission::ComplaintHandle)?;
    let mut tx = pool.begin().await?;
    let old =
        sqlx::query_scalar!("SELECT id FROM complaint_topics ORDER BY sort_order, id FOR UPDATE")
            .fetch_all(&mut *tx)
            .await?;
    if !body.matches(&old) {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    sqlx::query!(
        "UPDATE complaint_topics SET sort_order = ordered.sort_order
         FROM unnest($1::uuid[]) WITH ORDINALITY AS ordered (id, sort_order)
         WHERE complaint_topics.id = ordered.id",
        &body.ids,
    )
    .execute(&mut *tx)
    .await?;
    if old != body.ids {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "complaint_topic.order",
            ("complaint_topic", None),
            json!([{ "field": "order", "from": old, "to": body.ids }]),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_four_groups_of_crockford_base32() {
        let code = new_code();
        assert_eq!(code.len(), 19);
        assert!(code.split('-').all(|group| {
            group.len() == 4
                && group
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
                && !group.contains(['I', 'L', 'O', 'U'])
        }));
        assert_ne!(new_code(), code);
    }
}
