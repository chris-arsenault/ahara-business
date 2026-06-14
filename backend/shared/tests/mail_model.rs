use shared::db::{INITIAL_ROUTING_SEED, MAIL_MODEL_MIGRATION, MAIL_MODEL_ROLLBACK};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static CONTAINER_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct DockerPostgres {
    name: String,
}

impl Drop for DockerPostgres {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.name])
            .status();
    }
}

fn setup_postgres() -> DockerPostgres {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = CONTAINER_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = format!(
        "ahara-business-mail-model-{}-{suffix}-{sequence}",
        std::process::id()
    );

    let output = Command::new("docker")
        .args([
            "run",
            "-d",
            "--rm",
            "--name",
            &name,
            "-e",
            "POSTGRES_PASSWORD=postgres",
            "postgres:16-alpine",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "docker run failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let container = DockerPostgres { name };
    wait_for_postgres(&container.name);
    container
}

fn wait_for_postgres(container_name: &str) {
    let network = format!("container:{container_name}");

    for _ in 0..60 {
        let output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--network",
                &network,
                "-e",
                "PGPASSWORD=postgres",
                "postgres:16-alpine",
                "pg_isready",
                "-h",
                "127.0.0.1",
                "-U",
                "postgres",
            ])
            .output()
            .unwrap();

        if output.status.success() {
            return;
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    panic!("Postgres did not become ready");
}

fn run_psql(container_name: &str, sql: &str) -> String {
    let network = format!("container:{container_name}");
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--network",
            &network,
            "-e",
            "PGPASSWORD=postgres",
            "postgres:16-alpine",
            "psql",
            "-v",
            "ON_ERROR_STOP=1",
            "-qAt",
            "-h",
            "127.0.0.1",
            "-U",
            "postgres",
            "-d",
            "postgres",
            "-c",
            sql,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "psql failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap()
}

fn assert_psql_fails(container_name: &str, sql: &str) {
    let network = format!("container:{container_name}");
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--network",
            &network,
            "-e",
            "PGPASSWORD=postgres",
            "postgres:16-alpine",
            "psql",
            "-v",
            "ON_ERROR_STOP=1",
            "-qAt",
            "-h",
            "127.0.0.1",
            "-U",
            "postgres",
            "-d",
            "postgres",
            "-c",
            sql,
        ])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "psql unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn scalar_i64(container_name: &str, query: &str) -> i64 {
    run_psql(container_name, query)
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

fn table_exists(container_name: &str, table_name: &str) -> bool {
    let query = format!(
        "SELECT CASE WHEN EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = 'public' AND table_name = '{table_name}'
        ) THEN 1 ELSE 0 END;"
    );
    scalar_i64(container_name, &query) == 1
}

fn column_exists(container_name: &str, table_name: &str, column_name: &str) -> bool {
    let query = format!(
        "SELECT CASE WHEN EXISTS (
            SELECT 1
            FROM information_schema.columns
            WHERE table_schema = 'public'
              AND table_name = '{table_name}'
              AND column_name = '{column_name}'
        ) THEN 1 ELSE 0 END;"
    );
    scalar_i64(container_name, &query) == 1
}

#[test]
fn mail_model_migration_seed_and_rollback_round_trip() {
    let container = setup_postgres();

    run_psql(&container.name, MAIL_MODEL_MIGRATION);

    for table_name in [
        "domains",
        "addresses",
        "contacts",
        "threads",
        "messages",
        "recipients",
        "attachment_refs",
        "forwarding_rules",
        "suppressions",
        "outbound_work",
        "calendar_events",
        "bookings",
        "finance_expenses",
        "finance_expense_audit",
        "finance_receivables",
        "finance_receivable_audit",
    ] {
        assert!(
            table_exists(&container.name, table_name),
            "{table_name} exists"
        );
    }
    println!("forward migration applied with all M2 tables present");

    for column_name in [
        "spam_result",
        "virus_result",
        "security_disposition",
        "security_reason",
    ] {
        assert!(
            column_exists(&container.name, "messages", column_name),
            "messages.{column_name} exists"
        );
    }
    println!("messages table includes spam, virus, and security disposition fields");

    run_psql(
        &container.name,
        "WITH contact_row AS (
            INSERT INTO contacts (display_name)
            VALUES ('Calendar Contact')
            RETURNING id
        ),
        event_row AS (
            INSERT INTO calendar_events (
                title, status, starts_at, ends_at, contact_id
            )
            SELECT
                'Intro call', 'confirmed',
                '2026-06-13T14:00:00Z',
                '2026-06-13T14:30:00Z',
                id
            FROM contact_row
            RETURNING id, contact_id
        )
        INSERT INTO bookings (
            calendar_event_id, contact_id, title, status, starts_at, ends_at
        )
        SELECT
            id, contact_id, 'Intro booking', 'requested',
            '2026-06-13T14:00:00Z',
            '2026-06-13T14:30:00Z'
        FROM event_row;",
    );
    assert_psql_fails(
        &container.name,
        "INSERT INTO calendar_events (title, starts_at, ends_at)
         VALUES ('bad', '2026-06-13T15:00:00Z', '2026-06-13T14:00:00Z');",
    );
    println!("calendar and booking tables enforce linked operational events");

    run_psql(&container.name, INITIAL_ROUTING_SEED);
    run_psql(&container.name, INITIAL_ROUTING_SEED);

    let domain_count = scalar_i64(
        &container.name,
        "SELECT count(*) FROM domains WHERE domain_name = 'ahara.io'",
    );
    let address_count = scalar_i64(
        &container.name,
        "SELECT count(*)
         FROM addresses
         JOIN domains ON domains.id = addresses.domain_id
         WHERE domains.domain_name = 'ahara.io'
           AND addresses.local_part IN ('chris', 'contact')",
    );

    assert_eq!(domain_count, 1);
    assert_eq!(address_count, 2);
    println!(
        "seed applied twice without duplicates: domains={domain_count}, addresses={address_count}"
    );

    run_psql(&container.name, MAIL_MODEL_ROLLBACK);

    for table_name in [
        "bookings",
        "calendar_events",
        "finance_receivable_audit",
        "finance_receivables",
        "finance_expense_audit",
        "finance_expenses",
        "outbound_work",
        "suppressions",
        "forwarding_rules",
        "attachment_refs",
        "recipients",
        "messages",
        "threads",
        "contacts",
        "addresses",
        "domains",
    ] {
        assert!(
            !table_exists(&container.name, table_name),
            "{table_name} removed"
        );
    }
    println!("rollback removed all M2 tables");

    run_psql(&container.name, MAIL_MODEL_MIGRATION);
    assert!(table_exists(&container.name, "messages"));
    println!("forward migration re-applied after rollback");
}

#[test]
fn mail_model_enforces_scan_verdict_and_security_disposition_values() {
    let container = setup_postgres();

    run_psql(&container.name, MAIL_MODEL_MIGRATION);

    run_psql(
        &container.name,
        "INSERT INTO messages (
            direction,
            from_address,
            from_address_normalized,
            spam_result,
            virus_result,
            security_disposition,
            status
        ) VALUES (
            'inbound',
            'sender@example.com',
            'sender@example.com',
            'fail',
            'pass',
            'quarantined',
            'quarantined'
        );",
    );

    assert_psql_fails(
        &container.name,
        "INSERT INTO messages (
            direction,
            from_address,
            from_address_normalized,
            spam_result
        ) VALUES (
            'inbound',
            'sender@example.com',
            'sender@example.com',
            'unknown'
        );",
    );
    assert_psql_fails(
        &container.name,
        "INSERT INTO messages (
            direction,
            from_address,
            from_address_normalized,
            virus_result
        ) VALUES (
            'inbound',
            'sender@example.com',
            'sender@example.com',
            'unknown'
        );",
    );
    assert_psql_fails(
        &container.name,
        "INSERT INTO messages (
            direction,
            from_address,
            from_address_normalized,
            security_disposition
        ) VALUES (
            'inbound',
            'sender@example.com',
            'sender@example.com',
            'unknown'
        );",
    );
    println!("scan verdict and security disposition constraints enforced");
}
