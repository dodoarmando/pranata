//! Public search over Published News Posts, Announcements, Pages and Transparency Documents,
//! stemmed in Indonesian. What a resident types is never stored or logged.

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{ApiError, AppState, page::PageMenu, transparency_document::TransparencyDocumentType};

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(search))
}

/// How many results a page holds.
const PAGE_SIZE: i64 = 10;

/// What kind of item a result is.
#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy)]
#[serde(rename_all = "camelCase")]
#[sqlx(type_name = "search_kind", rename_all = "snake_case")]
pub enum SearchKind {
    NewsPost,
    Announcement,
    Page,
    TransparencyDocument,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// What the resident typed, 2–100 characters once trimmed; the last word matches as a
    /// prefix.
    q: String,
    /// Only results of this kind.
    r#type: Option<SearchKind>,
    /// From 1, the default.
    page: Option<u32>,
}

/// One search result, with what its link needs.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    kind: SearchKind,
    title: String,
    /// News Posts, Announcements and Pages.
    summary: Option<String>,
    /// News Posts, Pages and Announcements that have a body.
    slug: Option<String>,
    /// Pages.
    menu: Option<PageMenu>,
    /// Transparency Documents: the PDF, served at `/media/{file}/pdf`.
    file: Option<Uuid>,
    /// Transparency Documents.
    document_type: Option<TransparencyDocumentType>,
    /// Transparency Documents.
    fiscal_year: Option<i32>,
    /// The first publication; a Transparency Document's creation.
    dated_at: DateTime<Utc>,
}

/// One page of search results.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    results: Vec<SearchResult>,
    /// How many results match, over every page.
    total: i64,
    page_size: i64,
}

/// Published items matching the query, the best match first and then the newest, a page at a
/// time. Ended Announcements are included.
///
/// # Errors
/// `400` without a query or for an unknown kind, `422` for a query outside 2–100 characters.
#[utoipa::path(
    get,
    path = "/search",
    operation_id = "search",
    tag = "content",
    params(SearchQuery),
    responses(
        (status = OK, description = "A page of results", body = SearchPage),
        (status = BAD_REQUEST, description = "The query is missing or the kind unknown"),
        (status = UNPROCESSABLE_ENTITY, description = "The query is too short or too long"),
    ),
)]
pub(crate) async fn search(
    State(pool): State<PgPool>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchPage>, ApiError> {
    let q = query.q.trim();
    if !(2..=100).contains(&q.chars().count()) {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    let offset = (i64::from(query.page.unwrap_or(1).max(1)) - 1) * PAGE_SIZE;
    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM search_items
           WHERE search @@ search_query($1) AND ($2::search_kind IS NULL OR kind = $2)"#,
        q,
        query.r#type as Option<SearchKind>,
    )
    .fetch_one(&pool)
    .await?;
    let results = sqlx::query_as!(
        SearchResult,
        r#"SELECT kind AS "kind!: SearchKind", title AS "title!", summary, slug,
           menu AS "menu: PageMenu", file_id AS file,
           document_type AS "document_type: TransparencyDocumentType", fiscal_year,
           dated_at AS "dated_at!"
           FROM search_items
           WHERE search @@ search_query($1) AND ($2::search_kind IS NULL OR kind = $2)
           ORDER BY ts_rank_cd(search, search_query($1)) DESC, dated_at DESC, id DESC
           LIMIT $3 OFFSET $4"#,
        q,
        query.r#type as Option<SearchKind>,
        PAGE_SIZE,
        offset,
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(SearchPage {
        results,
        total,
        page_size: PAGE_SIZE,
    }))
}
