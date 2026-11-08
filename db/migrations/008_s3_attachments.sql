CREATE TABLE attachment_uploads (
    id UUID PRIMARY KEY,
    owner_sub TEXT NOT NULL,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL CHECK (size_bytes BETWEEN 1 AND 15728640),
    checksum_sha256 TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT now() + interval '1 day',
    object_key TEXT,
    version_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((object_key IS NULL) = (version_id IS NULL))
);

ALTER TABLE outbound_attachment_payloads
    ALTER COLUMN content_base64 DROP NOT NULL,
    ADD COLUMN object_key TEXT,
    ADD COLUMN version_id TEXT,
    ADD CONSTRAINT outbound_attachment_payload_source CHECK (
        (content_base64 IS NOT NULL AND object_key IS NULL AND version_id IS NULL)
        OR (content_base64 IS NULL AND object_key IS NOT NULL AND version_id IS NOT NULL)
    );
