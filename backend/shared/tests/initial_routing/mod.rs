use super::{run_psql, scalar_i64, setup_postgres};
use shared::db::MAIL_MODEL_MIGRATION;

const MIGRATION: &str = include_str!("../../../../db/migrations/007_initial_mail_routing.sql");
const ROLLBACK: &str =
    include_str!("../../../../db/migrations/rollback/007_initial_mail_routing.sql");

fn previous_migrations() -> &'static str {
    MAIL_MODEL_MIGRATION.split_once(MIGRATION).unwrap().0
}

#[test]
fn normal_migrations_install_initial_routes_without_a_seed() {
    let container = setup_postgres();
    run_psql(&container.name, previous_migrations());
    assert_eq!(
        scalar_i64(&container.name, "SELECT count(*) FROM domains;"),
        0
    );

    run_psql(&container.name, MIGRATION);
    run_psql(&container.name, MIGRATION);
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*) FROM domains
             WHERE domain_name = 'ahara.io' AND active AND routing_policy = 'allowlist';",
        ),
        1
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*) FROM addresses a JOIN domains d ON d.id = a.domain_id
             WHERE d.domain_name = 'ahara.io' AND a.active
               AND a.local_part IN ('chris', 'contact', 'support');",
        ),
        3
    );

    run_psql(&container.name, ROLLBACK);
    assert_eq!(
        scalar_i64(&container.name, "SELECT count(*) FROM addresses;"),
        3
    );
}

#[test]
fn routing_upgrade_preserves_operator_settings_and_adds_only_missing_addresses() {
    let container = setup_postgres();
    run_psql(&container.name, previous_migrations());
    run_psql(
        &container.name,
        "INSERT INTO domains (domain_name, routing_policy, active, raw_retention_days)
         VALUES ('ahara.io', 'catchall', false, 90), ('other.test', 'allowlist', true, 30);
         INSERT INTO addresses (domain_id, local_part, active, raw_retention_days)
         SELECT id, local_part, false, 14 FROM domains
         CROSS JOIN (VALUES ('contact'), ('support'), ('custom')) AS parts(local_part)
         WHERE domain_name = 'ahara.io';",
    );
    let existing = run_psql(
        &container.name,
        "SELECT row_to_json(d) FROM domains d ORDER BY domain_name;
         SELECT row_to_json(a) FROM addresses a ORDER BY local_part;",
    );

    run_psql(&container.name, MIGRATION);
    run_psql(&container.name, MIGRATION);
    assert_eq!(
        run_psql(
            &container.name,
            "SELECT row_to_json(d) FROM domains d ORDER BY domain_name;
             SELECT row_to_json(a) FROM addresses a WHERE local_part <> 'chris'
             ORDER BY local_part;",
        ),
        existing
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*) FROM addresses a JOIN domains d ON d.id = a.domain_id
             WHERE d.domain_name = 'ahara.io' AND a.local_part = 'chris' AND a.active;",
        ),
        1
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*) FROM addresses a JOIN domains d ON d.id = a.domain_id
             WHERE d.domain_name = 'other.test';",
        ),
        0
    );
}
