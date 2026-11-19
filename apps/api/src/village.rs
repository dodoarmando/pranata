use std::sync::Arc;

use anyhow::Context;
use axum::{Json, body::Body, extract::State, http::StatusCode};
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    auth::Session,
    imaging::Kind,
    role::Permission,
    storage::Storage,
};

/// The national code list, one `dotted-code<TAB>name` row each; see `data/README.md`.
const REGIONS: &str = include_str!("../data/regions.tsv");

/// Indonesia's three time zones; the Village's drives every date-only rule.
#[derive(Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TimeZone {
    /// UTC+7
    Wib,
    /// UTC+8
    Wita,
    /// UTC+9
    Wit,
}

impl TimeZone {
    /// The time zone of the province a region code starts with.
    #[must_use]
    pub fn of_province(code: &str) -> Self {
        match code.as_bytes() {
            // Bali, Nusa Tenggara, South/East/North Kalimantan, Sulawesi
            [b'5', b'1'..=b'3', ..] | [b'6', b'3'..=b'5', ..] | [b'7', ..] => Self::Wita,
            // Maluku, Papua
            [b'8' | b'9', ..] => Self::Wit,
            // Sumatra, Java, West/Central Kalimantan
            _ => Self::Wib,
        }
    }

    /// Hours ahead of UTC; none of the three keeps daylight saving.
    pub(crate) fn utc_offset_hours(self) -> i32 {
        match self {
            Self::Wib => 7,
            Self::Wita => 8,
            Self::Wit => 9,
        }
    }

    /// `WIB`, `WITA` or `WIT`.
    pub(crate) fn abbreviation(self) -> &'static str {
        match self {
            Self::Wib => "WIB",
            Self::Wita => "WITA",
            Self::Wit => "WIT",
        }
    }

    /// The IANA name, for Postgres `AT TIME ZONE`.
    pub(crate) fn iana(self) -> &'static str {
        match self {
            Self::Wib => "Asia/Jakarta",
            Self::Wita => "Asia/Makassar",
            Self::Wit => "Asia/Jayapura",
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Village {
    /// The 10-digit Village Code.
    code: String,
    name: String,
    district: String,
    regency: String,
    province: String,
    time_zone: TimeZone,
}

/// The name of a dotted region code (`11`, `11.01`, `11.01.01` or `11.01.01.2001`) in the national list.
#[must_use]
pub fn region_name(code: &str) -> Option<&'static str> {
    REGIONS
        .lines()
        .find_map(|line| line.strip_prefix(code)?.strip_prefix('\t'))
}

/// Sets the Installation's Village from a desa or desa adat code in the national list.
///
/// # Errors
/// When the code is not 10 digits, is a kelurahan or otherwise not a desa or desa adat code,
/// is not in the national list, or the database write fails.
pub async fn set_code(pool: &PgPool, code: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        code.len() == 10 && code.bytes().all(|b| b.is_ascii_digit()),
        "a Village Code is 10 digits, got {code:?}"
    );
    match code.as_bytes()[6] {
        b'2' | b'3' => {}
        b'1' => anyhow::bail!("{code} is a kelurahan code; only desa and desa adat are served"),
        _ => anyhow::bail!("{code} is not a desa or desa adat code"),
    }
    let name = |dotted: String| {
        region_name(&dotted).with_context(|| format!("{code} is not in the national code list"))
    };
    let (p, r, d, v) = (&code[..2], &code[2..4], &code[4..6], &code[6..]);
    let village = name(format!("{p}.{r}.{d}.{v}"))?;
    let district = name(format!("{p}.{r}.{d}"))?;
    let regency = name(format!("{p}.{r}"))?;
    let province = name(p.to_owned())?;

    let mut tx = pool.begin().await?;
    sqlx::query!(
        "INSERT INTO village (code, name, district, regency, province) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (singleton) DO UPDATE SET code = EXCLUDED.code, name = EXCLUDED.name,
         district = EXCLUDED.district, regency = EXCLUDED.regency, province = EXCLUDED.province",
        code,
        village,
        district,
        regency,
        province,
    )
    .execute(&mut *tx)
    .await
    .context("cannot save the Village")?;
    audit::record(
        &mut tx,
        Actor::System,
        "village.set_code",
        ("village", Some(code)),
        json!({ "name": village }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// The Village's official identity; read-only, set by the vendor from the CLI.
///
/// # Errors
/// `404` before the Village Code is set, `500` when the database is unreachable.
#[utoipa::path(
    get,
    path = "/village",
    operation_id = "getVillage",
    tag = "village",
    responses(
        (status = OK, description = "The Village", body = Village),
        (status = NOT_FOUND, description = "The Village Code is not set yet"),
    ),
)]
pub async fn get_village(State(pool): State<PgPool>) -> Result<Json<Village>, StatusCode> {
    let row = sqlx::query!("SELECT code, name, district, regency, province FROM village")
        .fetch_optional(&pool)
        .await
        .map_err(|error| {
            tracing::error!(%error, "cannot read the Village");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(Village {
        time_zone: TimeZone::of_province(&row.code),
        code: row.code,
        name: row.name,
        district: row.district,
        regency: row.regency,
        province: row.province,
    }))
}

#[derive(Serialize, ToSchema)]
pub struct Upload {
    /// The stored file, unreferenced until saved into its owner and swept after 24 h if not.
    id: Uuid,
}

/// Stores a Village logo, a PNG or SVG of at most 2 MiB, for saving into the profile.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `413` over 2 MiB, `415`
/// for content that is not PNG or SVG, `422` over 40 megapixels or undecodable.
#[utoipa::path(
    post,
    path = "/village/logo",
    operation_id = "uploadVillageLogo",
    tag = "village",
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = CREATED, description = "The stored logo", body = Upload),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = PAYLOAD_TOO_LARGE, description = "The file is over 2 MiB"),
        (status = UNSUPPORTED_MEDIA_TYPE, description = "The file is not PNG or SVG"),
        (status = UNPROCESSABLE_ENTITY, description = "The image is too large or broken"),
    ),
)]
pub(crate) async fn upload_logo(
    State(pool): State<PgPool>,
    State(storage): State<Arc<Storage>>,
    session: Session,
    body: Body,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    if !session.holds(Permission::VillageManage) {
        return Err(ApiError::Status(StatusCode::FORBIDDEN));
    }
    let upload = axum::body::to_bytes(body, Kind::Logo.max_bytes())
        .await
        .map_err(|_| ApiError::Status(StatusCode::PAYLOAD_TOO_LARGE))?;
    let id = storage.save_image(&pool, Kind::Logo, upload.into()).await?;
    Ok((StatusCode::CREATED, Json(Upload { id })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_province_in_the_list_has_its_time_zone() {
        let wita = [
            "51", "52", "53", "63", "64", "65", "71", "72", "73", "74", "75", "76",
        ];
        let wit = ["81", "82", "91", "92", "93", "94", "95", "96"];
        let provinces: Vec<_> = REGIONS
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(code, _)| code)
            .filter(|code| code.len() == 2)
            .collect();

        assert_eq!(provinces.len(), 38);
        for code in provinces {
            let expected = if wita.contains(&code) {
                TimeZone::Wita
            } else if wit.contains(&code) {
                TimeZone::Wit
            } else {
                TimeZone::Wib
            };
            assert_eq!(TimeZone::of_province(code), expected, "{code}");
        }
    }
}
