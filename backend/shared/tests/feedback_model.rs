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
        "ahara-business-feedback-model-{}-{suffix}",
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
fn feedback_model_updates_outbound_status_and_suppression_rows() {
    if !postgres_docker_available() {
        eprintln!(
            "skipping PostgreSQL feedback model assertion: postgres:16-alpine cannot execute in this Docker runner"
        );
        return;
    }

    let sql = format!(
        r#"
{MAIL_MODEL_MIGRATION}

WITH inserted_message AS (
    INSERT INTO messages (
        direction, ses_message_id, rfc_message_id,
        from_address, from_address_normalized, subject, body_text,
        security_disposition, status, sent_at
    )
    VALUES (
        'outbound', 'ses-provider-1', '<outbound@ahara.io>',
        'contact@ahara.io', 'contact@ahara.io', 'Hello', 'body',
        'accepted', 'sent', now()
    )
    RETURNING id
)
INSERT INTO outbound_work (message_id, status, idempotency_key)
SELECT id, 'sent', 'compose:test'
FROM inserted_message;

WITH affected AS (
    UPDATE messages
    SET status = 'complained',
        last_error = 'ses complained',
        updated_at = now()
    WHERE direction = 'outbound'
      AND ses_message_id = 'ses-provider-1'
    RETURNING id
),
work_update AS (
    UPDATE outbound_work
    SET status = 'complained',
        last_error = 'ses complained',
        updated_at = now()
    FROM affected
    WHERE outbound_work.message_id = affected.id
)
INSERT INTO suppressions (
    address, address_normalized, reason, source_message_id, notes
)
SELECT
    'Person@Example.COM',
    'person@example.com',
    'complaint',
    id,
    'ses feedback ses-provider-1'
FROM affected;

SELECT 'STATUS=' || messages.status || ':' || outbound_work.status
FROM messages
JOIN outbound_work ON outbound_work.message_id = messages.id
WHERE messages.ses_message_id = 'ses-provider-1';

SELECT 'SUPPRESSION=' || address_normalized || ':' || reason
FROM suppressions
WHERE address_normalized = 'person@example.com';
"#
    );

    let output = run_ephemeral_postgres_sql(&sql);

    assert!(
        output
            .lines()
            .any(|line| line == "STATUS=complained:complained"),
        "{output}"
    );
    assert!(
        output
            .lines()
            .any(|line| line == "SUPPRESSION=person@example.com:complaint"),
        "{output}"
    );
    println!("feedback PostgreSQL shape supports status and suppression updates");
}
