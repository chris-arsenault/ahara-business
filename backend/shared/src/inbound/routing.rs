use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::inbound::types::InboundRoutingMatch;
use crate::routing::{RoutingPolicy, parse_route};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundRoutingDomain {
    pub id: Uuid,
    pub domain_name: String,
    pub routing_policy: RoutingPolicy,
    pub active: bool,
    pub addresses: Vec<InboundRoutingAddress>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundRoutingAddress {
    pub id: Uuid,
    pub local_part: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingDecision {
    Accepted(InboundRoutingMatch),
    Rejected(InboundRoutingRejection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundRoutingRejection {
    pub reason: RoutingRejectionReason,
    pub recipient: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingRejectionReason {
    NoRecipients,
    MalformedRecipient,
    UnknownDomain,
    InactiveDomain,
    InactiveAddress,
    AllowlistMiss,
}

impl RoutingRejectionReason {
    pub fn as_db_reason(self) -> &'static str {
        match self {
            Self::NoRecipients => "routing_no_recipients",
            Self::MalformedRecipient => "routing_malformed_recipient",
            Self::UnknownDomain => "routing_unknown_domain",
            Self::InactiveDomain => "routing_inactive_domain",
            Self::InactiveAddress => "routing_inactive_address",
            Self::AllowlistMiss => "routing_allowlist_miss",
        }
    }
}

#[async_trait]
pub trait InboundRoutingLookup: Send + Sync {
    async fn find_domain(&self, domain_name: &str) -> AppResult<Option<InboundRoutingDomain>>;
}

#[derive(Debug, Clone)]
pub struct PgInboundRoutingLookup {
    pool: DbPool,
}

impl PgInboundRoutingLookup {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InboundRoutingLookup for PgInboundRoutingLookup {
    async fn find_domain(&self, domain_name: &str) -> AppResult<Option<InboundRoutingDomain>> {
        let domain_name = domain_name.to_ascii_lowercase();
        let row: Option<DomainRow> = sqlx::query_as(
            "SELECT id, domain_name, routing_policy, active
             FROM domains
             WHERE domain_name = $1",
        )
        .bind(&domain_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };
        let address_rows: Vec<AddressRow> = sqlx::query_as(
            "SELECT addresses.id, addresses.local_part, addresses.active
             FROM addresses
             WHERE addresses.domain_id = $1
             ORDER BY addresses.local_part",
        )
        .bind(row.id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        Ok(Some(InboundRoutingDomain {
            id: row.id,
            domain_name: row.domain_name,
            routing_policy: RoutingPolicy::from_str(&row.routing_policy)
                .map_err(|err| AppError::Internal(err.to_string()))?,
            active: row.active,
            addresses: address_rows
                .into_iter()
                .map(|row| InboundRoutingAddress {
                    id: row.id,
                    local_part: row.local_part,
                    active: row.active,
                })
                .collect(),
        }))
    }
}

#[derive(Debug, sqlx::FromRow)]
struct DomainRow {
    id: Uuid,
    domain_name: String,
    routing_policy: String,
    active: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct AddressRow {
    id: Uuid,
    local_part: String,
    active: bool,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryInboundRoutingLookup {
    domains: Arc<Mutex<BTreeMap<String, InboundRoutingDomain>>>,
}

impl InMemoryInboundRoutingLookup {
    pub fn with_domains(domains: impl IntoIterator<Item = InboundRoutingDomain>) -> Self {
        Self {
            domains: Arc::new(Mutex::new(
                domains
                    .into_iter()
                    .map(|domain| (domain.domain_name.clone(), domain))
                    .collect(),
            )),
        }
    }
}

#[async_trait]
impl InboundRoutingLookup for InMemoryInboundRoutingLookup {
    async fn find_domain(&self, domain_name: &str) -> AppResult<Option<InboundRoutingDomain>> {
        Ok(self
            .domains
            .lock()
            .unwrap()
            .get(&domain_name.to_ascii_lowercase())
            .cloned())
    }
}

pub async fn resolve_inbound_route(
    recipients: &[String],
    lookup: &dyn InboundRoutingLookup,
) -> AppResult<RoutingDecision> {
    if recipients.is_empty() {
        return Ok(reject(RoutingRejectionReason::NoRecipients, None));
    }

    let mut first_rejection: Option<InboundRoutingRejection> = None;
    for recipient in recipients {
        let route = match parse_route(recipient) {
            Ok(route) => route,
            Err(_) => {
                first_rejection.get_or_insert(InboundRoutingRejection {
                    reason: RoutingRejectionReason::MalformedRecipient,
                    recipient: Some(recipient.clone()),
                });
                continue;
            }
        };

        let Some(domain) = lookup.find_domain(&route.domain).await? else {
            first_rejection.get_or_insert(InboundRoutingRejection {
                reason: RoutingRejectionReason::UnknownDomain,
                recipient: Some(recipient.clone()),
            });
            continue;
        };
        if !domain.active {
            first_rejection.get_or_insert(InboundRoutingRejection {
                reason: RoutingRejectionReason::InactiveDomain,
                recipient: Some(recipient.clone()),
            });
            continue;
        }

        match domain.routing_policy {
            RoutingPolicy::Catchall => {
                return Ok(RoutingDecision::Accepted(InboundRoutingMatch {
                    domain_id: domain.id,
                    domain_name: domain.domain_name,
                    address_id: None,
                    matched_local_part: route.base_local_part,
                    plus_tag: route.plus_tag,
                }));
            }
            RoutingPolicy::Allowlist => {
                let address = domain
                    .addresses
                    .iter()
                    .find(|address| address.local_part == route.base_local_part);
                match address {
                    Some(address) if address.active => {
                        return Ok(RoutingDecision::Accepted(InboundRoutingMatch {
                            domain_id: domain.id,
                            domain_name: domain.domain_name,
                            address_id: Some(address.id),
                            matched_local_part: route.base_local_part,
                            plus_tag: route.plus_tag,
                        }));
                    }
                    Some(_) => {
                        first_rejection.get_or_insert(InboundRoutingRejection {
                            reason: RoutingRejectionReason::InactiveAddress,
                            recipient: Some(recipient.clone()),
                        });
                    }
                    None => {
                        first_rejection.get_or_insert(InboundRoutingRejection {
                            reason: RoutingRejectionReason::AllowlistMiss,
                            recipient: Some(recipient.clone()),
                        });
                    }
                }
            }
        }
    }

    Ok(RoutingDecision::Rejected(first_rejection.unwrap_or(
        InboundRoutingRejection {
            reason: RoutingRejectionReason::NoRecipients,
            recipient: None,
        },
    )))
}

fn reject(reason: RoutingRejectionReason, recipient: Option<String>) -> RoutingDecision {
    RoutingDecision::Rejected(InboundRoutingRejection { reason, recipient })
}
