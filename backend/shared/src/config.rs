use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub mail: MailConfig,
    pub feedback: FeedbackConfig,
    pub api: ApiConfig,
    pub cognito: CognitoConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub name: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailConfig {
    pub domain: String,
    pub raw_mail_bucket: String,
    pub raw_mail_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackConfig {
    pub bounce_topic_arn: String,
    pub complaint_topic_arn: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiConfig {
    pub api_base_url: String,
    pub app_base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CognitoConfig {
    pub user_pool_id: String,
    pub client_id: String,
    pub domain: String,
    pub issuer: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    MissingEnv { name: &'static str },
    InvalidEnv { name: &'static str, reason: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnv { name } => write!(f, "missing required environment variable {name}"),
            Self::InvalidEnv { name, reason } => {
                write!(f, "invalid environment variable {name}: {reason}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub fn from_lookup(
        lookup: impl Fn(&'static str) -> Option<String>,
    ) -> Result<Self, ConfigError> {
        let db_port = optional_value(&lookup, "DB_PORT")
            .map(|port| {
                port.parse::<u16>().map_err(|_| ConfigError::InvalidEnv {
                    name: "DB_PORT",
                    reason: "must be an integer TCP port".to_string(),
                })
            })
            .transpose()?
            .unwrap_or(5432);

        Ok(Self {
            database: DatabaseConfig {
                host: required_value(&lookup, "DB_HOST")?,
                port: db_port,
                name: required_value(&lookup, "DB_NAME")?,
                username: required_value(&lookup, "DB_USERNAME")?,
                password: required_value(&lookup, "DB_PASSWORD")?,
            },
            mail: MailConfig {
                domain: required_value(&lookup, "MAIL_DOMAIN")?,
                raw_mail_bucket: required_value(&lookup, "RAW_MAIL_BUCKET")?,
                raw_mail_prefix: normalize_prefix(&required_value(&lookup, "RAW_MAIL_PREFIX")?),
            },
            feedback: FeedbackConfig {
                bounce_topic_arn: required_value(&lookup, "SES_BOUNCE_TOPIC_ARN")?,
                complaint_topic_arn: required_value(&lookup, "SES_COMPLAINT_TOPIC_ARN")?,
            },
            api: ApiConfig {
                api_base_url: required_value(&lookup, "API_BASE_URL")?,
                app_base_url: required_value(&lookup, "APP_BASE_URL")?,
            },
            cognito: CognitoConfig {
                user_pool_id: required_value(&lookup, "COGNITO_USER_POOL_ID")?,
                client_id: required_value(&lookup, "COGNITO_CLIENT_ID")?,
                domain: required_value(&lookup, "COGNITO_DOMAIN")?,
                issuer: required_value(&lookup, "COGNITO_ISSUER")?,
            },
        })
    }
}

fn required_value(
    lookup: &impl Fn(&'static str) -> Option<String>,
    name: &'static str,
) -> Result<String, ConfigError> {
    optional_value(lookup, name).ok_or(ConfigError::MissingEnv { name })
}

fn optional_value(
    lookup: &impl Fn(&'static str) -> Option<String>,
    name: &'static str,
) -> Option<String> {
    lookup(name).and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim().trim_matches('/');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}/")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{AppConfig, ConfigError};

    fn base_env() -> HashMap<&'static str, String> {
        HashMap::from([
            ("DB_HOST", "db.internal".to_string()),
            ("DB_NAME", "ahara_business".to_string()),
            ("DB_USERNAME", "app".to_string()),
            ("DB_PASSWORD", "secret".to_string()),
            ("MAIL_DOMAIN", "ahara.io".to_string()),
            ("RAW_MAIL_BUCKET", "ahara-business-raw-mail-123".to_string()),
            ("RAW_MAIL_PREFIX", "raw".to_string()),
            ("SES_BOUNCE_TOPIC_ARN", "arn:aws:sns:::bounces".to_string()),
            (
                "SES_COMPLAINT_TOPIC_ARN",
                "arn:aws:sns:::complaints".to_string(),
            ),
            ("API_BASE_URL", "https://api.example.test".to_string()),
            ("APP_BASE_URL", "https://app.example.test".to_string()),
            ("COGNITO_USER_POOL_ID", "us-east-1_pool".to_string()),
            ("COGNITO_CLIENT_ID", "client-123".to_string()),
            ("COGNITO_DOMAIN", "auth.example.test".to_string()),
            (
                "COGNITO_ISSUER",
                "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_pool".to_string(),
            ),
        ])
    }

    fn load(env: &HashMap<&'static str, String>) -> Result<AppConfig, ConfigError> {
        AppConfig::from_lookup(|name| env.get(name).cloned())
    }

    fn assert_database_config(config: &AppConfig) {
        assert_eq!(config.database.host, "db.internal");
        assert_eq!(config.database.port, 6543);
        assert_eq!(config.database.name, "ahara_business");
    }

    fn assert_mail_config(config: &AppConfig) {
        assert_eq!(config.mail.domain, "ahara.io");
        assert_eq!(config.mail.raw_mail_bucket, "ahara-business-raw-mail-123");
        assert_eq!(config.mail.raw_mail_prefix, "raw/");
        assert_eq!(config.feedback.bounce_topic_arn, "arn:aws:sns:::bounces");
    }

    fn assert_platform_config(config: &AppConfig) {
        assert_eq!(config.api.api_base_url, "https://api.example.test");
        assert_eq!(config.cognito.user_pool_id, "us-east-1_pool");
        assert_eq!(config.cognito.client_id, "client-123");
    }

    #[test]
    fn config_loads_required_runtime_values() {
        let mut env = base_env();
        env.insert("DB_PORT", "6543".to_string());

        let config = load(&env).unwrap();

        assert_database_config(&config);
        assert_mail_config(&config);
        assert_platform_config(&config);
    }

    #[test]
    fn config_reports_missing_required_values() {
        let mut env = base_env();
        env.remove("DB_HOST");

        let err = load(&env).unwrap_err();

        assert_eq!(err, ConfigError::MissingEnv { name: "DB_HOST" });
    }

    #[test]
    fn config_defaults_database_port() {
        let env = base_env();

        let config = load(&env).unwrap();

        assert_eq!(config.database.port, 5432);
    }

    #[test]
    fn config_normalizes_raw_mail_prefix_for_key_joins() {
        let mut env = base_env();
        env.insert("RAW_MAIL_PREFIX", "/raw/inbound//".to_string());

        let config = load(&env).unwrap();

        assert_eq!(config.mail.raw_mail_prefix, "raw/inbound/");
    }
}
