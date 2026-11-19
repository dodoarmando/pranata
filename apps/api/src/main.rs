use std::net::SocketAddr;

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
    },
    /// Apply pending database migrations.
    Migrate {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
    },
    /// Print the OpenAPI spec as JSON.
    Openapi,
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
        } => {
            let pool = PgPool::connect(&database_url).await?;
            pranata::db::ensure_schema(&pool).await?;
            let listener = tokio::net::TcpListener::bind(listen)
                .await
                .with_context(|| format!("cannot listen on {listen}"))?;
            // As PID 1 in a container, SIGTERM is ignored unless handled.
            let mut terminate = signal(SignalKind::terminate())?;
            tracing::info!(%listen, "serving");
            axum::serve(listener, pranata::app(pool))
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
    }
    Ok(())
}
