use anyhow::Context;
use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::audit::{self, Actor};

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
