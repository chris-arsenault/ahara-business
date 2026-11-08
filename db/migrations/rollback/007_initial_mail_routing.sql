-- Retain operational routing configuration on rollback. Removing it would stop
-- delivery and cannot distinguish pre-existing routes from migration inserts.
SELECT 1;
