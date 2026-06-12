use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::mail_security::{SecurityDisposition, SecurityReason};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboundMailbox {
    pub address: String,
    pub address_normalized: String,
    pub display_name: String,
}

impl InboundMailbox {
    pub fn unknown() -> Self {
        Self {
            address: "unknown".to_string(),
            address_normalized: "unknown".to_string(),
            display_name: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InboundRecipientKind {
    To,
    Cc,
    Bcc,
}

impl InboundRecipientKind {
    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::To => "to",
            Self::Cc => "cc",
            Self::Bcc => "bcc",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboundRecipient {
    pub kind: InboundRecipientKind,
    pub mailbox: InboundMailbox,
    pub position: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboundAttachment {
    pub position: i32,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: Option<i64>,
    pub content_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedInboundMessage {
    pub from: InboundMailbox,
    pub subject: String,
    pub message_date_epoch: Option<i64>,
    pub rfc_message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub reference_ids: Vec<String>,
    pub recipients: Vec<InboundRecipient>,
    pub body_text: String,
    pub attachments: Vec<InboundAttachment>,
    pub size_bytes: i64,
}

impl ParsedInboundMessage {
    pub fn has_attachments(&self) -> bool {
        !self.attachments.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthResult {
    Pass,
    Fail,
    Neutral,
    Softfail,
    Temperror,
    Permerror,
    None,
}

impl AuthResult {
    pub fn from_ses_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pass" => Some(Self::Pass),
            "fail" => Some(Self::Fail),
            "neutral" => Some(Self::Neutral),
            "softfail" => Some(Self::Softfail),
            "temperror" => Some(Self::Temperror),
            "permerror" => Some(Self::Permerror),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Neutral => "neutral",
            Self::Softfail => "softfail",
            Self::Temperror => "temperror",
            Self::Permerror => "permerror",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InboundAuthResults {
    pub spf: Option<AuthResult>,
    pub dkim: Option<AuthResult>,
    pub dmarc: Option<AuthResult>,
    pub auth_verdict: Option<AuthResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboundSecurityRecord {
    pub disposition: SecurityDisposition,
    pub reason: SecurityReason,
    pub spam_result: Option<String>,
    pub virus_result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboundRoutingMatch {
    pub domain_id: Uuid,
    pub domain_name: String,
    pub address_id: Option<Uuid>,
    pub matched_local_part: String,
    pub plus_tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedInboundAudit {
    pub ses_message_id: String,
    pub s3_raw_key: Option<String>,
    pub envelope_recipients: Vec<String>,
    pub from: InboundMailbox,
    pub status: InboundMessageStatus,
    pub security: InboundSecurityRecord,
    pub rejection_reason: String,
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedInboundMessage {
    pub id: Uuid,
    pub status: InboundMessageStatus,
    pub security_disposition: SecurityDisposition,
    pub idempotent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InboundMessageStatus {
    Received,
    Quarantined,
    Rejected,
}

impl InboundMessageStatus {
    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Quarantined => "quarantined",
            Self::Rejected => "rejected",
        }
    }

    pub fn from_security_disposition(disposition: SecurityDisposition) -> Self {
        match disposition {
            SecurityDisposition::Accepted => Self::Received,
            SecurityDisposition::Quarantined => Self::Quarantined,
            SecurityDisposition::Rejected => Self::Rejected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthResult, InboundAttachment, InboundMessageStatus, InboundRecipient, InboundRecipientKind,
    };
    use crate::inbound::types::InboundMailbox;
    use crate::mail_security::SecurityDisposition;

    #[test]
    fn inbound_types_attachment_metadata_excludes_bytes() {
        let attachment = InboundAttachment {
            position: 0,
            filename: "invoice.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            size_bytes: Some(1234),
            content_id: Some("content-id".to_string()),
        };

        assert_eq!(attachment.filename, "invoice.pdf");
        assert_eq!(attachment.size_bytes, Some(1234));
    }

    #[test]
    fn inbound_types_recipient_kind_matches_database_values() {
        let recipient = InboundRecipient {
            kind: InboundRecipientKind::Cc,
            mailbox: InboundMailbox {
                address: "Person@Example.Test".to_string(),
                address_normalized: "person@example.test".to_string(),
                display_name: "Person".to_string(),
            },
            position: 1,
        };

        assert_eq!(recipient.kind.as_db_value(), "cc");
        assert_eq!(InboundRecipientKind::To.as_db_value(), "to");
        assert_eq!(InboundRecipientKind::Bcc.as_db_value(), "bcc");
    }

    #[test]
    fn inbound_types_status_and_auth_values_match_database_constraints() {
        assert_eq!(InboundMessageStatus::Received.as_db_value(), "received");
        assert_eq!(
            InboundMessageStatus::Quarantined.as_db_value(),
            "quarantined"
        );
        assert_eq!(InboundMessageStatus::Rejected.as_db_value(), "rejected");
        assert_eq!(
            InboundMessageStatus::from_security_disposition(SecurityDisposition::Accepted),
            InboundMessageStatus::Received
        );
        assert_eq!(AuthResult::Pass.as_db_value(), "pass");
        assert_eq!(AuthResult::Softfail.as_db_value(), "softfail");
        assert_eq!(AuthResult::Permerror.as_db_value(), "permerror");
    }
}
