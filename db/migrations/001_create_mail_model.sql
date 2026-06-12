CREATE TABLE domains (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_name         TEXT NOT NULL UNIQUE,
    routing_policy      TEXT NOT NULL DEFAULT 'allowlist',
    active              BOOLEAN NOT NULL DEFAULT true,
    dkim_status         TEXT NOT NULL DEFAULT 'pending',
    verification_status TEXT NOT NULL DEFAULT 'pending',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT domains_domain_name_lowercase CHECK (domain_name = lower(domain_name)),
    CONSTRAINT domains_domain_name_not_empty CHECK (domain_name <> ''),
    CONSTRAINT domains_routing_policy_valid CHECK (routing_policy IN ('allowlist', 'catchall')),
    CONSTRAINT domains_dkim_status_valid CHECK (dkim_status IN ('pending', 'verified', 'failed', 'disabled')),
    CONSTRAINT domains_verification_status_valid CHECK (verification_status IN ('pending', 'verified', 'failed', 'disabled'))
);

CREATE TABLE addresses (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id  UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    local_part TEXT NOT NULL,
    active     BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT addresses_local_part_lowercase CHECK (local_part = lower(local_part)),
    CONSTRAINT addresses_local_part_not_empty CHECK (local_part <> ''),
    CONSTRAINT addresses_domain_local_part_unique UNIQUE (domain_id, local_part)
);

CREATE TABLE contacts (
    id                         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name               TEXT NOT NULL DEFAULT '',
    primary_address            TEXT,
    primary_address_normalized TEXT,
    notes                      TEXT NOT NULL DEFAULT '',
    created_at                 TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                 TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT contacts_primary_address_normalized_lowercase
        CHECK (primary_address_normalized IS NULL OR primary_address_normalized = lower(primary_address_normalized)),
    CONSTRAINT contacts_primary_address_pair
        CHECK ((primary_address IS NULL) = (primary_address_normalized IS NULL))
);

CREATE TABLE threads (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    normalized_subject TEXT NOT NULL DEFAULT '',
    participants       JSONB NOT NULL DEFAULT '[]'::jsonb,
    last_activity_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    message_count      INTEGER NOT NULL DEFAULT 0,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT threads_message_count_nonnegative CHECK (message_count >= 0)
);

CREATE TABLE messages (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    direction               TEXT NOT NULL,
    ses_message_id          TEXT,
    rfc_message_id          TEXT,
    in_reply_to             TEXT,
    reference_ids           TEXT[] NOT NULL DEFAULT '{}',
    thread_id               UUID REFERENCES threads(id) ON DELETE SET NULL,
    from_address            TEXT NOT NULL,
    from_address_normalized TEXT NOT NULL,
    from_display_name       TEXT NOT NULL DEFAULT '',
    subject                 TEXT NOT NULL DEFAULT '',
    message_date            TIMESTAMPTZ,
    matched_domain_id       UUID REFERENCES domains(id) ON DELETE SET NULL,
    matched_address_id      UUID REFERENCES addresses(id) ON DELETE SET NULL,
    matched_local_part      TEXT,
    plus_tag                TEXT,
    body_text               TEXT NOT NULL DEFAULT '',
    s3_raw_key              TEXT,
    spf_result              TEXT,
    dkim_result             TEXT,
    dmarc_result            TEXT,
    auth_verdict            TEXT,
    spam_result             TEXT,
    virus_result            TEXT,
    security_disposition    TEXT NOT NULL DEFAULT 'accepted',
    security_reason         TEXT,
    contact_id              UUID REFERENCES contacts(id) ON DELETE SET NULL,
    is_read                 BOOLEAN NOT NULL DEFAULT false,
    status                  TEXT NOT NULL DEFAULT 'received',
    send_attempt_count      INTEGER NOT NULL DEFAULT 0,
    next_retry_at           TIMESTAMPTZ,
    last_error              TEXT,
    has_attachments         BOOLEAN NOT NULL DEFAULT false,
    attachment_count        INTEGER NOT NULL DEFAULT 0,
    size_bytes              BIGINT,
    received_at             TIMESTAMPTZ,
    sent_at                 TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT messages_direction_valid CHECK (direction IN ('inbound', 'outbound')),
    CONSTRAINT messages_from_address_not_empty CHECK (from_address <> ''),
    CONSTRAINT messages_from_address_normalized_lowercase CHECK (from_address_normalized = lower(from_address_normalized)),
    CONSTRAINT messages_matched_local_part_lowercase
        CHECK (matched_local_part IS NULL OR matched_local_part = lower(matched_local_part)),
    CONSTRAINT messages_spf_result_valid
        CHECK (spf_result IS NULL OR spf_result IN ('pass', 'fail', 'neutral', 'softfail', 'temperror', 'permerror', 'none')),
    CONSTRAINT messages_dkim_result_valid
        CHECK (dkim_result IS NULL OR dkim_result IN ('pass', 'fail', 'neutral', 'softfail', 'temperror', 'permerror', 'none')),
    CONSTRAINT messages_dmarc_result_valid
        CHECK (dmarc_result IS NULL OR dmarc_result IN ('pass', 'fail', 'neutral', 'softfail', 'temperror', 'permerror', 'none')),
    CONSTRAINT messages_auth_verdict_valid
        CHECK (auth_verdict IS NULL OR auth_verdict IN ('pass', 'fail', 'neutral', 'softfail', 'temperror', 'permerror', 'none')),
    CONSTRAINT messages_spam_result_valid
        CHECK (spam_result IS NULL OR spam_result IN ('pass', 'fail', 'gray', 'processing_failed')),
    CONSTRAINT messages_virus_result_valid
        CHECK (virus_result IS NULL OR virus_result IN ('pass', 'fail', 'gray', 'processing_failed')),
    CONSTRAINT messages_security_disposition_valid
        CHECK (security_disposition IN ('accepted', 'quarantined', 'rejected')),
    CONSTRAINT messages_status_valid CHECK (status IN ('received', 'quarantined', 'rejected', 'queued', 'sending', 'sent', 'failed', 'bounced', 'complained')),
    CONSTRAINT messages_send_attempt_count_nonnegative CHECK (send_attempt_count >= 0),
    CONSTRAINT messages_attachment_count_nonnegative CHECK (attachment_count >= 0),
    CONSTRAINT messages_size_bytes_nonnegative CHECK (size_bytes IS NULL OR size_bytes >= 0)
);

CREATE TABLE recipients (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id         UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    kind               TEXT NOT NULL,
    address            TEXT NOT NULL,
    address_normalized TEXT NOT NULL,
    display_name       TEXT NOT NULL DEFAULT '',
    position           INTEGER NOT NULL DEFAULT 0,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT recipients_kind_valid CHECK (kind IN ('to', 'cc', 'bcc')),
    CONSTRAINT recipients_address_not_empty CHECK (address <> ''),
    CONSTRAINT recipients_address_normalized_lowercase CHECK (address_normalized = lower(address_normalized)),
    CONSTRAINT recipients_position_nonnegative CHECK (position >= 0),
    CONSTRAINT recipients_message_kind_position_unique UNIQUE (message_id, kind, position)
);

CREATE TABLE attachment_refs (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id   UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    position     INTEGER NOT NULL DEFAULT 0,
    filename     TEXT NOT NULL DEFAULT '',
    content_type TEXT NOT NULL DEFAULT 'application/octet-stream',
    size_bytes   BIGINT,
    s3_key       TEXT,
    content_id   TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT attachment_refs_position_nonnegative CHECK (position >= 0),
    CONSTRAINT attachment_refs_size_bytes_nonnegative CHECK (size_bytes IS NULL OR size_bytes >= 0),
    CONSTRAINT attachment_refs_message_position_unique UNIQUE (message_id, position)
);

CREATE TABLE forwarding_rules (
    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_kind                 TEXT NOT NULL,
    domain_id                 UUID REFERENCES domains(id) ON DELETE CASCADE,
    address_id                UUID REFERENCES addresses(id) ON DELETE CASCADE,
    target_address            TEXT NOT NULL,
    target_address_normalized TEXT NOT NULL,
    active                    BOOLEAN NOT NULL DEFAULT true,
    created_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT forwarding_rules_kind_valid CHECK (rule_kind IN ('domain', 'address')),
    CONSTRAINT forwarding_rules_target_address_not_empty CHECK (target_address <> ''),
    CONSTRAINT forwarding_rules_target_address_normalized_lowercase
        CHECK (target_address_normalized = lower(target_address_normalized)),
    CONSTRAINT forwarding_rules_scope_valid CHECK (
        (rule_kind = 'domain' AND domain_id IS NOT NULL AND address_id IS NULL)
        OR (rule_kind = 'address' AND address_id IS NOT NULL)
    )
);

CREATE TABLE suppressions (
    address_normalized TEXT PRIMARY KEY,
    address            TEXT NOT NULL,
    reason             TEXT NOT NULL,
    source_message_id  UUID REFERENCES messages(id) ON DELETE SET NULL,
    notes              TEXT NOT NULL DEFAULT '',
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT suppressions_address_not_empty CHECK (address <> ''),
    CONSTRAINT suppressions_address_normalized_lowercase CHECK (address_normalized = lower(address_normalized)),
    CONSTRAINT suppressions_reason_valid CHECK (reason IN ('bounce', 'complaint', 'manual'))
);

CREATE TABLE outbound_work (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id      UUID NOT NULL UNIQUE REFERENCES messages(id) ON DELETE CASCADE,
    source_message_id UUID REFERENCES messages(id) ON DELETE SET NULL,
    status          TEXT NOT NULL DEFAULT 'queued',
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked_at       TIMESTAMPTZ,
    locked_by       TEXT,
    last_error      TEXT,
    idempotency_key TEXT NOT NULL UNIQUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT outbound_work_status_valid CHECK (status IN ('queued', 'sending', 'sent', 'failed', 'bounced', 'complained')),
    CONSTRAINT outbound_work_attempt_count_nonnegative CHECK (attempt_count >= 0),
    CONSTRAINT outbound_work_idempotency_key_not_empty CHECK (idempotency_key <> '')
);

CREATE UNIQUE INDEX idx_contacts_primary_address_normalized
    ON contacts (primary_address_normalized)
    WHERE primary_address_normalized IS NOT NULL;

CREATE UNIQUE INDEX idx_messages_ses_message_id
    ON messages (ses_message_id)
    WHERE ses_message_id IS NOT NULL;

CREATE UNIQUE INDEX idx_messages_s3_raw_key
    ON messages (s3_raw_key)
    WHERE s3_raw_key IS NOT NULL;

CREATE INDEX idx_addresses_domain_active
    ON addresses (domain_id, active);

CREATE INDEX idx_threads_last_activity
    ON threads (last_activity_at DESC);

CREATE INDEX idx_messages_direction_received_at
    ON messages (direction, received_at DESC);

CREATE INDEX idx_messages_inbound_unread
    ON messages (received_at DESC)
    WHERE direction = 'inbound' AND is_read = false AND security_disposition = 'accepted';

CREATE INDEX idx_messages_security_disposition
    ON messages (security_disposition, received_at DESC)
    WHERE direction = 'inbound';

CREATE INDEX idx_messages_thread_received_at
    ON messages (thread_id, received_at DESC)
    WHERE thread_id IS NOT NULL;

CREATE INDEX idx_messages_contact
    ON messages (contact_id)
    WHERE contact_id IS NOT NULL;

CREATE INDEX idx_recipients_address_normalized
    ON recipients (address_normalized);

CREATE INDEX idx_attachment_refs_message
    ON attachment_refs (message_id);

CREATE UNIQUE INDEX idx_forwarding_rules_domain_target
    ON forwarding_rules (domain_id, target_address_normalized)
    WHERE rule_kind = 'domain';

CREATE UNIQUE INDEX idx_forwarding_rules_address_target
    ON forwarding_rules (address_id, target_address_normalized)
    WHERE rule_kind = 'address';

CREATE INDEX idx_outbound_work_pickup
    ON outbound_work (status, next_attempt_at)
    WHERE status IN ('queued', 'failed');
