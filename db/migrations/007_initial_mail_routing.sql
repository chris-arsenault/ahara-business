-- Required application routing belongs in the normal migration path.
-- Preserve operator changes to existing domains and addresses.
INSERT INTO domains (domain_name, routing_policy, active)
VALUES ('ahara.io', 'allowlist', true)
ON CONFLICT (domain_name) DO NOTHING;

INSERT INTO addresses (domain_id, local_part, active)
SELECT domains.id, accepted.local_part, true
FROM domains
CROSS JOIN (VALUES ('chris'), ('contact'), ('support')) AS accepted(local_part)
WHERE domains.domain_name = 'ahara.io'
ON CONFLICT (domain_id, local_part) DO NOTHING;
