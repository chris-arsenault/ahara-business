use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::inbound::types::{AuthResult, InboundAuthResults};
use crate::mail_security::SecurityDisposition;
use crate::outbound::{EnqueueForwardRequest, OutboundService};
use crate::routing::parse_route;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardingRuleConfig {
    pub id: String,
    pub domain_name: String,
    pub local_part: String,
    pub address_id: String,
    pub target_address: String,
    pub target_address_normalized: String,
    pub active: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertForwardingRuleRequest {
    pub domain_name: String,
    pub local_part: String,
    pub target_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardingPlannerMessage {
    pub message_id: String,
    pub thread_id: Option<String>,
    pub rfc_message_id: Option<String>,
    pub reference_ids: Vec<String>,
    pub from_address: String,
    pub subject: String,
    pub body_text: String,
    pub auth: InboundAuthResults,
    pub security_disposition: SecurityDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ForwardingPlanSummary {
    pub rules: usize,
    pub enqueued: usize,
    pub skipped: usize,
    pub failed: usize,
}

#[async_trait]
pub trait ForwardingRuleService: Send + Sync {
    async fn list_rules(&self) -> AppResult<Vec<ForwardingRuleConfig>>;

    async fn upsert_address_rule(
        &self,
        request: UpsertForwardingRuleRequest,
    ) -> AppResult<ForwardingRuleConfig>;

    async fn deactivate_rule(&self, rule_id: &str) -> AppResult<ForwardingRuleConfig>;

    async fn active_rules_for_message(
        &self,
        message_id: &str,
    ) -> AppResult<Vec<ForwardingRuleConfig>>;
}

#[derive(Clone)]
pub struct ForwardingPlanner {
    rules: Arc<dyn ForwardingRuleService>,
    outbound: Arc<dyn OutboundService>,
}

impl ForwardingPlanner {
    pub fn new(rules: Arc<dyn ForwardingRuleService>, outbound: Arc<dyn OutboundService>) -> Self {
        Self { rules, outbound }
    }

    pub async fn process_message(
        &self,
        message: ForwardingPlannerMessage,
    ) -> AppResult<ForwardingPlanSummary> {
        if message.security_disposition != SecurityDisposition::Accepted
            || !allows_forwarding_auth(&message.auth)
        {
            return Ok(ForwardingPlanSummary {
                skipped: 1,
                ..ForwardingPlanSummary::default()
            });
        }

        let rules = self
            .rules
            .active_rules_for_message(&message.message_id)
            .await?;
        let mut summary = ForwardingPlanSummary {
            rules: rules.len(),
            ..ForwardingPlanSummary::default()
        };
        for rule in rules {
            let request = EnqueueForwardRequest {
                source_message_id: message.message_id.clone(),
                source_thread_id: message.thread_id.clone(),
                source_rfc_message_id: message.rfc_message_id.clone(),
                source_reference_ids: message.reference_ids.clone(),
                forwarding_rule_id: rule.id.clone(),
                from_address: format!("{}@{}", rule.local_part, rule.domain_name),
                target_address: rule.target_address.clone(),
                original_from_address: message.from_address.clone(),
                original_subject: message.subject.clone(),
                original_body_text: message.body_text.clone(),
            };
            match self.outbound.enqueue_forward(request).await {
                Ok(_) => summary.enqueued += 1,
                Err(_) => summary.failed += 1,
            }
        }

        Ok(summary)
    }
}

#[derive(Debug, Clone)]
pub struct PgForwardingRuleService {
    pool: DbPool,
}

impl PgForwardingRuleService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ForwardingRuleService for PgForwardingRuleService {
    async fn list_rules(&self) -> AppResult<Vec<ForwardingRuleConfig>> {
        let rows: Vec<ForwardingRuleRow> = sqlx::query_as(FORWARDING_RULE_SELECT)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn upsert_address_rule(
        &self,
        request: UpsertForwardingRuleRequest,
    ) -> AppResult<ForwardingRuleConfig> {
        let source = normalize_source_address(&request.domain_name, &request.local_part)?;
        let target = normalize_target_address(&request.target_address)?;
        let address = self
            .find_active_address(&source.domain_name, &source.local_part)
            .await?;

        let row: ForwardingRuleRow = sqlx::query_as(
            "WITH upserted AS (
                 INSERT INTO forwarding_rules (
                     rule_kind, address_id, target_address, target_address_normalized, active
                 )
                 VALUES ('address', $1, $2, $3, true)
                 ON CONFLICT (address_id, target_address_normalized)
                 WHERE rule_kind = 'address'
                 DO UPDATE
                 SET target_address = EXCLUDED.target_address,
                     active = true,
                     updated_at = now()
                 RETURNING id
             )
             SELECT forwarding_rules.id,
                    domains.domain_name,
                    addresses.local_part,
                    forwarding_rules.address_id,
                    forwarding_rules.target_address,
                    forwarding_rules.target_address_normalized,
                    forwarding_rules.active,
                    forwarding_rules.created_at::text AS created_at,
                    forwarding_rules.updated_at::text AS updated_at
             FROM forwarding_rules
             JOIN upserted ON upserted.id = forwarding_rules.id
             JOIN addresses ON addresses.id = forwarding_rules.address_id
             JOIN domains ON domains.id = addresses.domain_id",
        )
        .bind(address.id)
        .bind(target.address)
        .bind(target.address_normalized)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        row.try_into()
    }

    async fn deactivate_rule(&self, rule_id: &str) -> AppResult<ForwardingRuleConfig> {
        let rule_id = parse_uuid(rule_id, "forwarding rule id")?;
        let row: Option<ForwardingRuleRow> = sqlx::query_as(
            "WITH deactivated AS (
                 UPDATE forwarding_rules
                 SET active = false,
                     updated_at = now()
                 WHERE id = $1
                   AND rule_kind = 'address'
                 RETURNING id
             )
             SELECT forwarding_rules.id,
                    domains.domain_name,
                    addresses.local_part,
                    forwarding_rules.address_id,
                    forwarding_rules.target_address,
                    forwarding_rules.target_address_normalized,
                    forwarding_rules.active,
                    forwarding_rules.created_at::text AS created_at,
                    forwarding_rules.updated_at::text AS updated_at
             FROM forwarding_rules
             JOIN deactivated ON deactivated.id = forwarding_rules.id
             JOIN addresses ON addresses.id = forwarding_rules.address_id
             JOIN domains ON domains.id = addresses.domain_id",
        )
        .bind(rule_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        row.ok_or_else(|| AppError::NotFound(format!("forwarding rule {rule_id}")))?
            .try_into()
    }

    async fn active_rules_for_message(
        &self,
        message_id: &str,
    ) -> AppResult<Vec<ForwardingRuleConfig>> {
        let message_id = parse_uuid(message_id, "message id")?;
        let rows: Vec<ForwardingRuleRow> = sqlx::query_as(
            "SELECT forwarding_rules.id,
                    domains.domain_name,
                    addresses.local_part,
                    forwarding_rules.address_id,
                    forwarding_rules.target_address,
                    forwarding_rules.target_address_normalized,
                    forwarding_rules.active,
                    forwarding_rules.created_at::text AS created_at,
                    forwarding_rules.updated_at::text AS updated_at
             FROM messages
             JOIN forwarding_rules ON forwarding_rules.address_id = messages.matched_address_id
             JOIN addresses ON addresses.id = forwarding_rules.address_id
             JOIN domains ON domains.id = addresses.domain_id
             WHERE messages.id = $1
               AND messages.direction = 'inbound'
               AND messages.security_disposition = 'accepted'
               AND messages.status = 'received'
               AND forwarding_rules.rule_kind = 'address'
               AND forwarding_rules.active = true
             ORDER BY domains.domain_name ASC,
                      addresses.local_part ASC,
                      forwarding_rules.target_address_normalized ASC",
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        rows.into_iter().map(TryInto::try_into).collect()
    }
}

impl PgForwardingRuleService {
    async fn find_active_address(
        &self,
        domain_name: &str,
        local_part: &str,
    ) -> AppResult<AddressRow> {
        sqlx::query_as(
            "SELECT addresses.id
             FROM addresses
             JOIN domains ON domains.id = addresses.domain_id
             WHERE domains.domain_name = $1
               AND domains.active = true
               AND addresses.local_part = $2
               AND addresses.active = true",
        )
        .bind(domain_name)
        .bind(local_part)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("address {local_part}@{domain_name}")))
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryForwardingRuleService {
    state: Arc<Mutex<InMemoryForwardingState>>,
}

impl InMemoryForwardingRuleService {
    pub fn with_addresses(addresses: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            state: Arc::new(Mutex::new(InMemoryForwardingState {
                addresses: addresses
                    .into_iter()
                    .map(|(domain, local)| {
                        (domain.to_ascii_lowercase(), local.to_ascii_lowercase())
                    })
                    .collect(),
                ..InMemoryForwardingState::default()
            })),
        }
    }

    pub fn seed_message_address(
        &self,
        message_id: impl Into<String>,
        domain_name: impl Into<String>,
        local_part: impl Into<String>,
    ) {
        self.state.lock().unwrap().message_addresses.insert(
            message_id.into(),
            (
                domain_name.into().to_ascii_lowercase(),
                local_part.into().to_ascii_lowercase(),
            ),
        );
    }
}

#[async_trait]
impl ForwardingRuleService for InMemoryForwardingRuleService {
    async fn list_rules(&self) -> AppResult<Vec<ForwardingRuleConfig>> {
        Ok(self.state.lock().unwrap().rules.values().cloned().collect())
    }

    async fn upsert_address_rule(
        &self,
        request: UpsertForwardingRuleRequest,
    ) -> AppResult<ForwardingRuleConfig> {
        let source = normalize_source_address(&request.domain_name, &request.local_part)?;
        let target = normalize_target_address(&request.target_address)?;
        let mut state = self.state.lock().unwrap();
        if !state
            .addresses
            .contains(&(source.domain_name.clone(), source.local_part.clone()))
        {
            return Err(AppError::NotFound(format!(
                "address {}@{}",
                source.local_part, source.domain_name
            )));
        }

        if let Some(existing) = state.rules.values_mut().find(|rule| {
            rule.domain_name == source.domain_name
                && rule.local_part == source.local_part
                && rule.target_address_normalized == target.address_normalized
        }) {
            existing.target_address = target.address;
            existing.active = true;
            return Ok(existing.clone());
        }

        let id = Uuid::new_v4().to_string();
        let rule = ForwardingRuleConfig {
            id: id.clone(),
            domain_name: source.domain_name.clone(),
            local_part: source.local_part.clone(),
            address_id: format!("{}:{}", source.domain_name, source.local_part),
            target_address: target.address,
            target_address_normalized: target.address_normalized,
            active: true,
            created_at: None,
            updated_at: None,
        };
        state.rules.insert(id, rule.clone());
        Ok(rule)
    }

    async fn deactivate_rule(&self, rule_id: &str) -> AppResult<ForwardingRuleConfig> {
        let mut state = self.state.lock().unwrap();
        let rule = state
            .rules
            .get_mut(rule_id)
            .ok_or_else(|| AppError::NotFound(format!("forwarding rule {rule_id}")))?;
        rule.active = false;
        Ok(rule.clone())
    }

    async fn active_rules_for_message(
        &self,
        message_id: &str,
    ) -> AppResult<Vec<ForwardingRuleConfig>> {
        let state = self.state.lock().unwrap();
        let Some((domain_name, local_part)) = state.message_addresses.get(message_id) else {
            return Ok(Vec::new());
        };
        Ok(state
            .rules
            .values()
            .filter(|rule| {
                rule.active && &rule.domain_name == domain_name && &rule.local_part == local_part
            })
            .cloned()
            .collect())
    }
}

#[derive(Debug, Clone, Default)]
struct InMemoryForwardingState {
    addresses: BTreeSet<(String, String)>,
    rules: BTreeMap<String, ForwardingRuleConfig>,
    message_addresses: BTreeMap<String, (String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedSourceAddress {
    domain_name: String,
    local_part: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedTargetAddress {
    address: String,
    address_normalized: String,
}

fn normalize_source_address(
    domain_name: &str,
    local_part: &str,
) -> AppResult<NormalizedSourceAddress> {
    let domain_name = domain_name.trim().to_ascii_lowercase();
    let local_part = local_part.trim().to_ascii_lowercase();
    parse_route(&format!("{local_part}@{domain_name}"))
        .map_err(|err| AppError::Validation(err.to_string()))?;
    Ok(NormalizedSourceAddress {
        domain_name,
        local_part,
    })
}

fn normalize_target_address(address: &str) -> AppResult<NormalizedTargetAddress> {
    let parsed = parse_route(address).map_err(|err| AppError::Validation(err.to_string()))?;
    let (local_part, _domain) = parsed
        .address
        .split_once('@')
        .ok_or_else(|| AppError::Validation("address must contain exactly one @".to_string()))?;
    let address_normalized = format!("{}@{}", local_part.to_ascii_lowercase(), parsed.domain);
    Ok(NormalizedTargetAddress {
        address: parsed.address,
        address_normalized,
    })
}

fn parse_uuid(value: &str, label: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| AppError::Validation(format!("{label} must be a UUID")))
}

fn allows_forwarding_auth(auth: &InboundAuthResults) -> bool {
    matches!(auth.spf, Some(AuthResult::Pass))
        && matches!(auth.dkim, Some(AuthResult::Pass))
        && matches!(auth.dmarc, Some(AuthResult::Pass))
        && matches!(auth.auth_verdict, Some(AuthResult::Pass))
}

const FORWARDING_RULE_SELECT: &str = "SELECT forwarding_rules.id,
                    domains.domain_name,
                    addresses.local_part,
                    forwarding_rules.address_id,
                    forwarding_rules.target_address,
                    forwarding_rules.target_address_normalized,
                    forwarding_rules.active,
                    forwarding_rules.created_at::text AS created_at,
                    forwarding_rules.updated_at::text AS updated_at
             FROM forwarding_rules
             JOIN addresses ON addresses.id = forwarding_rules.address_id
             JOIN domains ON domains.id = addresses.domain_id
             WHERE forwarding_rules.rule_kind = 'address'
             ORDER BY domains.domain_name ASC,
                      addresses.local_part ASC,
                      forwarding_rules.target_address_normalized ASC";

#[derive(Debug, sqlx::FromRow)]
struct AddressRow {
    id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct ForwardingRuleRow {
    id: Uuid,
    domain_name: String,
    local_part: String,
    address_id: Uuid,
    target_address: String,
    target_address_normalized: String,
    active: bool,
    created_at: Option<String>,
    updated_at: Option<String>,
}

impl TryFrom<ForwardingRuleRow> for ForwardingRuleConfig {
    type Error = AppError;

    fn try_from(row: ForwardingRuleRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id.to_string(),
            domain_name: row.domain_name,
            local_part: row.local_part,
            address_id: row.address_id.to_string(),
            target_address: row.target_address,
            target_address_normalized: row.target_address_normalized,
            active: row.active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[cfg(test)]
mod forwarding_rules_tests {
    use std::sync::Arc;

    use crate::inbound::types::{AuthResult, InboundAuthResults};
    use crate::mail_security::SecurityDisposition;
    use crate::outbound::{InMemoryOutboundService, OutboundMessageStatus, OutboundService};

    use super::{
        ForwardingPlanner, ForwardingPlannerMessage, ForwardingRuleService,
        InMemoryForwardingRuleService, UpsertForwardingRuleRequest,
    };

    #[tokio::test]
    async fn forwarding_rules_upsert_normalizes_target_and_reactivates() {
        let service = InMemoryForwardingRuleService::with_addresses([(
            "ahara.io".to_string(),
            "contact".to_string(),
        )]);
        let created = service
            .upsert_address_rule(UpsertForwardingRuleRequest {
                domain_name: "Ahara.IO".to_string(),
                local_part: "Contact".to_string(),
                target_address: "Target+Ops@Example.COM".to_string(),
            })
            .await
            .unwrap();
        let deactivated = service.deactivate_rule(&created.id).await.unwrap();
        let reactivated = service
            .upsert_address_rule(UpsertForwardingRuleRequest {
                domain_name: "ahara.io".to_string(),
                local_part: "contact".to_string(),
                target_address: "target+ops@example.com".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(created.domain_name, "ahara.io");
        assert_eq!(created.local_part, "contact");
        assert_eq!(created.target_address_normalized, "target+ops@example.com");
        assert!(!deactivated.active);
        assert_eq!(created.id, reactivated.id);
        assert!(reactivated.active);
    }

    #[tokio::test]
    async fn forwarding_rules_active_rules_for_message_returns_active_address_rules_only() {
        let service = InMemoryForwardingRuleService::with_addresses([
            ("ahara.io".to_string(), "contact".to_string()),
            ("ahara.io".to_string(), "chris".to_string()),
        ]);
        service.seed_message_address("message-1", "ahara.io", "contact");
        let active = service
            .upsert_address_rule(UpsertForwardingRuleRequest {
                domain_name: "ahara.io".to_string(),
                local_part: "contact".to_string(),
                target_address: "target@example.com".to_string(),
            })
            .await
            .unwrap();
        let inactive = service
            .upsert_address_rule(UpsertForwardingRuleRequest {
                domain_name: "ahara.io".to_string(),
                local_part: "contact".to_string(),
                target_address: "inactive@example.com".to_string(),
            })
            .await
            .unwrap();
        service.deactivate_rule(&inactive.id).await.unwrap();
        service
            .upsert_address_rule(UpsertForwardingRuleRequest {
                domain_name: "ahara.io".to_string(),
                local_part: "chris".to_string(),
                target_address: "other@example.com".to_string(),
            })
            .await
            .unwrap();

        let rules = service.active_rules_for_message("message-1").await.unwrap();

        assert_eq!(rules, vec![active]);
    }

    #[tokio::test]
    async fn forwarding_planner_enqueue_forward_for_accepted_pass_auth_message() {
        let rules = InMemoryForwardingRuleService::with_addresses([(
            "ahara.io".to_string(),
            "contact".to_string(),
        )]);
        let source_message_id = uuid::Uuid::new_v4().to_string();
        rules.seed_message_address(&source_message_id, "ahara.io", "contact");
        rules
            .upsert_address_rule(UpsertForwardingRuleRequest {
                domain_name: "ahara.io".to_string(),
                local_part: "contact".to_string(),
                target_address: "target@example.com".to_string(),
            })
            .await
            .unwrap();
        let outbound = InMemoryOutboundService::new("ahara.io");
        let planner = ForwardingPlanner::new(Arc::new(rules), Arc::new(outbound.clone()));

        let summary = planner
            .process_message(pass_auth_message(&source_message_id))
            .await
            .unwrap();
        let duplicate = planner
            .process_message(pass_auth_message(&source_message_id))
            .await
            .unwrap();
        let claimed = outbound.claim_due_work("worker-1", 25).await.unwrap();
        let raw = String::from_utf8(claimed[0].raw_message.clone()).unwrap();

        assert_eq!(summary.rules, 1);
        assert_eq!(summary.enqueued, 1);
        assert_eq!(duplicate.enqueued, 1);
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].to_addresses, vec!["target@example.com"]);
        assert_eq!(
            outbound
                .suppressed_recipient(&claimed[0].to_addresses)
                .await
                .unwrap(),
            None
        );
        assert!(raw.contains("Subject: Fwd: Invoice\r\n"));
        assert!(raw.contains("Reply-To: sender@example.com\r\n"));
        let detail = outbound
            .get_outbound_message(&claimed[0].message_id)
            .await
            .unwrap();
        assert_eq!(detail.status, OutboundMessageStatus::Sending);
        assert_eq!(
            detail.source_message_id.as_deref(),
            Some(source_message_id.as_str())
        );
    }

    #[tokio::test]
    async fn forwarding_planner_skips_non_pass_auth_messages() {
        let rules = InMemoryForwardingRuleService::with_addresses([(
            "ahara.io".to_string(),
            "contact".to_string(),
        )]);
        let source_message_id = uuid::Uuid::new_v4().to_string();
        rules.seed_message_address(&source_message_id, "ahara.io", "contact");
        rules
            .upsert_address_rule(UpsertForwardingRuleRequest {
                domain_name: "ahara.io".to_string(),
                local_part: "contact".to_string(),
                target_address: "target@example.com".to_string(),
            })
            .await
            .unwrap();
        let outbound = InMemoryOutboundService::new("ahara.io");
        let planner = ForwardingPlanner::new(Arc::new(rules), Arc::new(outbound.clone()));
        let mut message = pass_auth_message(&source_message_id);
        message.auth.dkim = Some(AuthResult::Fail);

        let summary = planner.process_message(message).await.unwrap();

        assert_eq!(summary.skipped, 1);
        assert!(
            outbound
                .claim_due_work("worker-1", 25)
                .await
                .unwrap()
                .is_empty()
        );
    }

    fn pass_auth_message(message_id: &str) -> ForwardingPlannerMessage {
        ForwardingPlannerMessage {
            message_id: message_id.to_string(),
            thread_id: Some(uuid::Uuid::new_v4().to_string()),
            rfc_message_id: Some("<source@example.com>".to_string()),
            reference_ids: vec!["<root@example.com>".to_string()],
            from_address: "sender@example.com".to_string(),
            subject: "Invoice".to_string(),
            body_text: "body".to_string(),
            auth: InboundAuthResults {
                spf: Some(AuthResult::Pass),
                dkim: Some(AuthResult::Pass),
                dmarc: Some(AuthResult::Pass),
                auth_verdict: Some(AuthResult::Pass),
            },
            security_disposition: SecurityDisposition::Accepted,
        }
    }
}
