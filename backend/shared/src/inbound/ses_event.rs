use serde::Deserialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundReceipt {
    pub ses_message_id: String,
    pub timestamp: Option<String>,
    pub recipients: Vec<String>,
    pub verdicts: InboundReceiptVerdicts,
    pub raw_mail: RawMailPointer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundReceiptVerdicts {
    pub spf: Option<String>,
    pub dkim: Option<String>,
    pub dmarc: Option<String>,
    pub spam: Option<String>,
    pub virus: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMailPointer {
    pub bucket: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMailLocation {
    bucket: String,
    prefix: String,
}

impl RawMailLocation {
    pub fn new(bucket: impl Into<String>, prefix: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            prefix: normalize_object_prefix(prefix.into()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct SesReceiptEvent {
    #[serde(rename = "Records")]
    records: Vec<SesRecord>,
}

#[derive(Debug, Deserialize)]
struct SesRecord {
    #[serde(rename = "eventSource")]
    event_source: String,
    ses: SesPayload,
}

#[derive(Debug, Deserialize)]
struct SesPayload {
    mail: SesMail,
    receipt: SesReceipt,
}

#[derive(Debug, Deserialize)]
struct SesMail {
    #[serde(rename = "messageId")]
    message_id: String,
    timestamp: Option<String>,
    destination: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct SesReceipt {
    recipients: Option<Vec<String>>,
    #[serde(rename = "spfVerdict")]
    spf_verdict: Option<SesVerdict>,
    #[serde(rename = "dkimVerdict")]
    dkim_verdict: Option<SesVerdict>,
    #[serde(rename = "dmarcVerdict")]
    dmarc_verdict: Option<SesVerdict>,
    #[serde(rename = "spamVerdict")]
    spam_verdict: Option<SesVerdict>,
    #[serde(rename = "virusVerdict")]
    virus_verdict: Option<SesVerdict>,
    action: SesAction,
}

#[derive(Debug, Deserialize)]
struct SesVerdict {
    status: String,
}

#[derive(Debug, Deserialize)]
struct SesAction {
    #[serde(rename = "type")]
    action_type: String,
    #[serde(rename = "bucketName")]
    bucket_name: Option<String>,
    #[serde(rename = "objectKey")]
    object_key: Option<String>,
}

pub fn parse_ses_receipt_event(payload: Value) -> AppResult<Vec<InboundReceipt>> {
    parse_ses_receipt_event_with_raw_mail_location(payload, None)
}

pub fn parse_ses_receipt_event_with_raw_mail_location(
    payload: Value,
    raw_mail_location: Option<&RawMailLocation>,
) -> AppResult<Vec<InboundReceipt>> {
    let event: SesReceiptEvent = serde_json::from_value(payload)
        .map_err(|err| AppError::Validation(format!("invalid SES receipt event: {err}")))?;
    if event.records.is_empty() {
        return Err(AppError::Validation(
            "SES receipt event must contain at least one record".to_string(),
        ));
    }

    event
        .records
        .into_iter()
        .enumerate()
        .map(|(index, record)| record.into_receipt(index, raw_mail_location))
        .collect()
}

impl SesRecord {
    fn into_receipt(
        self,
        index: usize,
        raw_mail_location: Option<&RawMailLocation>,
    ) -> AppResult<InboundReceipt> {
        if self.event_source != "aws:ses" {
            return Err(AppError::Validation(format!(
                "record {index} is not an SES event"
            )));
        }

        let ses_message_id = require_non_empty(
            self.ses.mail.message_id,
            format!("record {index} missing SES message id"),
        )?;
        let recipients = self
            .ses
            .receipt
            .recipients
            .or(self.ses.mail.destination)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|recipient| {
                let recipient = recipient.trim();
                (!recipient.is_empty()).then(|| recipient.to_string())
            })
            .collect::<Vec<_>>();
        if recipients.is_empty() {
            return Err(AppError::Validation(format!(
                "record {index} must include at least one recipient"
            )));
        }

        let raw_mail =
            self.ses
                .receipt
                .action
                .raw_mail_pointer(index, &ses_message_id, raw_mail_location)?;

        Ok(InboundReceipt {
            ses_message_id,
            timestamp: self.ses.mail.timestamp,
            recipients,
            verdicts: InboundReceiptVerdicts {
                spf: self.ses.receipt.spf_verdict.map(|verdict| verdict.status),
                dkim: self.ses.receipt.dkim_verdict.map(|verdict| verdict.status),
                dmarc: self.ses.receipt.dmarc_verdict.map(|verdict| verdict.status),
                spam: self.ses.receipt.spam_verdict.map(|verdict| verdict.status),
                virus: self.ses.receipt.virus_verdict.map(|verdict| verdict.status),
            },
            raw_mail,
        })
    }
}

impl SesAction {
    fn raw_mail_pointer(
        &self,
        index: usize,
        ses_message_id: &str,
        raw_mail_location: Option<&RawMailLocation>,
    ) -> AppResult<RawMailPointer> {
        if self.action_type == "S3" {
            let bucket = require_non_empty(
                self.bucket_name.clone().unwrap_or_default(),
                format!("record {index} missing S3 bucket"),
            )?;
            let key = require_non_empty(
                self.object_key.clone().unwrap_or_default(),
                format!("record {index} missing S3 object key"),
            )?;
            return Ok(RawMailPointer { bucket, key });
        }

        let Some(location) = raw_mail_location else {
            return Err(AppError::Validation(format!(
                "record {index} receipt action must be S3"
            )));
        };
        let bucket = require_non_empty(
            location.bucket.clone(),
            format!("record {index} missing configured raw mail bucket"),
        )?;
        let key = configured_object_key(&location.prefix, ses_message_id);
        Ok(RawMailPointer { bucket, key })
    }
}

fn require_non_empty(value: String, message: String) -> AppResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AppError::Validation(message));
    }
    Ok(value)
}

fn normalize_object_prefix(prefix: String) -> String {
    let prefix = prefix.trim().to_string();
    if prefix.is_empty() || prefix.ends_with('/') {
        prefix
    } else {
        format!("{prefix}/")
    }
}

fn configured_object_key(prefix: &str, ses_message_id: &str) -> String {
    if prefix.is_empty() {
        ses_message_id.to_string()
    } else {
        format!("{prefix}{ses_message_id}")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        InboundReceipt, RawMailLocation, parse_ses_receipt_event,
        parse_ses_receipt_event_with_raw_mail_location,
    };

    fn fixture(name: &str) -> Value {
        serde_json::from_str(include_str!(concat!(
            "../../tests/fixtures/inbound/",
            "ses_receipt_clean.json"
        )))
        .and_then(|default_fixture| {
            if name == "ses_receipt_clean.json" {
                Ok(default_fixture)
            } else {
                serde_json::from_str(include_str!(concat!(
                    "../../tests/fixtures/inbound/",
                    "ses_receipt_multi_record.json"
                )))
            }
        })
        .unwrap()
    }

    #[test]
    fn ses_event_parses_clean_one_record_event() {
        let receipts = parse_ses_receipt_event(fixture("ses_receipt_clean.json")).unwrap();

        assert_eq!(receipts.len(), 1);
        let receipt = &receipts[0];
        assert_clean_receipt_identity(receipt);
        assert_clean_receipt_storage(receipt);
        assert_clean_receipt_verdicts(receipt);
    }

    fn assert_clean_receipt_identity(receipt: &InboundReceipt) {
        assert_eq!(receipt.ses_message_id, "ses-message-1");
        assert_eq!(
            receipt.timestamp.as_deref(),
            Some("2026-06-10T18:00:00.000Z")
        );
        assert_eq!(
            receipt.recipients,
            vec!["contact@ahara.io".to_string(), "chris@ahara.io".to_string()]
        );
    }

    fn assert_clean_receipt_storage(receipt: &InboundReceipt) {
        assert_eq!(receipt.raw_mail.bucket, "ahara-business-raw-mail-test");
        assert_eq!(receipt.raw_mail.key, "raw/ses-message-1");
    }

    fn assert_clean_receipt_verdicts(receipt: &InboundReceipt) {
        assert_eq!(receipt.verdicts.spf.as_deref(), Some("PASS"));
        assert_eq!(receipt.verdicts.dkim.as_deref(), Some("PASS"));
        assert_eq!(receipt.verdicts.dmarc.as_deref(), Some("PASS"));
        assert_eq!(receipt.verdicts.spam.as_deref(), Some("PASS"));
        assert_eq!(receipt.verdicts.virus.as_deref(), Some("PASS"));
    }

    #[test]
    fn ses_event_parses_multi_record_event() {
        let receipts = parse_ses_receipt_event(fixture("ses_receipt_multi_record.json")).unwrap();

        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].ses_message_id, "ses-message-1");
        assert_eq!(receipts[1].ses_message_id, "ses-message-2");
        assert_eq!(receipts[1].raw_mail.key, "raw/ses-message-2");
        assert_eq!(receipts[1].verdicts.spam.as_deref(), Some("FAIL"));
    }

    #[test]
    fn ses_event_derives_raw_mail_pointer_for_lambda_invocation() {
        let payload = json!({
            "Records": [{
                "eventSource": "aws:ses",
                "ses": {
                    "mail": {
                        "messageId": "ses-message-1",
                        "timestamp": "2026-06-10T18:00:00.000Z",
                        "destination": ["contact@ahara.io"]
                    },
                    "receipt": {
                        "recipients": ["contact@ahara.io"],
                        "spfVerdict": { "status": "PASS" },
                        "dkimVerdict": { "status": "PASS" },
                        "dmarcVerdict": { "status": "PASS" },
                        "spamVerdict": { "status": "PASS" },
                        "virusVerdict": { "status": "PASS" },
                        "action": {
                            "type": "Lambda",
                            "functionArn": "arn:aws:lambda:us-east-1:123456789012:function:ahara-business-ingest",
                            "invocationType": "Event"
                        }
                    }
                }
            }]
        });

        let location = RawMailLocation::new("ahara-business-raw-mail-test", "raw/");
        let receipts =
            parse_ses_receipt_event_with_raw_mail_location(payload, Some(&location)).unwrap();

        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].raw_mail.bucket, "ahara-business-raw-mail-test");
        assert_eq!(receipts[0].raw_mail.key, "raw/ses-message-1");
    }

    #[test]
    fn ses_event_validates_required_fields() {
        for payload in [
            json!({ "Records": [] }),
            json!({
                "Records": [{
                    "eventSource": "aws:sns",
                    "ses": {
                        "mail": { "messageId": "ses-message", "destination": ["contact@ahara.io"] },
                        "receipt": { "recipients": ["contact@ahara.io"], "action": { "type": "S3", "bucketName": "bucket", "objectKey": "raw/key" } }
                    }
                }]
            }),
            json!({
                "Records": [{
                    "eventSource": "aws:ses",
                    "ses": {
                        "mail": { "messageId": "", "destination": ["contact@ahara.io"] },
                        "receipt": { "recipients": ["contact@ahara.io"], "action": { "type": "S3", "bucketName": "bucket", "objectKey": "raw/key" } }
                    }
                }]
            }),
            json!({
                "Records": [{
                    "eventSource": "aws:ses",
                    "ses": {
                        "mail": { "messageId": "ses-message", "destination": [] },
                        "receipt": { "recipients": [], "action": { "type": "S3", "bucketName": "bucket", "objectKey": "raw/key" } }
                    }
                }]
            }),
            json!({
                "Records": [{
                    "eventSource": "aws:ses",
                    "ses": {
                        "mail": { "messageId": "ses-message", "destination": ["contact@ahara.io"] },
                        "receipt": { "recipients": ["contact@ahara.io"], "action": { "type": "S3", "bucketName": "", "objectKey": "raw/key" } }
                    }
                }]
            }),
        ] {
            assert!(parse_ses_receipt_event(payload).is_err());
        }
    }
}
