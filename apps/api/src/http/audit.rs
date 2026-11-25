//! Audit log routes: a filtered, paged list and a CSV export. Both need
//! `audit.read` and only read, so neither needs Sudo Mode.

use std::borrow::Cow;

use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use time::{
    Date,
    format_description::well_known::{Iso8601, Rfc3339},
};

use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    ApiError, AppState, FieldError,
    access::Permission,
    audit::{self, AuditCursor, AuditFilter, AuditRow},
    auth::CurrentUser,
    error::Problem,
    http::auth::no_store,
    ids::UserId,
    village,
};

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 200;
const EXPORT_LIMIT: i64 = 10_000;

/// One page of the log.
#[derive(Debug, Serialize, ToSchema)]
pub struct AuditPage {
    pub entries: Vec<AuditRow>,
    /// Pass as `before` to get the next page; `null` on the last page.
    pub next: Option<String>,
}

/// The filters both routes share. Empty text counts as absent.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ExportQuery {
    /// Only entries by this User.
    actor: Option<String>,
    /// Only this action, for example `update`.
    action: Option<String>,
    /// Only this kind of object, for example `user`.
    object_type: Option<String>,
    /// First day, `YYYY-MM-DD` in the Village's time zone.
    from: Option<String>,
    /// Last day, inclusive, `YYYY-MM-DD` in the Village's time zone.
    to: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListQuery {
    /// Only entries by this User.
    actor: Option<String>,
    /// Only this action, for example `update`.
    action: Option<String>,
    /// Only this kind of object, for example `user`.
    object_type: Option<String>,
    /// First day, `YYYY-MM-DD` in the Village's time zone.
    from: Option<String>,
    /// Last day, inclusive, `YYYY-MM-DD` in the Village's time zone.
    to: Option<String>,
    /// The `next` value of the previous page.
    before: Option<String>,
    /// Page size, 1 to 200; 50 when absent.
    limit: Option<String>,
}

fn present(value: Option<&String>) -> Option<&str> {
    value.map(String::as_str).filter(|text| !text.is_empty())
}

fn invalid(field: &str, code: &str) -> FieldError {
    FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
    }
}

fn parse_date(field: &str, value: Option<&String>, errors: &mut Vec<FieldError>) -> Option<Date> {
    let text = present(value)?;
    let date = Date::parse(text, &Iso8601::DATE).ok();
    if date.is_none() {
        errors.push(invalid(field, "invalid"));
    }
    date
}

/// Read the filters shared by both routes; `limit` is left for the caller.
fn parse_filter(query: &ExportQuery, errors: &mut Vec<FieldError>) -> AuditFilter {
    let actor = present(query.actor.as_ref()).and_then(|text| {
        let id = Uuid::parse_str(text).ok().map(UserId);
        if id.is_none() {
            errors.push(invalid("actor", "invalid"));
        }
        id
    });
    let from = parse_date("from", query.from.as_ref(), errors);
    let to = parse_date("to", query.to.as_ref(), errors);
    if let (Some(from), Some(to)) = (from, to)
        && from > to
    {
        errors.push(invalid("from", "range_invalid"));
    }
    AuditFilter {
        actor,
        action: present(query.action.as_ref()).map(str::to_owned),
        object_type: present(query.object_type.as_ref()).map(str::to_owned),
        from,
        to,
        before: None,
        limit: DEFAULT_LIMIT,
    }
}

fn rejected(_: QueryRejection) -> ApiError {
    ApiError::Validation(vec![invalid("query", "invalid")])
}

fn failed(errors: Vec<FieldError>) -> Result<(), ApiError> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

/// Entries newest first, a page at a time.
///
/// # Errors
/// 403 `permission_denied` without `audit.read`; 422 `validation_failed` for a
/// malformed filter, cursor or limit, or `from` after `to` (field `from`,
/// code `range_invalid`).
#[utoipa::path(
    get,
    path = "/admin/audit",
    operation_id = "listAuditEntries",
    tag = "audit",
    params(ListQuery),
    security(("session" = [])),
    responses(
        (status = 200, description = "A page of entries and the cursor of the next page", body = AuditPage),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks audit.read (code permission_denied)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid filter (code validation_failed; field codes invalid, range_invalid)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn list_audit_entries(
    State(state): State<AppState>,
    user: CurrentUser,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    user.require(Permission::AuditRead)?;
    let Query(query) = query.map_err(rejected)?;
    let mut errors = Vec::new();
    let mut filter = parse_filter(
        &ExportQuery {
            actor: query.actor.clone(),
            action: query.action.clone(),
            object_type: query.object_type.clone(),
            from: query.from.clone(),
            to: query.to.clone(),
        },
        &mut errors,
    );
    filter.before = present(query.before.as_ref()).and_then(|text| {
        let cursor = AuditCursor::decode(text);
        if cursor.is_none() {
            errors.push(invalid("before", "invalid"));
        }
        cursor
    });
    let limit = match present(query.limit.as_ref()) {
        None => DEFAULT_LIMIT,
        Some(text) => text.parse::<i64>().map_or_else(
            |_| {
                errors.push(invalid("limit", "invalid"));
                DEFAULT_LIMIT
            },
            |limit| limit.clamp(1, MAX_LIMIT),
        ),
    };
    failed(errors)?;

    // One more than the page, to know whether another page follows.
    filter.limit = limit + 1;
    let offset = village::time_zone(&state.db).await?.utc_offset();
    let mut entries = audit::list(&state.db, &filter, offset).await?;
    let page = usize::try_from(limit).unwrap_or(usize::MAX);
    let next = (entries.len() > page)
        .then(|| {
            entries.truncate(page);
            entries.last().map(|row| {
                AuditCursor {
                    at: row.at,
                    id: row.id,
                }
                .encode()
            })
        })
        .flatten();
    Ok((no_store(), Json(AuditPage { entries, next })).into_response())
}

/// Make a cell safe to open in a spreadsheet: one that starts with `=`, `+`,
/// `-` or `@` (or a tab or carriage return) would run as a formula, so it is
/// prefixed with `'`.
#[must_use]
pub fn csv_cell(value: &str) -> Cow<'_, str> {
    if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        Cow::Owned(format!("'{value}"))
    } else {
        Cow::Borrowed(value)
    }
}

fn csv_body(rows: &[AuditRow], offset: time::UtcOffset) -> anyhow::Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record([
        "waktu",
        "pengguna",
        "tindakan",
        "objek",
        "id_objek",
        "ringkasan",
        "perubahan",
    ])?;
    for row in rows {
        let at = row
            .at
            .to_offset(offset)
            .replace_nanosecond(0)?
            .format(&Rfc3339)?;
        let actor = row
            .actor
            .as_ref()
            .map_or("Sistem", |actor| actor.display_name.as_str());
        let changes = serde_json::to_string(&row.changes)?;
        writer.write_record(
            [
                csv_cell(&at),
                csv_cell(actor),
                csv_cell(&row.action),
                csv_cell(&row.object_type),
                csv_cell(&row.object_id),
                csv_cell(&row.summary),
                csv_cell(&changes),
            ]
            .map(Cow::into_owned),
        )?;
    }
    Ok(writer.into_inner()?)
}

/// The log as a CSV file, newest first, at most 10 000 entries.
///
/// # Errors
/// 403 `permission_denied` without `audit.read`; 422 `validation_failed` for a
/// malformed filter or `from` after `to`.
#[utoipa::path(
    get,
    path = "/admin/audit.csv",
    operation_id = "exportAuditEntries",
    tag = "audit",
    params(ExportQuery),
    security(("session" = [])),
    responses(
        (status = 200, description = "A CSV file with the columns waktu, pengguna, tindakan, objek, id_objek, ringkasan, perubahan", content_type = "text/csv", body = String),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks audit.read (code permission_denied)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid filter (code validation_failed; field codes invalid, range_invalid)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn export_audit_entries(
    State(state): State<AppState>,
    user: CurrentUser,
    query: Result<Query<ExportQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    user.require(Permission::AuditRead)?;
    let Query(query) = query.map_err(rejected)?;
    let mut errors = Vec::new();
    let mut filter = parse_filter(&query, &mut errors);
    failed(errors)?;

    filter.limit = EXPORT_LIMIT;
    let offset = village::time_zone(&state.db).await?.utc_offset();
    let rows = audit::list(&state.db, &filter, offset).await?;
    let body = csv_body(&rows, offset)?;
    Ok((
        no_store(),
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/csv; charset=utf-8"),
            ),
            (
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"log-audit.csv\""),
            ),
        ],
        body,
    )
        .into_response())
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_audit_entries))
        .routes(routes!(export_audit_entries))
}

#[cfg(test)]
mod tests {
    use super::csv_cell;

    #[test]
    fn csv_cell_neutralises_formula_starters() {
        for value in ["=1+1", "+1", "-1", "@SUM(A1)"] {
            assert_eq!(csv_cell(value), format!("'{value}"));
        }
        assert_eq!(csv_cell("Menonaktifkan neng"), "Menonaktifkan neng");
        assert_eq!(csv_cell(""), "");
    }
}
