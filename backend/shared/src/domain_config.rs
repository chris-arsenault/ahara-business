use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::routing::{RoutingPolicy, parse_route};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainConfig {
    pub domain_name: String,
    pub routing_policy: RoutingPolicy,
    pub active: bool,
    pub addresses: Vec<AcceptedAddress>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedAddress {
    pub local_part: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateDomainRequest {
    pub routing_policy: Option<RoutingPolicy>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateAddressRequest {
    pub local_part: String,
}

#[async_trait]
pub trait DomainConfigService: Send + Sync {
    async fn list_domains(&self) -> AppResult<Vec<DomainConfig>>;
    async fn update_domain(
        &self,
        domain_name: &str,
        request: UpdateDomainRequest,
    ) -> AppResult<DomainConfig>;
    async fn upsert_address(
        &self,
        domain_name: &str,
        request: CreateAddressRequest,
    ) -> AppResult<AcceptedAddress>;
    async fn deactivate_address(
        &self,
        domain_name: &str,
        local_part: &str,
    ) -> AppResult<AcceptedAddress>;
}

#[derive(Debug, Clone)]
pub struct PgDomainConfigService {
    pool: DbPool,
}

impl PgDomainConfigService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DomainConfigService for PgDomainConfigService {
    async fn list_domains(&self) -> AppResult<Vec<DomainConfig>> {
        let rows: Vec<DomainRow> = sqlx::query_as(
            "SELECT domain_name, routing_policy, active FROM domains ORDER BY domain_name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        let mut domains = Vec::with_capacity(rows.len());
        for row in rows {
            let addresses: Vec<AddressRow> = sqlx::query_as(
                "SELECT addresses.local_part, addresses.active
                 FROM addresses
                 JOIN domains ON domains.id = addresses.domain_id
                 WHERE domains.domain_name = $1
                 ORDER BY addresses.local_part",
            )
            .bind(&row.domain_name)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;

            domains.push(row.into_domain_config(addresses)?);
        }

        Ok(domains)
    }

    async fn update_domain(
        &self,
        domain_name: &str,
        request: UpdateDomainRequest,
    ) -> AppResult<DomainConfig> {
        let domain_name = normalize_domain_name(domain_name)?;
        let routing_policy = request.routing_policy.map(RoutingPolicy::as_db_value);
        let row: DomainRow = sqlx::query_as(
            "UPDATE domains
             SET routing_policy = COALESCE($2, routing_policy),
                 active = COALESCE($3, active),
                 updated_at = now()
             WHERE domain_name = $1
             RETURNING domain_name, routing_policy, active",
        )
        .bind(&domain_name)
        .bind(routing_policy)
        .bind(request.active)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("domain {domain_name}")))?;

        let addresses: Vec<AddressRow> = sqlx::query_as(
            "SELECT addresses.local_part, addresses.active
             FROM addresses
             JOIN domains ON domains.id = addresses.domain_id
             WHERE domains.domain_name = $1
             ORDER BY addresses.local_part",
        )
        .bind(&domain_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        row.into_domain_config(addresses)
    }

    async fn upsert_address(
        &self,
        domain_name: &str,
        request: CreateAddressRequest,
    ) -> AppResult<AcceptedAddress> {
        let domain_name = normalize_domain_name(domain_name)?;
        let local_part = normalize_local_part(&domain_name, &request.local_part)?;
        let row: AddressRow = sqlx::query_as(
            "WITH target_domain AS (
                 SELECT id FROM domains WHERE domain_name = $1
             )
             INSERT INTO addresses (domain_id, local_part, active)
             SELECT id, $2, true FROM target_domain
             ON CONFLICT (domain_id, local_part) DO UPDATE SET
                 active = true,
                 updated_at = now()
             RETURNING local_part, active",
        )
        .bind(&domain_name)
        .bind(&local_part)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("domain {domain_name}")))?;

        Ok(row.into())
    }

    async fn deactivate_address(
        &self,
        domain_name: &str,
        local_part: &str,
    ) -> AppResult<AcceptedAddress> {
        let domain_name = normalize_domain_name(domain_name)?;
        let local_part = normalize_local_part(&domain_name, local_part)?;
        let row: AddressRow = sqlx::query_as(
            "UPDATE addresses
             SET active = false,
                 updated_at = now()
             FROM domains
             WHERE addresses.domain_id = domains.id
               AND domains.domain_name = $1
               AND addresses.local_part = $2
             RETURNING addresses.local_part, addresses.active",
        )
        .bind(&domain_name)
        .bind(&local_part)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("address {local_part}@{domain_name}")))?;

        Ok(row.into())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct DomainRow {
    domain_name: String,
    routing_policy: String,
    active: bool,
}

impl DomainRow {
    fn into_domain_config(self, addresses: Vec<AddressRow>) -> AppResult<DomainConfig> {
        Ok(DomainConfig {
            domain_name: self.domain_name,
            routing_policy: RoutingPolicy::from_str(&self.routing_policy)
                .map_err(|err| AppError::Internal(err.to_string()))?,
            active: self.active,
            addresses: addresses.into_iter().map(Into::into).collect(),
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct AddressRow {
    local_part: String,
    active: bool,
}

impl From<AddressRow> for AcceptedAddress {
    fn from(value: AddressRow) -> Self {
        Self {
            local_part: value.local_part,
            active: value.active,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryDomainConfigService {
    domains: Arc<Mutex<BTreeMap<String, DomainConfig>>>,
}

impl InMemoryDomainConfigService {
    pub fn with_domains(domains: impl IntoIterator<Item = DomainConfig>) -> Self {
        let domains = domains
            .into_iter()
            .map(|domain| (domain.domain_name.clone(), domain))
            .collect();
        Self {
            domains: Arc::new(Mutex::new(domains)),
        }
    }
}

#[async_trait]
impl DomainConfigService for InMemoryDomainConfigService {
    async fn list_domains(&self) -> AppResult<Vec<DomainConfig>> {
        Ok(self.domains.lock().unwrap().values().cloned().collect())
    }

    async fn update_domain(
        &self,
        domain_name: &str,
        request: UpdateDomainRequest,
    ) -> AppResult<DomainConfig> {
        let domain_name = normalize_domain_name(domain_name)?;
        let mut domains = self.domains.lock().unwrap();
        let domain = domains
            .get_mut(&domain_name)
            .ok_or_else(|| AppError::NotFound(format!("domain {domain_name}")))?;

        if let Some(routing_policy) = request.routing_policy {
            domain.routing_policy = routing_policy;
        }
        if let Some(active) = request.active {
            domain.active = active;
        }

        Ok(domain.clone())
    }

    async fn upsert_address(
        &self,
        domain_name: &str,
        request: CreateAddressRequest,
    ) -> AppResult<AcceptedAddress> {
        let domain_name = normalize_domain_name(domain_name)?;
        let local_part = normalize_local_part(&domain_name, &request.local_part)?;
        let mut domains = self.domains.lock().unwrap();
        let domain = domains
            .get_mut(&domain_name)
            .ok_or_else(|| AppError::NotFound(format!("domain {domain_name}")))?;

        if let Some(address) = domain
            .addresses
            .iter_mut()
            .find(|address| address.local_part == local_part)
        {
            address.active = true;
            return Ok(address.clone());
        }

        let address = AcceptedAddress {
            local_part,
            active: true,
        };
        domain.addresses.push(address.clone());
        domain
            .addresses
            .sort_by(|left, right| left.local_part.cmp(&right.local_part));
        Ok(address)
    }

    async fn deactivate_address(
        &self,
        domain_name: &str,
        local_part: &str,
    ) -> AppResult<AcceptedAddress> {
        let domain_name = normalize_domain_name(domain_name)?;
        let local_part = normalize_local_part(&domain_name, local_part)?;
        let mut domains = self.domains.lock().unwrap();
        let domain = domains
            .get_mut(&domain_name)
            .ok_or_else(|| AppError::NotFound(format!("domain {domain_name}")))?;
        let address = domain
            .addresses
            .iter_mut()
            .find(|address| address.local_part == local_part)
            .ok_or_else(|| AppError::NotFound(format!("address {local_part}@{domain_name}")))?;

        address.active = false;
        Ok(address.clone())
    }
}

fn normalize_domain_name(domain_name: &str) -> AppResult<String> {
    let domain_name = domain_name.trim().to_ascii_lowercase();
    if domain_name.is_empty() {
        return Err(AppError::Validation("domain name is required".to_string()));
    }
    Ok(domain_name)
}

fn normalize_local_part(domain_name: &str, local_part: &str) -> AppResult<String> {
    let route = parse_route(&format!("{local_part}@{domain_name}"))
        .map_err(|err| AppError::Validation(err.to_string()))?;
    if route.plus_tag.is_some() {
        return Err(AppError::Validation(
            "accepted address local part cannot include a plus tag".to_string(),
        ));
    }
    if route.domain != domain_name {
        return Err(AppError::Validation(
            "accepted address domain does not match route domain".to_string(),
        ));
    }
    Ok(route.base_local_part)
}

#[cfg(test)]
mod tests {
    use super::{
        AcceptedAddress, CreateAddressRequest, DomainConfig, DomainConfigService,
        InMemoryDomainConfigService, UpdateDomainRequest,
    };
    use crate::error::AppError;
    use crate::routing::RoutingPolicy;

    fn service() -> InMemoryDomainConfigService {
        InMemoryDomainConfigService::with_domains([DomainConfig {
            domain_name: "ahara.io".to_string(),
            routing_policy: RoutingPolicy::Allowlist,
            active: true,
            addresses: vec![
                AcceptedAddress {
                    local_part: "chris".to_string(),
                    active: true,
                },
                AcceptedAddress {
                    local_part: "contact".to_string(),
                    active: false,
                },
            ],
        }])
    }

    #[tokio::test]
    async fn domain_config_lists_configured_domains() {
        let domains = service().list_domains().await.unwrap();

        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0].domain_name, "ahara.io");
        assert_eq!(domains[0].addresses.len(), 2);
    }

    #[tokio::test]
    async fn domain_config_updates_policy_and_active_flag() {
        let updated = service()
            .update_domain(
                "AHARA.IO",
                UpdateDomainRequest {
                    routing_policy: Some(RoutingPolicy::Catchall),
                    active: Some(false),
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.routing_policy, RoutingPolicy::Catchall);
        assert!(!updated.active);
    }

    #[tokio::test]
    async fn domain_config_adds_and_reactivates_addresses() {
        let service = service();
        let added = service
            .upsert_address(
                "ahara.io",
                CreateAddressRequest {
                    local_part: "Support".to_string(),
                },
            )
            .await
            .unwrap();
        let reactivated = service
            .upsert_address(
                "ahara.io",
                CreateAddressRequest {
                    local_part: "contact".to_string(),
                },
            )
            .await
            .unwrap();

        assert_eq!(added.local_part, "support");
        assert!(added.active);
        assert_eq!(reactivated.local_part, "contact");
        assert!(reactivated.active);
    }

    #[tokio::test]
    async fn domain_config_deactivates_addresses() {
        let deactivated = service()
            .deactivate_address("ahara.io", "Chris")
            .await
            .unwrap();

        assert_eq!(deactivated.local_part, "chris");
        assert!(!deactivated.active);
    }

    #[tokio::test]
    async fn domain_config_rejects_invalid_accepted_address() {
        let err = service()
            .upsert_address(
                "ahara.io",
                CreateAddressRequest {
                    local_part: "contact+tag".to_string(),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }
}
