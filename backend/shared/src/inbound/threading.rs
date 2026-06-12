use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::inbound::types::{InboundMailbox, ParsedInboundMessage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadSeed {
    pub rfc_message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub reference_ids: Vec<String>,
    pub normalized_subject: String,
    pub participants: Vec<String>,
    pub activity_epoch: Option<i64>,
}

pub fn build_thread_seed(message: &ParsedInboundMessage) -> ThreadSeed {
    ThreadSeed {
        rfc_message_id: message.rfc_message_id.clone(),
        in_reply_to: message.in_reply_to.clone(),
        reference_ids: message.reference_ids.clone(),
        normalized_subject: normalize_subject(&message.subject),
        participants: normalized_participants(message),
        activity_epoch: message.message_date_epoch,
    }
}

pub fn normalize_subject(subject: &str) -> String {
    let mut normalized = subject.trim();
    loop {
        let trimmed = normalized.trim_start();
        let lower = trimmed.to_ascii_lowercase();
        let Some(prefix_len) = ["re:", "fw:", "fwd:"]
            .into_iter()
            .find_map(|prefix| lower.starts_with(prefix).then_some(prefix.len()))
        else {
            normalized = trimmed;
            break;
        };
        normalized = &trimmed[prefix_len..];
    }
    normalized
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn normalized_participants(message: &ParsedInboundMessage) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut participants = Vec::new();
    for address in std::iter::once(&message.from.address_normalized).chain(
        message
            .recipients
            .iter()
            .map(|recipient| &recipient.mailbox.address_normalized),
    ) {
        if seen.insert(address.clone()) {
            participants.push(address.clone());
        }
    }
    participants
}

pub fn participants_json(participants: &[String]) -> Value {
    Value::Array(
        participants
            .iter()
            .cloned()
            .map(Value::String)
            .collect::<Vec<_>>(),
    )
}

#[async_trait]
pub trait ContactLinkLookup: Send + Sync {
    async fn find_contact_id_by_sender(&self, sender: &InboundMailbox) -> AppResult<Option<Uuid>>;
}

#[derive(Debug, Clone)]
pub struct PgContactLinkLookup {
    pool: DbPool,
}

impl PgContactLinkLookup {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContactLinkLookup for PgContactLinkLookup {
    async fn find_contact_id_by_sender(&self, sender: &InboundMailbox) -> AppResult<Option<Uuid>> {
        let row: Option<ContactIdRow> = sqlx::query_as(
            "SELECT id
             FROM contacts
             WHERE primary_address_normalized = $1",
        )
        .bind(&sender.address_normalized)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        Ok(row.map(|row| row.id))
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ContactIdRow {
    id: Uuid,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryContactLinkLookup {
    contacts_by_address: Arc<Mutex<BTreeMap<String, Uuid>>>,
}

impl InMemoryContactLinkLookup {
    pub fn with_contacts(contacts: impl IntoIterator<Item = (String, Uuid)>) -> Self {
        Self {
            contacts_by_address: Arc::new(Mutex::new(
                contacts
                    .into_iter()
                    .map(|(address, id)| (address.to_ascii_lowercase(), id))
                    .collect(),
            )),
        }
    }
}

#[async_trait]
impl ContactLinkLookup for InMemoryContactLinkLookup {
    async fn find_contact_id_by_sender(&self, sender: &InboundMailbox) -> AppResult<Option<Uuid>> {
        Ok(self
            .contacts_by_address
            .lock()
            .unwrap()
            .get(&sender.address_normalized)
            .copied())
    }
}
