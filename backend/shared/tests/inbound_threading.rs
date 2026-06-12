use shared::inbound::limits::IngestLimits;
use shared::inbound::mime::parse_raw_mime;
use shared::inbound::threading::{
    ContactLinkLookup, InMemoryContactLinkLookup, build_thread_seed, normalize_subject,
    participants_json,
};
use uuid::Uuid;

#[test]
fn inbound_threading_normalizes_reply_subjects() {
    assert_eq!(
        normalize_subject(" Re: FWD:  Project   Thread "),
        "project thread"
    );
    assert_eq!(normalize_subject("FW:Re:Invoice"), "invoice");
}

#[test]
fn inbound_threading_builds_seed_from_references_and_participants() {
    let message = parse_raw_mime(
        include_bytes!("fixtures/inbound/threaded_reply.eml"),
        IngestLimits::default(),
    )
    .unwrap();
    let seed = build_thread_seed(&message);

    assert_eq!(seed.rfc_message_id.as_deref(), Some("<reply@example.test>"));
    assert_eq!(seed.in_reply_to.as_deref(), Some("<previous@example.test>"));
    assert_eq!(
        seed.reference_ids,
        vec![
            "<root@example.test>".to_string(),
            "<previous@example.test>".to_string()
        ]
    );
    assert_eq!(seed.normalized_subject, "project thread");
    assert_eq!(
        seed.participants,
        vec![
            "sender@example.test".to_string(),
            "contact@ahara.io".to_string(),
            "chris@ahara.io".to_string(),
            "support@ahara.io".to_string(),
        ]
    );
    assert_eq!(
        participants_json(&seed.participants).to_string(),
        "[\"sender@example.test\",\"contact@ahara.io\",\"chris@ahara.io\",\"support@ahara.io\"]"
    );
}

#[tokio::test]
async fn inbound_threading_links_contact_by_normalized_sender_address() {
    let contact_id = Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap();
    let lookup =
        InMemoryContactLinkLookup::with_contacts([("sender@example.test".to_string(), contact_id)]);
    let message = parse_raw_mime(
        include_bytes!("fixtures/inbound/threaded_reply.eml"),
        IngestLimits::default(),
    )
    .unwrap();

    assert_eq!(
        lookup
            .find_contact_id_by_sender(&message.from)
            .await
            .unwrap(),
        Some(contact_id)
    );
}

#[tokio::test]
async fn inbound_threading_does_not_match_spoofed_display_name_to_contact() {
    let contact_id = Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap();
    let lookup =
        InMemoryContactLinkLookup::with_contacts([("contact@ahara.io".to_string(), contact_id)]);
    let message = parse_raw_mime(
        include_bytes!("fixtures/inbound/display_name_spoof.eml"),
        IngestLimits::default(),
    )
    .unwrap();

    assert_eq!(message.from.display_name, "contact@ahara.io");
    assert_eq!(
        lookup
            .find_contact_id_by_sender(&message.from)
            .await
            .unwrap(),
        None
    );
}
