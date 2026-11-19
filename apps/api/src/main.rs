use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};
use sqlx::PgPool;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(version, about = "Pranata API server and admin tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the HTTP API; refuses to start on a schema mismatch.
    Serve {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        #[arg(long, env = "PRANATA_LISTEN", default_value = "127.0.0.1:8080")]
        listen: SocketAddr,
        /// The Installation's public origin, such as `https://example.desa.id`.
        #[arg(long, env = "PRANATA_PUBLIC_ORIGIN")]
        public_origin: String,
        /// Where uploaded files live; created when missing.
        #[arg(long, env = "PRANATA_FILES_DIR")]
        files_dir: PathBuf,
    },
    /// Apply pending database migrations.
    Migrate {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
    },
    /// Print the OpenAPI spec as JSON.
    Openapi,
    /// Provision the Installation's Village.
    #[command(subcommand)]
    Village(VillageCommand),
    /// Manage Users.
    #[command(subcommand)]
    User(UserCommand),
}

#[derive(Subcommand)]
enum UserCommand {
    /// Create a User with the Administrator Role and print their Access Link.
    Create {
        username: String,
        /// Give the User the Administrator Role; the only kind the CLI creates.
        #[arg(long, required = true)]
        admin: bool,
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        /// The Installation's public origin, which the Access Link points to.
        #[arg(long, env = "PRANATA_PUBLIC_ORIGIN")]
        public_origin: String,
    },
    /// Revoke a User's sessions and print a new Access Link, such as for a locked-out Administrator.
    Reset {
        username: String,
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        /// The Installation's public origin, which the Access Link points to.
        #[arg(long, env = "PRANATA_PUBLIC_ORIGIN")]
        public_origin: String,
    },
}

#[derive(Subcommand)]
enum VillageCommand {
    /// Set the Village from its 10-digit desa or desa adat code in the national list.
    SetCode {
        code: String,
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
    },
}

async fn clean_up_every_10_minutes(pool: PgPool, files_dir: PathBuf) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(600));
    loop {
        tick.tick().await;
        if let Err(error) = pranata::purge_expired(&pool).await {
            tracing::error!(%error, "cannot purge expired sessions and Access Links");
        }
        match pranata::storage::sweep(&pool, &files_dir).await {
            Ok(0) => {}
            Ok(swept) => tracing::info!(swept, "swept unreferenced files"),
            Err(error) => tracing::error!(error = format!("{error:#}"), "cannot sweep files"),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    match Cli::parse().command {
        Command::Serve {
            database_url,
            listen,
            public_origin,
            files_dir,
        } => {
            let pool = PgPool::connect(&database_url).await?;
            pranata::db::ensure_schema(&pool).await?;
            std::fs::create_dir_all(&files_dir)
                .with_context(|| format!("cannot create {}", files_dir.display()))?;
            let listener = tokio::net::TcpListener::bind(listen)
                .await
                .with_context(|| format!("cannot listen on {listen}"))?;
            // As PID 1 in a container, SIGTERM is ignored unless handled.
            let mut terminate = signal(SignalKind::terminate())?;
            tokio::spawn(clean_up_every_10_minutes(pool.clone(), files_dir.clone()));
            tracing::info!(%listen, "serving");
            let public_origin = public_origin.trim_end_matches('/').to_owned();
            axum::serve(listener, pranata::app(pool, public_origin, files_dir)?)
                .with_graceful_shutdown(async move {
                    tokio::select! {
                        _ = terminate.recv() => {}
                        _ = tokio::signal::ctrl_c() => {}
                    }
                    tracing::info!("shutting down");
                })
                .await?;
        }
        Command::Migrate { database_url } => {
            let pool = PgPool::connect(&database_url).await?;
            pranata::db::MIGRATOR.run(&pool).await?;
            tracing::info!("migrations applied");
        }
        Command::Openapi => println!("{}", pranata::openapi().to_pretty_json()?),
        Command::Village(VillageCommand::SetCode { code, database_url }) => {
            let pool = PgPool::connect(&database_url).await?;
            pranata::db::ensure_schema(&pool).await?;
            pranata::village::set_code(&pool, &code).await?;
            tracing::info!(%code, "village set");
        }
        Command::User(UserCommand::Create {
            username,
            admin: _,
            database_url,
            public_origin,
        }) => {
            let pool = PgPool::connect(&database_url).await?;
            pranata::db::ensure_schema(&pool).await?;
            let token = pranata::user::create_administrator(&pool, &username).await?;
            tracing::info!(%username, "administrator created");
            println!("{}/akses#{token}", public_origin.trim_end_matches('/'));
        }
        Command::User(UserCommand::Reset {
            username,
            database_url,
            public_origin,
        }) => {
            let pool = PgPool::connect(&database_url).await?;
            pranata::db::ensure_schema(&pool).await?;
            let token = pranata::user::reset_access_link(&pool, &username).await?;
            tracing::info!(%username, "access reset");
            println!("{}/akses#{token}", public_origin.trim_end_matches('/'));
        }
    }
    Ok(())
}
