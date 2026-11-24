use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use pranata::{AppState, Config, db, openapi};
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser)]
#[command(name = "pranata", version, about = "Pranata API")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Apply migrations, then serve the API.
    Serve,
    /// Apply migrations and exit.
    Migrate,
    /// Print the `OpenAPI` document as JSON.
    Openapi,
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Openapi => {
            println!("{}", openapi::document().to_pretty_json()?);
            Ok(())
        }
        Command::Migrate => {
            init_tracing();
            let config = Config::from_env().context("loading configuration")?;
            db::migrate(&config.migration_database_url).await
        }
        Command::Serve => {
            init_tracing();
            serve().await
        }
    }
}

fn init_tracing() {
    fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}

async fn serve() -> Result<()> {
    let config = Config::from_env().context("loading configuration")?;
    db::migrate(&config.migration_database_url).await?;
    let pool = db::connect(&config.database_url).await?;
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .context("binding listener")?;
    tracing::info!(addr = %config.listen, "listening");
    let state = AppState {
        db: pool,
        config: Arc::new(config),
    };
    axum::serve(listener, pranata::app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
