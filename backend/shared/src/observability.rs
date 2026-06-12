use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::SERVICE_NAME;
use crate::error::{AppError, AppResult};

pub const MAIL_METRIC_NAMESPACE: &str = "AharaBusiness/Mail";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CountMetric {
    pub name: String,
    pub value: u64,
}

impl CountMetric {
    pub fn new(name: impl Into<String>, value: u64) -> AppResult<Self> {
        let name = name.into();
        validate_metric_name(&name)?;
        Ok(Self { name, value })
    }
}

pub fn redact_email_for_log(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return "email:empty".to_string();
    }
    let digest = Sha256::digest(normalized.as_bytes());
    format!("email:{:x}", &digest)[..22].to_string()
}

pub fn mail_metric_payload(
    handler: &str,
    mail_domain: &str,
    metrics: &[CountMetric],
) -> AppResult<Value> {
    mail_metric_payload_at(now_millis(), handler, mail_domain, metrics)
}

pub fn mail_metric_payload_at(
    timestamp_millis: u64,
    handler: &str,
    mail_domain: &str,
    metrics: &[CountMetric],
) -> AppResult<Value> {
    validate_dimension("handler", handler)?;
    validate_dimension("mail domain", mail_domain)?;
    if metrics.is_empty() {
        return Err(AppError::Validation(
            "at least one metric is required".to_string(),
        ));
    }
    for metric in metrics {
        validate_metric_name(&metric.name)?;
    }

    let metric_descriptors = metrics
        .iter()
        .map(|metric| json!({ "Name": metric.name, "Unit": "Count" }))
        .collect::<Vec<_>>();
    let mut payload = json!({
        "_aws": {
            "Timestamp": timestamp_millis,
            "CloudWatchMetrics": [{
                "Namespace": MAIL_METRIC_NAMESPACE,
                "Dimensions": [["Service", "Handler", "MailDomain"]],
                "Metrics": metric_descriptors
            }]
        },
        "Service": SERVICE_NAME,
        "Handler": handler,
        "MailDomain": mail_domain
    });

    let object = payload
        .as_object_mut()
        .expect("mail metric payload is always an object");
    for metric in metrics {
        object.insert(metric.name.clone(), json!(metric.value));
    }
    Ok(payload)
}

pub fn emit_mail_metric(
    handler: &str,
    mail_domain: &str,
    metrics: &[CountMetric],
) -> AppResult<()> {
    let payload = mail_metric_payload(handler, mail_domain, metrics)?;
    tracing::info!(metric = %payload, "mail metric");
    Ok(())
}

fn validate_dimension(label: &str, value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::Validation(format!("{label} is required")));
    }
    if value.contains('@')
        || value.contains('\r')
        || value.contains('\n')
        || value.contains('<')
        || value.contains('>')
    {
        return Err(AppError::Validation(format!("{label} is not PII-safe")));
    }
    Ok(())
}

fn validate_metric_name(name: &str) -> AppResult<()> {
    if name.is_empty() {
        return Err(AppError::Validation("metric name is required".to_string()));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err(AppError::Validation(
            "metric name must be ASCII alphanumeric or underscore".to_string(),
        ));
    }
    Ok(())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{CountMetric, MAIL_METRIC_NAMESPACE, mail_metric_payload_at, redact_email_for_log};

    #[test]
    fn observability_redacts_email_addresses_stably() {
        let redacted = redact_email_for_log("Sender@Example.Test");

        assert_eq!(redacted, redact_email_for_log("sender@example.test"));
        assert!(redacted.starts_with("email:"));
        assert!(!redacted.contains("sender"));
        assert!(!redacted.contains("example.test"));
        assert!(!redacted.contains('@'));
    }

    #[test]
    fn observability_builds_count_only_emf_payload() {
        let payload = mail_metric_payload_at(
            1_780_000_000_000,
            "ingest",
            "ahara.io",
            &[
                CountMetric::new("InboundProcessed", 2).unwrap(),
                CountMetric::new("InboundAccepted", 1).unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(payload["_aws"]["Timestamp"], 1_780_000_000_000_u64);
        assert_eq!(
            payload["_aws"]["CloudWatchMetrics"][0]["Namespace"],
            MAIL_METRIC_NAMESPACE
        );
        assert_eq!(payload["Service"], "ahara-business");
        assert_eq!(payload["Handler"], "ingest");
        assert_eq!(payload["MailDomain"], "ahara.io");
        assert_eq!(payload["InboundProcessed"], 2);
        assert_eq!(payload["InboundAccepted"], 1);
    }

    #[test]
    fn observability_metric_payload_omits_mail_content_and_pii() {
        let payload = mail_metric_payload_at(
            1_780_000_000_000,
            "feedback-handler",
            "ahara.io",
            &[CountMetric::new("FeedbackComplained", 1).unwrap()],
        )
        .unwrap();
        let serialized = serde_json::to_string(&payload).unwrap();

        for unsafe_fragment in [
            "sender@example.test",
            "recipient@example.test",
            "Subject: Invoice",
            "Plaintext invoice body",
            "raw/ses-message-1",
            "invoice.pdf",
            "From:",
            "To:",
        ] {
            assert!(!serialized.contains(unsafe_fragment));
        }
    }

    #[test]
    fn observability_rejects_pii_dimensions_and_unsafe_metric_names() {
        assert!(
            mail_metric_payload_at(
                1,
                "ingest",
                "sender@example.test",
                &[CountMetric::new("InboundProcessed", 1).unwrap()]
            )
            .is_err()
        );
        assert!(CountMetric::new("Inbound-Processed", 1).is_err());
    }
}
