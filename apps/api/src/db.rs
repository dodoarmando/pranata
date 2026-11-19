use anyhow::Context;
use sqlx::{PgPool, migrate::Migrator};

/// Forward-only migrations, embedded in the binary.
pub static MIGRATOR: Migrator = sqlx::migrate!();

/// Checks that the database holds exactly the migrations this binary embeds.
///
/// # Errors
/// When the database is unreachable, unmigrated, behind or ahead of this binary.
pub async fn ensure_schema(pool: &PgPool) -> anyhow::Result<()> {
    let applied: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT version, checksum FROM _sqlx_migrations WHERE success ORDER BY version",
    )
    .fetch_all(pool)
    .await
    .context("cannot read the schema version; run `pranata migrate`")?;
    let expected: Vec<(i64, Vec<u8>)> = MIGRATOR
        .iter()
        .map(|m| (m.version, m.checksum.to_vec()))
        .collect();

    anyhow::ensure!(
        applied == expected,
        "schema version mismatch: database is at {:?}, this binary expects {:?}; run `pranata migrate` or deploy the matching binary",
        applied.last().map(|m| m.0),
        expected.last().map(|m| m.0),
    );
    Ok(())
}
