INSERT INTO addresses (domain_id, local_part, active)
SELECT id, 'support', true
FROM domains
WHERE domain_name = 'ahara.io'
ON CONFLICT (domain_id, local_part) DO UPDATE SET
    active = EXCLUDED.active,
    updated_at = now();
