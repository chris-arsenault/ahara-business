use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::routing::parse_route;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SesFeedbackKind {
    Bounce,
    Complaint,
}

impl SesFeedbackKind {
    pub fn status(self) -> &'static str {
        match self {
            Self::Bounce => "bounced",
            Self::Complaint => "complained",
        }
    }

    pub fn suppression_reason(self) -> &'static str {
        match self {
            Self::Bounce => "bounce",
            Self::Complaint => "complaint",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SesFeedbackEvent {
    pub kind: SesFeedbackKind,
    pub provider_message_id: String,
    pub recipients: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FeedbackProcessSummary {
    pub processed: usize,
    pub suppressed_recipients: usize,
    pub bounced: usize,
    pub complained: usize,
}

#[async_trait]
pub trait FeedbackService: Send + Sync {
    async fn process_feedback(&self, payload: Value) -> AppResult<FeedbackProcessSummary>;
}

#[derive(Debug, Clone)]
pub struct PgFeedbackService {
    pool: DbPool,
}

impl PgFeedbackService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FeedbackService for PgFeedbackService {
    async fn process_feedback(&self, payload: Value) -> AppResult<FeedbackProcessSummary> {
        let events = parse_ses_feedback_event(payload)?;
        let mut summary = FeedbackProcessSummary::default();
        for event in events {
            summary.processed += 1;
            let message_id = self
                .update_message_status(&event.provider_message_id, event.kind)
                .await?;
            match event.kind {
                SesFeedbackKind::Bounce => summary.bounced += 1,
                SesFeedbackKind::Complaint => summary.complained += 1,
            }
            for recipient in event.recipients {
                upsert_suppression(
                    &self.pool,
                    &recipient,
                    event.kind,
                    message_id,
                    &event.provider_message_id,
                )
                .await?;
                summary.suppressed_recipients += 1;
            }
        }
        Ok(summary)
    }
}

impl PgFeedbackService {
    async fn update_message_status(
        &self,
        provider_message_id: &str,
        kind: SesFeedbackKind,
    ) -> AppResult<Option<Uuid>> {
        let row: Option<MessageIdRow> = sqlx::query_as(
            "UPDATE messages
             SET status = $2,
                 last_error = $3,
                 updated_at = now()
             WHERE direction = 'outbound'
               AND ses_message_id = $1
             RETURNING id",
        )
        .bind(provider_message_id)
        .bind(kind.status())
        .bind(format!("ses {}", kind.status()))
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        if let Some(row) = row {
            sqlx::query(
                "UPDATE outbound_work
                 SET status = $2,
                     last_error = $3,
                     updated_at = now()
                 WHERE message_id = $1",
            )
            .bind(row.id)
            .bind(kind.status())
            .bind(format!("ses {}", kind.status()))
            .execute(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
            Ok(Some(row.id))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryFeedbackService {
    state: Arc<Mutex<InMemoryFeedbackState>>,
}

impl InMemoryFeedbackService {
    pub fn seed_provider_message(&self, provider_message_id: &str, message_id: Uuid) {
        self.state
            .lock()
            .unwrap()
            .messages_by_provider_id
            .insert(provider_message_id.to_string(), message_id);
    }

    pub fn message_status(&self, message_id: Uuid) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .message_status
            .get(&message_id)
            .cloned()
    }

    pub fn suppression_reason(&self, address_normalized: &str) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .suppressions
            .get(address_normalized)
            .cloned()
    }
}

#[async_trait]
impl FeedbackService for InMemoryFeedbackService {
    async fn process_feedback(&self, payload: Value) -> AppResult<FeedbackProcessSummary> {
        let events = parse_ses_feedback_event(payload)?;
        let mut summary = FeedbackProcessSummary::default();
        let mut state = self.state.lock().unwrap();
        for event in events {
            summary.processed += 1;
            let source_message_id = state
                .messages_by_provider_id
                .get(&event.provider_message_id)
                .copied();
            if let Some(message_id) = source_message_id {
                state
                    .message_status
                    .insert(message_id, event.kind.status().to_string());
            }
            match event.kind {
                SesFeedbackKind::Bounce => summary.bounced += 1,
                SesFeedbackKind::Complaint => summary.complained += 1,
            }
            for recipient in event.recipients {
                let normalized = normalize_email_address(&recipient)?;
                state
                    .suppressions
                    .insert(normalized, event.kind.suppression_reason().to_string());
                summary.suppressed_recipients += 1;
            }
        }
        Ok(summary)
    }
}

#[derive(Debug, Clone, Default)]
struct InMemoryFeedbackState {
    messages_by_provider_id: BTreeMap<String, Uuid>,
    message_status: BTreeMap<Uuid, String>,
    suppressions: BTreeMap<String, String>,
}

pub fn parse_ses_feedback_event(payload: Value) -> AppResult<Vec<SesFeedbackEvent>> {
    let records = payload
        .get("Records")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation("feedback event records are required".to_string()))?;
    let mut events = Vec::new();
    for record in records {
        let message = record
            .get("Sns")
            .or_else(|| record.get("sns"))
            .and_then(|sns| sns.get("Message").or_else(|| sns.get("message")))
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Validation("SNS message is required".to_string()))?;
        let payload: Value = serde_json::from_str(message)
            .map_err(|err| AppError::Validation(format!("invalid SES feedback message: {err}")))?;
        events.push(parse_feedback_message(&payload)?);
    }
    Ok(events)
}

fn parse_feedback_message(payload: &Value) -> AppResult<SesFeedbackEvent> {
    let notification_type = payload
        .get("notificationType")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation("SES notificationType is required".to_string()))?;
    let kind = match notification_type {
        "Bounce" => SesFeedbackKind::Bounce,
        "Complaint" => SesFeedbackKind::Complaint,
        _ => {
            return Err(AppError::Validation(format!(
                "unsupported SES feedback notification type {notification_type}"
            )));
        }
    };
    let provider_message_id = payload
        .get("mail")
        .and_then(|mail| mail.get("messageId"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Validation("SES mail.messageId is required".to_string()))?
        .to_string();
    let recipients = match kind {
        SesFeedbackKind::Bounce => feedback_recipients(payload, "bounce", "bouncedRecipients")?,
        SesFeedbackKind::Complaint => {
            feedback_recipients(payload, "complaint", "complainedRecipients")?
        }
    };

    Ok(SesFeedbackEvent {
        kind,
        provider_message_id,
        recipients,
    })
}

fn feedback_recipients(payload: &Value, section: &str, field: &str) -> AppResult<Vec<String>> {
    let recipients = payload
        .get(section)
        .and_then(|section| section.get(field))
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation(format!("SES {section}.{field} is required")))?;
    let mut parsed = Vec::new();
    for recipient in recipients {
        let address = recipient
            .get("emailAddress")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                AppError::Validation("SES feedback recipient emailAddress is required".to_string())
            })?;
        parsed.push(address.trim().to_string());
    }
    Ok(parsed)
}

async fn upsert_suppression(
    pool: &DbPool,
    address: &str,
    kind: SesFeedbackKind,
    source_message_id: Option<Uuid>,
    provider_message_id: &str,
) -> AppResult<()> {
    let address_normalized = normalize_email_address(address)?;
    sqlx::query(
        "INSERT INTO suppressions (
             address, address_normalized, reason, source_message_id, notes
         )
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (address_normalized)
         DO UPDATE
         SET address = EXCLUDED.address,
             reason = EXCLUDED.reason,
             source_message_id = COALESCE(EXCLUDED.source_message_id, suppressions.source_message_id),
             notes = EXCLUDED.notes",
    )
    .bind(address.trim())
    .bind(address_normalized)
    .bind(kind.suppression_reason())
    .bind(source_message_id)
    .bind(format!("ses feedback {provider_message_id}"))
    .execute(pool)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(())
}

fn normalize_email_address(address: &str) -> AppResult<String> {
    let parsed = parse_route(address).map_err(|err| AppError::Validation(err.to_string()))?;
    let (local_part, _domain) = parsed
        .address
        .split_once('@')
        .ok_or_else(|| AppError::Validation("address must contain exactly one @".to_string()))?;
    Ok(format!(
        "{}@{}",
        local_part.to_ascii_lowercase(),
        parsed.domain
    ))
}

#[derive(Debug, sqlx::FromRow)]
struct MessageIdRow {
    id: Uuid,
}

#[cfg(test)]
mod feedback_tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        FeedbackService, InMemoryFeedbackService, SesFeedbackEvent, SesFeedbackKind,
        parse_ses_feedback_event,
    };

    #[test]
    fn feedback_parses_sns_wrapped_bounce_and_complaint() {
        let events = parse_ses_feedback_event(json!({
            "Records": [
                sns_record(json!({
                    "notificationType": "Bounce",
                    "mail": { "messageId": "ses-message-1" },
                    "bounce": {
                        "bouncedRecipients": [
                            { "emailAddress": "Person@Example.COM" }
                        ]
                    }
                })),
                sns_record(json!({
                    "notificationType": "Complaint",
                    "mail": { "messageId": "ses-message-2" },
                    "complaint": {
                        "complainedRecipients": [
                            { "emailAddress": "Abuse@Example.COM" }
                        ]
                    }
                }))
            ]
        }))
        .unwrap();

        assert_eq!(
            events,
            vec![
                SesFeedbackEvent {
                    kind: SesFeedbackKind::Bounce,
                    provider_message_id: "ses-message-1".to_string(),
                    recipients: vec!["Person@Example.COM".to_string()],
                },
                SesFeedbackEvent {
                    kind: SesFeedbackKind::Complaint,
                    provider_message_id: "ses-message-2".to_string(),
                    recipients: vec!["Abuse@Example.COM".to_string()],
                },
            ]
        );
    }

    #[tokio::test]
    async fn feedback_service_suppresses_recipients_and_updates_status() {
        let service = InMemoryFeedbackService::default();
        let message_id = Uuid::new_v4();
        service.seed_provider_message("ses-message-1", message_id);

        let summary = service
            .process_feedback(json!({
                "Records": [sns_record(json!({
                    "notificationType": "Complaint",
                    "mail": { "messageId": "ses-message-1" },
                    "complaint": {
                        "complainedRecipients": [
                            { "emailAddress": "Person@Example.COM" }
                        ]
                    }
                }))]
            }))
            .await
            .unwrap();

        assert_eq!(summary.processed, 1);
        assert_eq!(summary.complained, 1);
        assert_eq!(summary.suppressed_recipients, 1);
        assert_eq!(
            service.message_status(message_id).as_deref(),
            Some("complained")
        );
        assert_eq!(
            service.suppression_reason("person@example.com").as_deref(),
            Some("complaint")
        );
    }

    fn sns_record(message: serde_json::Value) -> serde_json::Value {
        json!({
            "EventSource": "aws:sns",
            "Sns": {
                "Message": message.to_string()
            }
        })
    }
}
