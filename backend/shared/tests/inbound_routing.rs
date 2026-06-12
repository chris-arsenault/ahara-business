use shared::inbound::routing::{
    InMemoryInboundRoutingLookup, InboundRoutingAddress, InboundRoutingDomain, RoutingDecision,
    RoutingRejectionReason, resolve_inbound_route,
};
use shared::routing::RoutingPolicy;
use uuid::Uuid;

fn domain(policy: RoutingPolicy, active: bool) -> InboundRoutingDomain {
    InboundRoutingDomain {
        id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
        domain_name: "ahara.io".to_string(),
        routing_policy: policy,
        active,
        addresses: vec![
            InboundRoutingAddress {
                id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
                local_part: "contact".to_string(),
                active: true,
            },
            InboundRoutingAddress {
                id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
                local_part: "chris".to_string(),
                active: false,
            },
        ],
    }
}

#[tokio::test]
async fn inbound_routing_accepts_allowlist_addresses_and_retains_plus_tag() {
    let lookup =
        InMemoryInboundRoutingLookup::with_domains([domain(RoutingPolicy::Allowlist, true)]);
    let decision = resolve_inbound_route(&["Contact+Sales-Q2@Ahara.IO".to_string()], &lookup)
        .await
        .unwrap();

    let RoutingDecision::Accepted(route) = decision else {
        panic!("expected accepted route");
    };
    assert_eq!(route.domain_name, "ahara.io");
    assert_eq!(route.matched_local_part, "contact");
    assert_eq!(route.plus_tag.as_deref(), Some("Sales-Q2"));
    assert_eq!(
        route.address_id,
        Some(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap())
    );
}

#[tokio::test]
async fn inbound_routing_accepts_catchall_domain() {
    let lookup =
        InMemoryInboundRoutingLookup::with_domains([domain(RoutingPolicy::Catchall, true)]);
    let decision = resolve_inbound_route(&["anything@ahara.io".to_string()], &lookup)
        .await
        .unwrap();

    let RoutingDecision::Accepted(route) = decision else {
        panic!("expected accepted route");
    };
    assert_eq!(route.matched_local_part, "anything");
    assert_eq!(route.address_id, None);
}

#[tokio::test]
async fn inbound_routing_rejects_inactive_domain() {
    let lookup =
        InMemoryInboundRoutingLookup::with_domains([domain(RoutingPolicy::Allowlist, false)]);
    let decision = resolve_inbound_route(&["contact@ahara.io".to_string()], &lookup)
        .await
        .unwrap();

    assert_eq!(
        decision,
        RoutingDecision::Rejected(shared::inbound::routing::InboundRoutingRejection {
            reason: RoutingRejectionReason::InactiveDomain,
            recipient: Some("contact@ahara.io".to_string()),
        })
    );
}

#[tokio::test]
async fn inbound_routing_rejects_inactive_address_and_allowlist_miss() {
    let lookup =
        InMemoryInboundRoutingLookup::with_domains([domain(RoutingPolicy::Allowlist, true)]);

    let inactive = resolve_inbound_route(&["chris@ahara.io".to_string()], &lookup)
        .await
        .unwrap();
    let miss = resolve_inbound_route(&["unknown@ahara.io".to_string()], &lookup)
        .await
        .unwrap();

    assert!(matches!(
        inactive,
        RoutingDecision::Rejected(rejection)
            if rejection.reason == RoutingRejectionReason::InactiveAddress
    ));
    assert!(matches!(
        miss,
        RoutingDecision::Rejected(rejection)
            if rejection.reason == RoutingRejectionReason::AllowlistMiss
    ));
}

#[tokio::test]
async fn inbound_routing_rejects_unknown_and_malformed_recipients() {
    let lookup =
        InMemoryInboundRoutingLookup::with_domains([domain(RoutingPolicy::Allowlist, true)]);

    let unknown = resolve_inbound_route(&["contact@example.test".to_string()], &lookup)
        .await
        .unwrap();
    let malformed = resolve_inbound_route(&["not-an-address".to_string()], &lookup)
        .await
        .unwrap();

    assert!(matches!(
        unknown,
        RoutingDecision::Rejected(rejection)
            if rejection.reason == RoutingRejectionReason::UnknownDomain
    ));
    assert!(matches!(
        malformed,
        RoutingDecision::Rejected(rejection)
            if rejection.reason == RoutingRejectionReason::MalformedRecipient
    ));
}

#[tokio::test]
async fn inbound_routing_uses_first_matching_recipient_deterministically() {
    let lookup =
        InMemoryInboundRoutingLookup::with_domains([domain(RoutingPolicy::Allowlist, true)]);
    let decision = resolve_inbound_route(
        &[
            "unknown@example.test".to_string(),
            "contact@ahara.io".to_string(),
            "anything@ahara.io".to_string(),
        ],
        &lookup,
    )
    .await
    .unwrap();

    let RoutingDecision::Accepted(route) = decision else {
        panic!("expected accepted route");
    };
    assert_eq!(route.matched_local_part, "contact");
}
