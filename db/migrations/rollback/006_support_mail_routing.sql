-- Preserve address references in existing mail and forwarding rules.
UPDATE addresses
SET active = false, updated_at = now()
FROM domains
WHERE addresses.domain_id = domains.id
  AND domains.domain_name = 'ahara.io'
  AND addresses.local_part = 'support';
