use super::{run_psql, scalar_i64, setup_postgres};
use shared::db::{INITIAL_ROUTING_SEED, MAIL_MODEL_MIGRATION};

const MIGRATION: &str = include_str!("../../../../db/migrations/006_support_mail_routing.sql");
const ROLLBACK: &str =
    include_str!("../../../../db/migrations/rollback/006_support_mail_routing.sql");

#[test]
fn support_route_upgrade_is_idempotent_and_preserves_existing_addresses() {
    let container = setup_postgres();
    run_psql(&container.name, MAIL_MODEL_MIGRATION);
    run_psql(&container.name, INITIAL_ROUTING_SEED);
    run_psql(
        &container.name,
        "DELETE FROM addresses WHERE local_part = 'support';",
    );

    run_psql(&container.name, MIGRATION);
    let address_id = run_psql(
        &container.name,
        "SELECT id FROM addresses WHERE local_part = 'support' AND active;",
    );
    assert!(!address_id.trim().is_empty());

    run_psql(&container.name, ROLLBACK);
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*) FROM addresses WHERE local_part = 'support' AND NOT active;",
        ),
        1
    );
    run_psql(&container.name, MIGRATION);
    run_psql(&container.name, MIGRATION);
    assert_eq!(
        run_psql(
            &container.name,
            "SELECT id FROM addresses WHERE local_part = 'support' AND active;",
        ),
        address_id
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*) FROM addresses WHERE active;"
        ),
        3
    );
}
