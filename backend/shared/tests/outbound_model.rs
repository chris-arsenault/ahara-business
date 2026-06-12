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
        "ahara-business-outbound-model-{}-{suffix}",
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
fn outbound_model_supports_queue_claim_success_and_suppression_shape() {
    if !postgres_docker_available() {
        eprintln!(
            "skipping PostgreSQL outbound model assertion: postgres:16-alpine cannot execute in this Docker runner"
        );
        return;
    }

    let sql = format!(
        r#"
{MAIL_MODEL_MIGRATION}

WITH inserted_thread AS (
    INSERT INTO threads (normalized_subject, participants, message_count)
    VALUES ('hello', '["contact@ahara.io","person@example.com"]'::jsonb, 1)
    RETURNING id
),
inserted_message AS (
    INSERT INTO messages (
        direction, rfc_message_id, thread_id,
        from_address, from_address_normalized, subject, body_text,
        message_date, security_disposition, status, has_attachments, attachment_count
    )
    SELECT
        'outbound', '<outbound-1@ahara.io>', inserted_thread.id,
        'contact@ahara.io', 'contact@ahara.io', 'Hello', 'Plain body',
        now(), 'accepted', 'queued', false, 0
    FROM inserted_thread
    RETURNING id
),
recipient_to AS (
    INSERT INTO recipients (message_id, kind, address, address_normalized, position)
    SELECT id, 'to', 'person@example.com', 'person@example.com', 0
    FROM inserted_message
),
recipient_bcc AS (
    INSERT INTO recipients (message_id, kind, address, address_normalized, position)
    SELECT id, 'bcc', 'hidden@example.com', 'hidden@example.com', 1
    FROM inserted_message
)
INSERT INTO outbound_work (message_id, status, idempotency_key)
SELECT id, 'queued', 'compose:test'
FROM inserted_message;

INSERT INTO suppressions (address, address_normalized, reason)
VALUES ('blocked@example.com', 'blocked@example.com', 'manual');

SELECT 'QUEUED=' || count(*)
FROM messages
JOIN outbound_work ON outbound_work.message_id = messages.id
JOIN recipients ON recipients.message_id = messages.id
WHERE messages.direction = 'outbound'
  AND messages.status = 'queued'
  AND outbound_work.status = 'queued'
  AND recipients.kind IN ('to', 'bcc');

WITH due AS (
    SELECT outbound_work.id
    FROM outbound_work
    JOIN messages ON messages.id = outbound_work.message_id
    WHERE outbound_work.status = 'queued'
      AND outbound_work.next_attempt_at <= now()
      AND messages.direction = 'outbound'
      AND messages.status = 'queued'
    ORDER BY outbound_work.next_attempt_at ASC, outbound_work.created_at ASC
    LIMIT 25
    FOR UPDATE SKIP LOCKED
),
claimed AS (
    UPDATE outbound_work
    SET status = 'sending',
        attempt_count = outbound_work.attempt_count + 1,
        locked_at = now(),
        locked_by = 'worker-1',
        updated_at = now()
    FROM due
    WHERE outbound_work.id = due.id
    RETURNING outbound_work.message_id, outbound_work.attempt_count
)
UPDATE messages
SET status = 'sending',
    send_attempt_count = claimed.attempt_count,
    updated_at = now()
FROM claimed
WHERE messages.id = claimed.message_id;

SELECT 'CLAIMED=' || messages.status || ':' || outbound_work.status || ':' || outbound_work.attempt_count
FROM messages
JOIN outbound_work ON outbound_work.message_id = messages.id
WHERE messages.rfc_message_id = '<outbound-1@ahara.io>';

UPDATE outbound_work
SET status = 'sent',
    locked_at = NULL,
    locked_by = NULL,
    updated_at = now()
WHERE status = 'sending';

UPDATE messages
SET status = 'sent',
    ses_message_id = 'ses-provider-1',
    sent_at = now(),
    updated_at = now()
WHERE rfc_message_id = '<outbound-1@ahara.io>';

SELECT 'SENT=' || messages.status || ':' || outbound_work.status || ':' || messages.ses_message_id
FROM messages
JOIN outbound_work ON outbound_work.message_id = messages.id
WHERE messages.rfc_message_id = '<outbound-1@ahara.io>';

SELECT 'SUPPRESSED=' || count(*)
FROM suppressions
WHERE address_normalized = 'blocked@example.com'
  AND reason = 'manual';
"#
    );

    let output = run_ephemeral_postgres_sql(&sql);

    assert!(output.lines().any(|line| line == "QUEUED=2"), "{output}");
    assert!(
        output
            .lines()
            .any(|line| line == "CLAIMED=sending:sending:1"),
        "{output}"
    );
    assert!(
        output
            .lines()
            .any(|line| line == "SENT=sent:sent:ses-provider-1"),
        "{output}"
    );
    assert!(
        output.lines().any(|line| line == "SUPPRESSED=1"),
        "{output}"
    );
    println!("outbound PostgreSQL shape supports queue, claim, success, and suppression rows");
}
