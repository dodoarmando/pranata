//! Every Published item that has a page of its own, with its last change, for the public
//! site's `/sitemap.xml`.

use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{ApiError, AppState, page::PageMenu};

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_sitemap))
}

/// An item whose page is found by its slug.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SitemapItem {
    slug: String,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SitemapPage {
    menu: PageMenu,
    slug: String,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SitemapApbdesSummary {
    fiscal_year: i32,
    updated_at: DateTime<Utc>,
}

/// The Published items with a page, each list in no particular order.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Sitemap {
    news_posts: Vec<SitemapItem>,
    /// Only those with a body.
    announcements: Vec<SitemapItem>,
    pages: Vec<SitemapPage>,
    albums: Vec<SitemapItem>,
    apbdes_summaries: Vec<SitemapApbdesSummary>,
}

/// Every Published item that has a page, with when it last changed.
#[utoipa::path(
    get,
    path = "/sitemap",
    operation_id = "getSitemap",
    tag = "content",
    responses((status = OK, description = "The Published items with a page", body = Sitemap)),
)]
pub(crate) async fn get_sitemap(State(pool): State<PgPool>) -> Result<Json<Sitemap>, ApiError> {
    let news_posts = sqlx::query_as!(
        SitemapItem,
        "SELECT slug, updated_at FROM news_posts WHERE published"
    )
    .fetch_all(&pool)
    .await?;
    let announcements = sqlx::query_as!(
        SitemapItem,
        "SELECT slug, updated_at FROM announcements WHERE published AND has_body"
    )
    .fetch_all(&pool)
    .await?;
    let pages = sqlx::query_as!(
        SitemapPage,
        r#"SELECT menu AS "menu: PageMenu", slug, updated_at FROM pages WHERE published"#
    )
    .fetch_all(&pool)
    .await?;
    let albums = sqlx::query_as!(
        SitemapItem,
        "SELECT slug, updated_at FROM albums WHERE published"
    )
    .fetch_all(&pool)
    .await?;
    let apbdes_summaries = sqlx::query_as!(
        SitemapApbdesSummary,
        "SELECT fiscal_year, updated_at FROM apbdes_summaries WHERE published"
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(Sitemap {
        news_posts,
        announcements,
        pages,
        albums,
        apbdes_summaries,
    }))
}
