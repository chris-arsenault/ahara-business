use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::de::{DeserializeOwned, Deserializer};
use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::routing::{RoutingPolicy, parse_route};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainConfig {
    pub domain_name: String,
    pub routing_policy: RoutingPolicy,
    pub active: bool,
    pub raw_retention_days: Option<i32>,
    pub addresses: Vec<AcceptedAddress>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedAddress {
    pub local_part: String,
    pub active: bool,
    pub raw_retention_days: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateDomainRequest {
    pub routing_policy: Option<RoutingPolicy>,
    pub active: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_option")]
    pub raw_retention_days: Option<Option<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateDomainRequest {
    pub domain_name: String,
    pub routing_policy: Option<RoutingPolicy>,
    pub active: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_option")]
    pub raw_retention_days: Option<Option<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateAddressRequest {
    pub local_part: String,
    pub raw_retention_days: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateAddressRequest {
    pub active: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_option")]
    pub raw_retention_days: Option<Option<i32>>,
}

#[async_trait]
pub trait DomainConfigService: Send + Sync {
    async fn list_domains(&self) -> AppResult<Vec<DomainConfig>>;
    async fn upsert_domain(&self, request: CreateDomainRequest) -> AppResult<DomainConfig>;
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
    async fn update_address(
        &self,
        domain_name: &str,
        local_part: &str,
        request: UpdateAddressRequest,
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
            "SELECT domain_name, routing_policy, active, raw_retention_days
             FROM domains
             ORDER BY domain_name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        let mut domains = Vec::with_capacity(rows.len());
        for row in rows {
            let addresses: Vec<AddressRow> = sqlx::query_as(
                "SELECT addresses.local_part, addresses.active, addresses.raw_retention_days
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

    async fn upsert_domain(&self, request: CreateDomainRequest) -> AppResult<DomainConfig> {
        let domain_name = normalize_domain_name(&request.domain_name)?;
        validate_retention_days(request.raw_retention_days.flatten())?;
        let routing_policy = request.routing_policy.map(RoutingPolicy::as_db_value);
        let update_raw_retention_days = request.raw_retention_days.is_some();
        let raw_retention_days = request.raw_retention_days.flatten();
        let row: DomainRow = sqlx::query_as(
            "INSERT INTO domains (domain_name, routing_policy, active, raw_retention_days)
             VALUES ($1, COALESCE($2, 'allowlist'), COALESCE($3, true), $5)
             ON CONFLICT (domain_name) DO UPDATE SET
                 routing_policy = COALESCE($2, domains.routing_policy),
                 active = COALESCE($3, domains.active),
                 raw_retention_days = CASE
                     WHEN $4 THEN EXCLUDED.raw_retention_days
                     ELSE domains.raw_retention_days
                 END,
                 updated_at = now()
             RETURNING domain_name, routing_policy, active, raw_retention_days",
        )
        .bind(&domain_name)
        .bind(routing_policy)
        .bind(request.active)
        .bind(update_raw_retention_days)
        .bind(raw_retention_days)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        let addresses: Vec<AddressRow> = sqlx::query_as(
            "SELECT addresses.local_part, addresses.active, addresses.raw_retention_days
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

    async fn update_domain(
        &self,
        domain_name: &str,
        request: UpdateDomainRequest,
    ) -> AppResult<DomainConfig> {
        let domain_name = normalize_domain_name(domain_name)?;
        validate_retention_days(request.raw_retention_days.flatten())?;
        let routing_policy = request.routing_policy.map(RoutingPolicy::as_db_value);
        let update_raw_retention_days = request.raw_retention_days.is_some();
        let raw_retention_days = request.raw_retention_days.flatten();
        let row: DomainRow = sqlx::query_as(
            "UPDATE domains
             SET routing_policy = COALESCE($2, routing_policy),
                 active = COALESCE($3, active),
                 raw_retention_days = CASE WHEN $4 THEN $5 ELSE raw_retention_days END,
                 updated_at = now()
             WHERE domain_name = $1
             RETURNING domain_name, routing_policy, active, raw_retention_days",
        )
        .bind(&domain_name)
        .bind(routing_policy)
        .bind(request.active)
        .bind(update_raw_retention_days)
        .bind(raw_retention_days)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("domain {domain_name}")))?;

        let addresses: Vec<AddressRow> = sqlx::query_as(
            "SELECT addresses.local_part, addresses.active, addresses.raw_retention_days
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
        validate_retention_days(request.raw_retention_days)?;
        let row: AddressRow = sqlx::query_as(
            "WITH target_domain AS (
                 SELECT id FROM domains WHERE domain_name = $1
             )
             INSERT INTO addresses (domain_id, local_part, active, raw_retention_days)
             SELECT id, $2, true, $3 FROM target_domain
             ON CONFLICT (domain_id, local_part) DO UPDATE SET
                 active = true,
                 raw_retention_days = COALESCE($3, addresses.raw_retention_days),
                 updated_at = now()
             RETURNING local_part, active, raw_retention_days",
        )
        .bind(&domain_name)
        .bind(&local_part)
        .bind(request.raw_retention_days)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("domain {domain_name}")))?;

        Ok(row.into())
    }

    async fn update_address(
        &self,
        domain_name: &str,
        local_part: &str,
        request: UpdateAddressRequest,
    ) -> AppResult<AcceptedAddress> {
        let domain_name = normalize_domain_name(domain_name)?;
        let local_part = normalize_local_part(&domain_name, local_part)?;
        validate_retention_days(request.raw_retention_days.flatten())?;
        let update_raw_retention_days = request.raw_retention_days.is_some();
        let raw_retention_days = request.raw_retention_days.flatten();
        let row: AddressRow = sqlx::query_as(
            "UPDATE addresses
             SET active = COALESCE($3, addresses.active),
                 raw_retention_days = CASE WHEN $4 THEN $5 ELSE raw_retention_days END,
                 updated_at = now()
             FROM domains
             WHERE addresses.domain_id = domains.id
               AND domains.domain_name = $1
               AND addresses.local_part = $2
             RETURNING addresses.local_part, addresses.active, addresses.raw_retention_days",
        )
        .bind(&domain_name)
        .bind(&local_part)
        .bind(request.active)
        .bind(update_raw_retention_days)
        .bind(raw_retention_days)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("address {local_part}@{domain_name}")))?;

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
             RETURNING addresses.local_part, addresses.active, addresses.raw_retention_days",
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
    raw_retention_days: Option<i32>,
}

impl DomainRow {
    fn into_domain_config(self, addresses: Vec<AddressRow>) -> AppResult<DomainConfig> {
        Ok(DomainConfig {
            domain_name: self.domain_name,
            routing_policy: RoutingPolicy::from_str(&self.routing_policy)
                .map_err(|err| AppError::Internal(err.to_string()))?,
            active: self.active,
            raw_retention_days: self.raw_retention_days,
            addresses: addresses.into_iter().map(Into::into).collect(),
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct AddressRow {
    local_part: String,
    active: bool,
    raw_retention_days: Option<i32>,
}

impl From<AddressRow> for AcceptedAddress {
    fn from(value: AddressRow) -> Self {
        Self {
            local_part: value.local_part,
            active: value.active,
            raw_retention_days: value.raw_retention_days,
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

    async fn upsert_domain(&self, request: CreateDomainRequest) -> AppResult<DomainConfig> {
        let domain_name = normalize_domain_name(&request.domain_name)?;
        validate_retention_days(request.raw_retention_days.flatten())?;
        let mut domains = self.domains.lock().unwrap();
        let domain = domains.entry(domain_name.clone()).or_insert(DomainConfig {
            domain_name,
            routing_policy: RoutingPolicy::Allowlist,
            active: true,
            raw_retention_days: None,
            addresses: Vec::new(),
        });

        if let Some(routing_policy) = request.routing_policy {
            domain.routing_policy = routing_policy;
        }
        if let Some(active) = request.active {
            domain.active = active;
        }
        if let Some(raw_retention_days) = request.raw_retention_days {
            domain.raw_retention_days = raw_retention_days;
        }

        Ok(domain.clone())
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
        if let Some(raw_retention_days) = request.raw_retention_days {
            validate_retention_days(raw_retention_days)?;
            domain.raw_retention_days = raw_retention_days;
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
        validate_retention_days(request.raw_retention_days)?;
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
            if request.raw_retention_days.is_some() {
                address.raw_retention_days = request.raw_retention_days;
            }
            return Ok(address.clone());
        }

        let address = AcceptedAddress {
            local_part,
            active: true,
            raw_retention_days: request.raw_retention_days,
        };
        domain.addresses.push(address.clone());
        domain
            .addresses
            .sort_by(|left, right| left.local_part.cmp(&right.local_part));
        Ok(address)
    }

    async fn update_address(
        &self,
        domain_name: &str,
        local_part: &str,
        request: UpdateAddressRequest,
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
        if let Some(active) = request.active {
            address.active = active;
        }
        if let Some(raw_retention_days) = request.raw_retention_days {
            validate_retention_days(raw_retention_days)?;
            address.raw_retention_days = raw_retention_days;
        }
        Ok(address.clone())
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
    let route = parse_route(&format!("postmaster@{domain_name}"))
        .map_err(|err| AppError::Validation(err.to_string()))?;
    if route.domain != domain_name {
        return Err(AppError::Validation("domain name is invalid".to_string()));
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

fn validate_retention_days(value: Option<i32>) -> AppResult<()> {
    if let Some(days) = value
        && !(1..=3650).contains(&days)
    {
        return Err(AppError::Validation(
            "raw retention days must be between 1 and 3650".to_string(),
        ));
    }
    Ok(())
}

fn deserialize_optional_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests;
