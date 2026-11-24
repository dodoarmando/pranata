use std::{net::SocketAddr, path::PathBuf};

use secrecy::SecretString;
use url::Url;

const DEFAULT_LISTEN: &str = "0.0.0.0:8080";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing environment variable {0}")]
    Missing(&'static str),
    #[error("invalid value for {name}: {reason}")]
    Invalid { name: &'static str, reason: String },
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub migration_database_url: String,
    pub public_origin: Url,
    pub web_internal_url: Url,
    pub purge_token: SecretString,
    pub media_dir: PathBuf,
    pub listen: SocketAddr,
}

impl Config {
    /// Read the configuration from `PRANATA_*` environment variables.
    ///
    /// # Errors
    /// Returns an error when a required variable is missing or malformed.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Read the configuration through an arbitrary lookup (testable without
    /// touching the process environment).
    ///
    /// # Errors
    /// Returns an error when a required variable is missing or malformed.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let required = |name: &'static str| get(name).ok_or(ConfigError::Missing(name));
        let parse = |name: &'static str, value: &str| {
            value.parse::<Url>().map_err(|e| ConfigError::Invalid {
                name,
                reason: e.to_string(),
            })
        };
        let listen = get("PRANATA_LISTEN").unwrap_or_else(|| DEFAULT_LISTEN.to_owned());
        Ok(Self {
            database_url: required("PRANATA_DATABASE_URL")?,
            migration_database_url: required("PRANATA_MIGRATION_DATABASE_URL")?,
            public_origin: parse("PRANATA_PUBLIC_ORIGIN", &required("PRANATA_PUBLIC_ORIGIN")?)?,
            web_internal_url: parse(
                "PRANATA_WEB_INTERNAL_URL",
                &required("PRANATA_WEB_INTERNAL_URL")?,
            )?,
            purge_token: SecretString::from(required("PRANATA_PURGE_TOKEN")?),
            media_dir: PathBuf::from(required("PRANATA_MEDIA_DIR")?),
            listen: listen
                .parse()
                .map_err(|e: std::net::AddrParseError| ConfigError::Invalid {
                    name: "PRANATA_LISTEN",
                    reason: e.to_string(),
                })?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use secrecy::ExposeSecret;

    use super::*;

    fn env() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            ("PRANATA_DATABASE_URL", "postgres://app@db/pranata"),
            (
                "PRANATA_MIGRATION_DATABASE_URL",
                "postgres://owner@db/pranata",
            ),
            ("PRANATA_PUBLIC_ORIGIN", "https://desa.example"),
            ("PRANATA_WEB_INTERNAL_URL", "http://web:3000"),
            ("PRANATA_PURGE_TOKEN", "s3cret"),
            ("PRANATA_MEDIA_DIR", "/var/lib/pranata/media"),
        ])
    }

    fn load(env: &HashMap<&str, &str>) -> Result<Config, ConfigError> {
        Config::from_lookup(|k| env.get(k).map(|v| (*v).to_owned()))
    }

    #[test]
    fn reads_all_variables_and_defaults_listen() {
        let config = load(&env()).unwrap();
        assert_eq!(config.listen.to_string(), "0.0.0.0:8080");
        assert_eq!(config.purge_token.expose_secret(), "s3cret");
        assert_eq!(config.public_origin.as_str(), "https://desa.example/");
    }

    #[test]
    fn missing_variable_is_named() {
        let mut env = env();
        env.remove("PRANATA_PURGE_TOKEN");
        let err = load(&env).unwrap_err();
        assert!(matches!(err, ConfigError::Missing("PRANATA_PURGE_TOKEN")));
    }

    #[test]
    fn debug_output_hides_the_purge_token() {
        let config = load(&env()).unwrap();
        assert!(!format!("{config:?}").contains("s3cret"));
    }
}
