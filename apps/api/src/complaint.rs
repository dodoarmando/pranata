//! Complaints: what residents send from the public site, anonymously or with a name and phone,
//! and follow with their Complaint Code.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    ApiError, AppState, Clock,
    audit::{self, Actor},
    auth::client_ip,
    home::{required, text},
    public_form::{Proof, PublicForm},
    village::phone_ok,
};

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_form))
        .routes(routes!(submit))
        .routes(routes!(get_status))
}

/// Where a Complaint stands.
#[derive(Serialize, ToSchema, sqlx::Type, Clone, Copy, Debug, PartialEq, Eq)]
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
