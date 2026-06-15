use super::{
    AcceptedAddress, CreateAddressRequest, CreateDomainRequest, DomainConfig, DomainConfigService,
    InMemoryDomainConfigService, UpdateAddressRequest, UpdateDomainRequest,
};
use crate::error::AppError;
use crate::routing::RoutingPolicy;

fn service() -> InMemoryDomainConfigService {
    InMemoryDomainConfigService::with_domains([DomainConfig {
        domain_name: "ahara.io".to_string(),
        routing_policy: RoutingPolicy::Allowlist,
        active: true,
        raw_retention_days: Some(90),
        addresses: vec![
            AcceptedAddress {
                local_part: "chris".to_string(),
                active: true,
                raw_retention_days: None,
            },
            AcceptedAddress {
                local_part: "contact".to_string(),
                active: false,
                raw_retention_days: Some(30),
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
async fn domain_config_upserts_domain_without_resetting_omitted_fields() {
    let service = service();
    let created = service
        .upsert_domain(CreateDomainRequest {
            domain_name: "Example.TEST".to_string(),
            routing_policy: Some(RoutingPolicy::Catchall),
            active: Some(false),
            raw_retention_days: Some(Some(30)),
        })
        .await
        .unwrap();
    let preserved = service
        .upsert_domain(CreateDomainRequest {
            domain_name: "example.test".to_string(),
            routing_policy: None,
            active: None,
            raw_retention_days: None,
        })
        .await
        .unwrap();

    assert_eq!(created.domain_name, "example.test");
    assert_eq!(created.routing_policy, RoutingPolicy::Catchall);
    assert!(!created.active);
    assert_eq!(created.raw_retention_days, Some(30));
    assert_eq!(preserved, created);
}

#[tokio::test]
async fn domain_config_updates_policy_and_active_flag() {
    let updated = service()
        .update_domain(
            "AHARA.IO",
            UpdateDomainRequest {
                routing_policy: Some(RoutingPolicy::Catchall),
                active: Some(false),
                raw_retention_days: Some(Some(180)),
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.routing_policy, RoutingPolicy::Catchall);
    assert!(!updated.active);
    assert_eq!(updated.raw_retention_days, Some(180));
}

#[tokio::test]
async fn domain_config_updates_and_clears_raw_retention_days() {
    let service = service();
    let cleared = service
        .update_domain(
            "ahara.io",
            UpdateDomainRequest {
                routing_policy: None,
                active: None,
                raw_retention_days: Some(None),
            },
        )
        .await
        .unwrap();
    let address = service
        .update_address(
            "ahara.io",
            "contact",
            UpdateAddressRequest {
                active: Some(true),
                raw_retention_days: Some(None),
            },
        )
        .await
        .unwrap();

    assert_eq!(cleared.raw_retention_days, None);
    assert!(address.active);
    assert_eq!(address.raw_retention_days, None);
}

#[tokio::test]
async fn domain_config_adds_and_reactivates_addresses() {
    let service = service();
    let added = service
        .upsert_address(
            "ahara.io",
            CreateAddressRequest {
                local_part: "Support".to_string(),
                raw_retention_days: Some(14),
            },
        )
        .await
        .unwrap();
    let reactivated = service
        .upsert_address(
            "ahara.io",
            CreateAddressRequest {
                local_part: "contact".to_string(),
                raw_retention_days: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(added.local_part, "support");
    assert!(added.active);
    assert_eq!(added.raw_retention_days, Some(14));
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
                raw_retention_days: None,
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::Validation(_)));
}
