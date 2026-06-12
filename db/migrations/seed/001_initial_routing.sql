-- Application-side accepted address routing only.
-- This does not create MX records, SES receipt rules, or inbound delivery routes.

WITH routed_domain AS (
    INSERT INTO domains (domain_name, routing_policy, active)
    VALUES ('ahara.io', 'allowlist', true)
    ON CONFLICT (domain_name) DO UPDATE SET
        routing_policy = EXCLUDED.routing_policy,
        active = EXCLUDED.active,
        updated_at = now()
    RETURNING id
)
INSERT INTO addresses (domain_id, local_part, active)
SELECT routed_domain.id, accepted.local_part, true
FROM routed_domain
CROSS JOIN (
    VALUES
        ('chris'),
        ('contact')
) AS accepted(local_part)
ON CONFLICT (domain_id, local_part) DO UPDATE SET
    active = EXCLUDED.active,
    updated_at = now();
