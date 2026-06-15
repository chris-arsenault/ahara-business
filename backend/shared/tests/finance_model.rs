use shared::db::MAIL_MODEL_MIGRATION;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    let name = format!(
        "ahara-business-finance-model-{}-{suffix}",
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

#[test]
fn finance_tables_enforce_tax_allocation_and_audit_constraints() {
    let container = setup_postgres();

    run_psql(&container.name, MAIL_MODEL_MIGRATION);
    run_psql(
        &container.name,
        "WITH contact_row AS (
            INSERT INTO contacts (display_name)
            VALUES ('Finance Contact')
            RETURNING id
        ),
        expense_row AS (
            INSERT INTO finance_expenses (
                title, vendor_name, category, expense_kind, recurrence_interval,
                amount_cents, incurred_on, business_use_percent_bps
            )
            VALUES (
                'Cloud hosting', 'AWS', 'cloud', 'recurring', 'monthly',
                12000, '2026-06-01', 7500
            )
            RETURNING id
        )
        INSERT INTO finance_receivables (
            contact_id, title, amount_cents, due_on
        )
        SELECT contact_row.id, 'Client session', 25000, '2026-06-15'
        FROM contact_row, expense_row;",
    );
    run_psql(
        &container.name,
        "INSERT INTO finance_expenses (
            title, vendor_name, category, expense_kind, recurrence_interval,
            amount_cents, incurred_on, business_use_percent_bps,
            recurrence_parent_expense_id, recurrence_instance_on
        )
        SELECT title, vendor_name, category, expense_kind, recurrence_interval,
            13542, '2026-07-01', business_use_percent_bps, id, '2026-07-01'
        FROM finance_expenses
        WHERE title = 'Cloud hosting';",
    );

    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*) FROM finance_expense_audit",
        ),
        2
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*) FROM finance_receivable_audit",
        ),
        1
    );
    assert_psql_fails(
        &container.name,
        "INSERT INTO finance_expenses (
            title, category, amount_cents, incurred_on, business_use_percent_bps
        ) VALUES ('bad', 'cloud', 100, '2026-06-01', 10001);",
    );
    assert_psql_fails(
        &container.name,
        "INSERT INTO finance_receivables (
            title, status, amount_cents
        ) VALUES ('bad', 'paid', 100);",
    );
    assert_psql_fails(
        &container.name,
        "INSERT INTO finance_expenses (
            title, category, amount_cents, incurred_on, recurrence_parent_expense_id
        )
        SELECT 'bad', 'cloud', 100, '2026-07-01', id
        FROM finance_expenses
        WHERE title = 'Cloud hosting'
        LIMIT 1;",
    );
    println!("finance tables enforce tax allocation and receivable audit constraints");
}
