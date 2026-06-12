ALTER TABLE domains
    ADD COLUMN raw_retention_days INTEGER;

ALTER TABLE domains
    ADD CONSTRAINT domains_raw_retention_days_valid
        CHECK (raw_retention_days IS NULL OR raw_retention_days BETWEEN 1 AND 3650);

ALTER TABLE addresses
    ADD COLUMN raw_retention_days INTEGER;

ALTER TABLE addresses
    ADD CONSTRAINT addresses_raw_retention_days_valid
        CHECK (raw_retention_days IS NULL OR raw_retention_days BETWEEN 1 AND 3650);

ALTER TABLE messages
    ADD COLUMN raw_retained_until TIMESTAMPTZ,
    ADD COLUMN raw_deleted_at TIMESTAMPTZ;

ALTER TABLE forwarding_rules
    ADD COLUMN sender_address_normalized TEXT,
    ADD COLUMN plus_tag TEXT,
    ADD COLUMN require_auth_pass BOOLEAN NOT NULL DEFAULT true;

ALTER TABLE forwarding_rules
    ADD CONSTRAINT forwarding_rules_sender_address_normalized_lowercase
        CHECK (sender_address_normalized IS NULL OR sender_address_normalized = lower(sender_address_normalized)),
    ADD CONSTRAINT forwarding_rules_plus_tag_lowercase
        CHECK (plus_tag IS NULL OR plus_tag = lower(plus_tag));

DROP INDEX idx_forwarding_rules_domain_target;
DROP INDEX idx_forwarding_rules_address_target;

CREATE UNIQUE INDEX idx_forwarding_rules_domain_target
    ON forwarding_rules (
        domain_id,
        target_address_normalized,
        COALESCE(sender_address_normalized, ''),
        COALESCE(plus_tag, ''),
        require_auth_pass
    )
    WHERE rule_kind = 'domain';

CREATE UNIQUE INDEX idx_forwarding_rules_address_target
    ON forwarding_rules (
        address_id,
        target_address_normalized,
        COALESCE(sender_address_normalized, ''),
        COALESCE(plus_tag, ''),
        require_auth_pass
    )
    WHERE rule_kind = 'address';

CREATE INDEX idx_messages_raw_retention_due
    ON messages (raw_retained_until)
    WHERE direction = 'inbound'
      AND s3_raw_key IS NOT NULL
      AND raw_deleted_at IS NULL
      AND raw_retained_until IS NOT NULL;

CREATE TABLE outbound_attachment_payloads (
    attachment_id UUID PRIMARY KEY REFERENCES attachment_refs(id) ON DELETE CASCADE,
    content_base64 TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT outbound_attachment_payloads_content_not_empty CHECK (content_base64 <> '')
);
