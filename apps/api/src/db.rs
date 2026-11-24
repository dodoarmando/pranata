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
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("running migrations")?;
    pool.close().await;
    Ok(())
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
