-- S3 references may already own sent or queued mail. Restore an older application
-- only after converting those payloads; dropping these columns would lose mail.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM outbound_attachment_payloads WHERE object_key IS NOT NULL) THEN
        RAISE EXCEPTION 'S3 attachment payloads must be converted before rollback';
    END IF;
END $$;
ALTER TABLE outbound_attachment_payloads
    DROP CONSTRAINT outbound_attachment_payload_source,
    DROP COLUMN version_id,
    DROP COLUMN object_key,
    ALTER COLUMN content_base64 SET NOT NULL;
DROP TABLE attachment_uploads;
