use sqlx::postgres::PgPoolOptions;

use crate::config::{AppConfig, DatabaseConfig};
use crate::error::{AppError, AppResult};

pub const MAIL_MODEL_MIGRATION: &str = concat!(
    include_str!("../../../db/migrations/001_create_mail_model.sql"),
    "\n",
    include_str!("../../../db/migrations/002_attachments_retention_forwarding.sql"),
    "\n",
    include_str!("../../../db/migrations/003_calendar_booking.sql"),
    "\n",
    include_str!("../../../db/migrations/004_tax_audit_finance.sql")
);
pub const MAIL_MODEL_ROLLBACK: &str = concat!(
    include_str!("../../../db/migrations/rollback/004_tax_audit_finance.sql"),
    "\n",
    include_str!("../../../db/migrations/rollback/003_calendar_booking.sql"),
    "\n",
    include_str!("../../../db/migrations/rollback/002_attachments_retention_forwarding.sql"),
    "\n",
    include_str!("../../../db/migrations/rollback/001_create_mail_model.sql")
);
pub const INITIAL_ROUTING_SEED: &str =
    include_str!("../../../db/migrations/seed/001_initial_routing.sql");

pub type DbPool = sqlx::PgPool;

const MAX_POOL_CONNECTIONS: u32 = 5;

pub async fn connect_pool(config: &AppConfig) -> AppResult<DbPool> {
    PgPoolOptions::new()
        .max_connections(MAX_POOL_CONNECTIONS)
        .connect(&database_url(&config.database))
        .await
        .map_err(|err| AppError::Database(err.to_string()))
}

pub fn database_url(config: &DatabaseConfig) -> String {
    format!(
        "postgres://{}:{}@{}:{}/{}?sslmode=require",
        encode_userinfo(&config.username),
        encode_userinfo(&config.password),
        config.host,
        config.port,
        config.name
    )
}

fn encode_userinfo(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                vec![byte as char]
            }
            _ => {
                let encoded = format!("%{byte:02X}");
                encoded.chars().collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::config::DatabaseConfig;

    use super::{MAIL_MODEL_MIGRATION, database_url};

    #[test]
    fn database_url_uses_platform_env_config_with_tls_required() {
        let config = DatabaseConfig {
            host: "db.internal".to_string(),
            port: 6543,
            name: "ahara_business".to_string(),
            username: "app_user".to_string(),
            password: "p@ss word".to_string(),
        };

        assert_eq!(
            database_url(&config),
            "postgres://app_user:p%40ss%20word@db.internal:6543/ahara_business?sslmode=require"
        );
    }

    #[test]
    fn migration_constants_remain_available_for_storage_tests() {
        assert!(MAIL_MODEL_MIGRATION.contains("CREATE TABLE messages"));
    }
}
