use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use shared::db::MAIL_MODEL_MIGRATION;

struct DockerPostgres {
    name: String,
}

impl Drop for DockerPostgres {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn setup_postgres() -> DockerPostgres {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let name = format!(
        "ahara-business-forwarding-model-{}-{suffix}",
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

fn run_ephemeral_postgres_sql(sql: &str) -> String {
    let container = setup_postgres();
    let network = format!("container:{}", container.name);
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

fn postgres_docker_available() -> bool {
    Command::new("timeout")
        .args([
            "10",
            "docker",
            "run",
            "--rm",
            "postgres:16-alpine",
            "echo",
            "postgres-ok",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[test]
fn forwarding_rules_model_supports_address_scoped_active_rule_lookup() {
    if !postgres_docker_available() {
        eprintln!(
            "skipping PostgreSQL forwarding model assertion: postgres:16-alpine cannot execute in this Docker runner"
        );
        return;
    }

    let sql = format!(
        r#"
{MAIL_MODEL_MIGRATION}

WITH domain_row AS (
    INSERT INTO domains (domain_name, routing_policy, active)
    VALUES ('ahara.io', 'allowlist', true)
    RETURNING id
),
contact_address AS (
    INSERT INTO addresses (domain_id, local_part, active)
    SELECT id, 'contact', true FROM domain_row
    RETURNING id, domain_id
),
chris_address AS (
    INSERT INTO addresses (domain_id, local_part, active)
    SELECT domain_id, 'chris', true FROM contact_address
    RETURNING id
),
address_rule AS (
    INSERT INTO forwarding_rules (
        rule_kind, address_id, target_address, target_address_normalized, active
    )
    SELECT 'address', id, 'Target@Example.COM', 'target@example.com', true
    FROM contact_address
),
inactive_rule AS (
    INSERT INTO forwarding_rules (
        rule_kind, address_id, target_address, target_address_normalized, active
    )
    SELECT 'address', id, 'Inactive@Example.COM', 'inactive@example.com', false
    FROM contact_address
),
domain_rule AS (
    INSERT INTO forwarding_rules (
        rule_kind, domain_id, target_address, target_address_normalized, active
    )
    SELECT 'domain', domain_id, 'Domain@Example.COM', 'domain@example.com', true
    FROM contact_address
),
accepted_message AS (
    INSERT INTO messages (
        direction, matched_domain_id, matched_address_id, matched_local_part,
        from_address, from_address_normalized, subject, body_text,
        security_disposition, status, received_at
    )
    SELECT
        'inbound', domain_id, id, 'contact',
        'sender@example.com', 'sender@example.com', 'Forward me', 'body',
        'accepted', 'received', now()
    FROM contact_address
    RETURNING id
)
INSERT INTO messages (
    direction, matched_domain_id, matched_address_id, matched_local_part,
    from_address, from_address_normalized, subject, body_text,
    security_disposition, status, received_at
)
SELECT
    'inbound', domain_id, id, 'contact',
    'sender@example.com', 'sender@example.com', 'Quarantine', 'body',
    'quarantined', 'quarantined', now()
FROM contact_address;

SELECT 'LIST=' || count(*)
FROM forwarding_rules
WHERE rule_kind = 'address';

SELECT 'ACTIVE=' || COALESCE(string_agg(forwarding_rules.target_address_normalized, ',' ORDER BY forwarding_rules.target_address_normalized), '')
FROM messages
JOIN forwarding_rules ON forwarding_rules.address_id = messages.matched_address_id
WHERE messages.direction = 'inbound'
  AND messages.security_disposition = 'accepted'
  AND messages.status = 'received'
  AND forwarding_rules.rule_kind = 'address'
  AND forwarding_rules.active = true;

SELECT 'QUARANTINED=' || count(*)
FROM messages
JOIN forwarding_rules ON forwarding_rules.address_id = messages.matched_address_id
WHERE messages.direction = 'inbound'
  AND messages.security_disposition = 'accepted'
  AND messages.status = 'received'
  AND messages.subject = 'Quarantine'
  AND forwarding_rules.rule_kind = 'address'
  AND forwarding_rules.active = true;
"#
    );

    let output = run_ephemeral_postgres_sql(&sql);

    assert!(output.lines().any(|line| line == "LIST=2"), "{output}");
    assert!(
        output
            .lines()
            .any(|line| line == "ACTIVE=target@example.com"),
        "{output}"
    );
    assert!(
        output.lines().any(|line| line == "QUARANTINED=0"),
        "{output}"
    );
    println!("forwarding PostgreSQL shape supports address-scoped active rule lookup");
}
