DROP TABLE IF EXISTS outbound_attachment_payloads;

DROP INDEX IF EXISTS idx_messages_raw_retention_due;

DROP INDEX IF EXISTS idx_forwarding_rules_address_target;
DROP INDEX IF EXISTS idx_forwarding_rules_domain_target;

ALTER TABLE forwarding_rules
    DROP CONSTRAINT IF EXISTS forwarding_rules_plus_tag_lowercase,
    DROP CONSTRAINT IF EXISTS forwarding_rules_sender_address_normalized_lowercase,
    DROP COLUMN IF EXISTS require_auth_pass,
    DROP COLUMN IF EXISTS plus_tag,
    DROP COLUMN IF EXISTS sender_address_normalized;

CREATE UNIQUE INDEX idx_forwarding_rules_domain_target
    ON forwarding_rules (domain_id, target_address_normalized)
    WHERE rule_kind = 'domain';

CREATE UNIQUE INDEX idx_forwarding_rules_address_target
    ON forwarding_rules (address_id, target_address_normalized)
    WHERE rule_kind = 'address';

ALTER TABLE messages
    DROP COLUMN IF EXISTS raw_deleted_at,
    DROP COLUMN IF EXISTS raw_retained_until;

ALTER TABLE addresses
    DROP CONSTRAINT IF EXISTS addresses_raw_retention_days_valid,
    DROP COLUMN IF EXISTS raw_retention_days;

ALTER TABLE domains
    DROP CONSTRAINT IF EXISTS domains_raw_retention_days_valid,
    DROP COLUMN IF EXISTS raw_retention_days;
