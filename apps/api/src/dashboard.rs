use axum::{Json, extract::State};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError, Clock,
    auth::Session,
    complaint,
    role::Permission,
    village::{self, TimeZone},
};

/// The figures behind the admin dashboard's cards.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    active_users: i64,
    missing_village_data: MissingVillageData,
    /// Konten menunggu terbit: every Draft, the latest changed first; null for a User who may
    /// neither write nor publish content.
    drafts: Option<Vec<DraftSummary>>,
    /// Pengumuman segera berakhir: Published Announcements in effect today that end within 7
    /// days, the soonest first; null for a User who may neither write nor publish content.
    ending_announcements: Option<Vec<EndingAnnouncement>>,
    /// Pengaduan baru: how many Complaints are New; null without `complaint.read`.
    new_complaints: Option<i64>,
}

/// What kind of website item a Draft is.
#[derive(Serialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "camelCase")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum DraftKind {
    NewsPost,
    Announcement,
    Event,
    Page,
    Album,
    ImportantContact,
    Partner,
    TransparencyDocument,
    ApbdesSummary,
}

/// A Draft waiting to be published.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftSummary {
    id: Uuid,
    kind: DraftKind,
    title: String,
    updated_at: DateTime<Utc>,
}

/// A Published Announcement about to end.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndingAnnouncement {
    id: Uuid,
    title: String,
    /// The last day in effect.
    end_date: NaiveDate,
}

/// What the Village has yet to fill in.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissingVillageData {
    logo: bool,
    vision_mission: bool,
    /// Active Head-of-Hamlet Positions no Official serves in today.
    unfilled_positions: Vec<String>,
}

/// The admin dashboard's figures, for any signed-in User.
///
/// # Errors
/// `401` without a live session.
#[utoipa::path(
    get,
    path = "/dashboard",
    operation_id = "getDashboard",
    tag = "dashboard",
    responses(
        (status = OK, description = "The dashboard's figures", body = Dashboard),
        (status = UNAUTHORIZED, description = "There is no live session"),
    ),
)]
pub async fn get(
    State(pool): State<PgPool>,
    State(clock): State<Clock>,
    session: Session,
) -> Result<Json<Dashboard>, ApiError> {
    let active_users =
        sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM users WHERE active"#)
            .fetch_one(&pool)
            .await?;
    let village = sqlx::query!(
        "SELECT code, hamlet_label, logo_file_id IS NULL AS \"logo!\",
         vision IS NULL AS \"vision_mission!\" FROM village"
    )
    .fetch_optional(&pool)
    .await?;
    let unfilled_positions = match &village {
        None => Vec::new(),
        Some(village) => {
            sqlx::query_scalar!(
                r#"SELECT 'Kepala ' || initcap($2) || ' ' || hamlets.name AS "name!"
                   FROM positions JOIN region_units hamlets ON hamlets.id = hamlet_id
                   WHERE positions.active AND NOT EXISTS (SELECT 1 FROM tenures
                       WHERE position_id = positions.id
                       AND start_date <= (now() AT TIME ZONE $1)::date
                       AND (end_date IS NULL OR end_date >= (now() AT TIME ZONE $1)::date))
                   ORDER BY hamlets.sort_order"#,
                TimeZone::of_province(&village.code).iana(),
                village.hamlet_label,
            )
            .fetch_all(&pool)
            .await?
        }
    };
    let (drafts, ending_announcements) =
        if session.holds(Permission::ContentWrite) || session.holds(Permission::ContentPublish) {
            let today = village::today(&pool, clock.now()).await?;
            let drafts = sqlx::query_as!(
                DraftSummary,
                r#"SELECT id AS "id!", kind AS "kind!: DraftKind", title AS "title!",
                   updated_at AS "updated_at!"
                   FROM (SELECT id, 'news_post' AS kind, title, updated_at FROM news_posts
                         WHERE NOT published
                         UNION ALL SELECT id, 'announcement', title, updated_at
                         FROM announcements WHERE NOT published
                         UNION ALL SELECT id, 'event', title, updated_at
                         FROM events WHERE NOT published
                         UNION ALL SELECT id, 'page', title, updated_at
                         FROM pages WHERE NOT published
                         UNION ALL SELECT id, 'album', title, updated_at
                         FROM albums WHERE NOT published
                         UNION ALL SELECT id, 'important_contact', name, updated_at
                         FROM important_contacts WHERE NOT published
                         UNION ALL SELECT id, 'partner', name, updated_at
                         FROM partners WHERE NOT published
                         UNION ALL SELECT id, 'transparency_document', title, updated_at
                         FROM transparency_documents WHERE NOT published
                         UNION ALL SELECT id, 'apbdes_summary', 'APBDes ' || fiscal_year,
                         updated_at FROM apbdes_summaries WHERE NOT published) drafts
                   ORDER BY updated_at DESC, id DESC"#
            )
            .fetch_all(&pool)
            .await?;
            let ending = sqlx::query_as!(
                EndingAnnouncement,
                r#"SELECT id, title, end_date AS "end_date!" FROM announcements
                   WHERE published AND start_date <= $1 AND end_date BETWEEN $1 AND $2
                   ORDER BY end_date, id"#,
                today,
                today + TimeDelta::days(7),
            )
            .fetch_all(&pool)
            .await?;
            (Some(drafts), Some(ending))
        } else {
            (None, None)
        };
    let new_complaints = complaint::count_new(&pool, &session).await?;
    Ok(Json(Dashboard {
        active_users,
        new_complaints,
        drafts,
        ending_announcements,
        missing_village_data: MissingVillageData {
            logo: village.as_ref().is_none_or(|village| village.logo),
            vision_mission: village
                .as_ref()
                .is_none_or(|village| village.vision_mission),
            unfilled_positions,
        },
    }))
}
