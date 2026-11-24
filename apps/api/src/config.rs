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

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub migration_database_url: String,
    pub public_origin: Url,
    pub web_internal_url: Url,
    pub purge_token: SecretString,
    pub media_dir: PathBuf,
    pub listen: SocketAddr,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("database_url", &"[redacted]")
            .field("migration_database_url", &"[redacted]")
            .field("public_origin", &self.public_origin)
            .field("web_internal_url", &self.web_internal_url)
            .field("purge_token", &"[redacted]")
            .field("media_dir", &self.media_dir)
            .field("listen", &self.listen)
            .finish()
    }
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
            web_internal_url: {
                let url = parse(
                    "PRANATA_WEB_INTERNAL_URL",
                    &required("PRANATA_WEB_INTERNAL_URL")?,
                )?;
                // Plain HTTP on the pod network; the purger has no TLS support.
                if url.scheme() != "http" {
                    return Err(ConfigError::Invalid {
                        name: "PRANATA_WEB_INTERNAL_URL",
                        reason: "the scheme must be http".to_owned(),
                    });
                }
                url
            },
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
    fn web_internal_url_must_be_http() {
        let mut env = env();
        env.insert("PRANATA_WEB_INTERNAL_URL", "https://web:3000");
        let err = load(&env).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Invalid {
                name: "PRANATA_WEB_INTERNAL_URL",
                ..
            }
        ));
    }

    #[test]
    fn missing_variable_is_named() {
        let mut env = env();
        env.remove("PRANATA_PURGE_TOKEN");
        let err = load(&env).unwrap_err();
        assert!(matches!(err, ConfigError::Missing("PRANATA_PURGE_TOKEN")));
    }

    #[test]
    fn debug_output_hides_secrets() {
        let config = load(&env()).unwrap();
        let debug = format!("{config:?}");
        for secret in ["s3cret", "apppw", "ownerpw"] {
            assert!(!debug.contains(secret), "leaked {secret}");
        }
    }
}
