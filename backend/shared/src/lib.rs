pub mod attachments;
pub mod auth;
pub mod config;
pub mod contacts;
pub mod db;
pub mod domain_config;
pub mod error;
pub mod feedback;
pub mod forwarding;
pub mod inbound;
pub mod mail_security;
pub mod mailbox;
pub mod observability;
pub mod outbound;
pub mod ports;
pub mod raw_mail_store;
pub mod retention;
pub mod routing;
pub mod ses_mail_sender;

pub const SERVICE_NAME: &str = "ahara-business";

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .json()
        .try_init();
}

pub fn service_name() -> &'static str {
    SERVICE_NAME
}

#[cfg(test)]
mod tests {
    use super::service_name;

    #[test]
    fn exposes_service_name() {
        assert_eq!(service_name(), "ahara-business");
    }
}
