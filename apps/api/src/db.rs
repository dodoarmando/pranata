use anyhow::{Context, Result};
use sqlx::{PgPool, postgres::PgPoolOptions};

/// Apply embedded migrations with the owner role.
///
/// # Errors
/// Fails when the database is unreachable or a migration fails.
pub async fn migrate(owner_url: &str) -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(owner_url)
        .await
        .context("connecting with the migration (owner) URL")?;
    run_migrations(&pool).await?;
    pool.close().await;
    Ok(())
}

/// Apply the embedded migrations that are not applied yet.
///
/// Migrations that the database has but this binary does not know are
/// ignored: after a rollback the previous release must start on the schema of
/// the newer one (expand/contract, D-50).
///
/// # Errors
/// Fails when a migration fails or an applied one was edited.
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .set_ignore_missing(true)
        .run(pool)
        .await
        .context("running migrations")
}

/// Connect the runtime pool with the restricted app role.
///
/// # Errors
/// Fails when the database is unreachable.
pub async fn connect(app_url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(app_url)
        .await
        .context("connecting with the app URL")
}
