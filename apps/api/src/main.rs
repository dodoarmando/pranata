use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use pranata::auth::session::{SessionClock, SystemClock};
use pranata::region_codes::VillageCode;
use pranata::village::{self, InitError};
use pranata::{AppState, Config, HashLimiter, auth, db, openapi};
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
    /// Manage Users.
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
    /// Manage the Installation's Village.
    Village {
        #[command(subcommand)]
        command: VillageCommand,
    },
}

#[derive(Subcommand)]
enum UserCommand {
    /// Create a User and print the Access Link (valid 24 hours) that lets them
    /// set a password. In R0 every User has full access.
    CreateAdmin {
        /// Sign-in name: 3 to 64 of a-z, 0-9, dot, underscore, hyphen.
        username: String,
        /// Name shown in the interface.
        #[arg(long = "name")]
        name: String,
    },
}

#[derive(Subcommand)]
enum VillageCommand {
    /// Create the Village from its code (fails when one already exists).
    Init {
        /// Village code, dotted (14.05.03.2002) or plain (1405032002).
        #[arg(long)]
        code: String,
    },
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
        Command::User {
            command: UserCommand::CreateAdmin { username, name },
        } => create_admin(&username, &name).await,
        Command::Village {
            command: VillageCommand::Init { code },
        } => village_init(&code).await,
        Command::Serve => {
            init_tracing();
            serve().await
        }
    }
}

async fn village_init(code: &str) -> Result<()> {
    let code = VillageCode::parse(code).context("invalid village code")?;
    let config = Config::from_env().context("loading configuration")?;
    // The app role cannot insert into `village`; initialisation is an owner task.
    let pool = db::connect(&config.migration_database_url).await?;
    match village::init(&pool, &code).await {
        Ok(profile) => {
            println!(
                "Village initialised: {} ({}, {}, {})",
                profile.name, profile.district, profile.regency, profile.province
            );
            Ok(())
        }
        Err(InitError::UnknownCode) => anyhow::bail!("village code {code} is not in the dataset"),
        Err(InitError::AlreadyInitialised) => anyhow::bail!("a village is already initialised"),
        Err(InitError::Internal(error)) => Err(error),
    }
}

async fn create_admin(username: &str, name: &str) -> Result<()> {
    let config = Config::from_env().context("loading configuration")?;
    // Users are an application concern, so this uses the app role's URL.
    let pool = db::connect(&config.database_url).await?;
    let created = auth::create_user_with_access_link(&pool, username, name, SystemClock.now())
        .await
        .map_err(|error| match error {
            pranata::ApiError::Internal(error) => error,
            other => anyhow::anyhow!("{other}"),
        })?;
    let origin = config.public_origin.as_str().trim_end_matches('/');
    println!("{origin}/akses#{}", created.access_link_token);
    Ok(())
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
        clock: Arc::new(SystemClock),
        hasher: HashLimiter::for_host().context("preparing the password hasher")?,
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
