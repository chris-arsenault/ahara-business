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
        "ahara-business-mailbox-model-{}-{suffix}",
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
fn mailbox_model_query_shape_excludes_quarantined_and_rejected_mail() {
    if !postgres_docker_available() {
        eprintln!(
            "skipping PostgreSQL mailbox model assertion: postgres:16-alpine cannot execute in this Docker runner"
        );
        return;
    }

    let sql = format!(
        r#"
{MAIL_MODEL_MIGRATION}

WITH inserted_thread AS (
    INSERT INTO threads (normalized_subject, participants, message_count)
    VALUES ('invoice', '["sender@example.test","contact@ahara.io"]'::jsonb, 3)
    RETURNING id
)
INSERT INTO messages (
    direction, ses_message_id, rfc_message_id, thread_id,
    from_address, from_address_normalized, from_display_name, subject,
    body_text, spf_result, dkim_result, dmarc_result, auth_verdict,
    spam_result, virus_result, security_disposition, security_reason,
    status, has_attachments, attachment_count, size_bytes, received_at
)
SELECT
    'inbound', 'ses-accepted', '<accepted@example.test>', inserted_thread.id,
    'sender@example.test', 'sender@example.test', 'Sender', 'Invoice',
    'Accepted invoice body', 'pass', 'pass', 'pass', 'pass',
    'pass', 'pass', 'accepted', 'clean',
    'received', false, 0, 100, now()
FROM inserted_thread
UNION ALL
SELECT
    'inbound', 'ses-quarantined', '<quarantined@example.test>', inserted_thread.id,
    'sender@example.test', 'sender@example.test', 'Sender', 'Invoice',
    'Quarantined invoice body', 'pass', 'pass', 'pass', 'pass',
    'fail', 'pass', 'quarantined', 'spam_failed',
    'quarantined', false, 0, 100, now()
FROM inserted_thread
UNION ALL
SELECT
    'inbound', 'ses-rejected', '<rejected@example.test>', inserted_thread.id,
    'sender@example.test', 'sender@example.test', 'Sender', 'Invoice',
    'Rejected invoice body', 'pass', 'pass', 'pass', 'pass',
    'pass', 'fail', 'rejected', 'virus_failed',
    'rejected', false, 0, 100, now()
FROM inserted_thread;

SELECT 'LIST=' || COALESCE(string_agg(ses_message_id, ',' ORDER BY ses_message_id), '')
FROM messages
WHERE direction = 'inbound'
  AND security_disposition = 'accepted'
  AND status = 'received';

SELECT 'SEARCH=' || COALESCE(string_agg(ses_message_id, ',' ORDER BY ses_message_id), '')
FROM messages
WHERE direction = 'inbound'
  AND security_disposition = 'accepted'
  AND status = 'received'
  AND (
       position('invoice' in lower(subject)) > 0
    OR position('invoice' in lower(from_address)) > 0
    OR position('invoice' in lower(from_display_name)) > 0
    OR position('invoice' in lower(body_text)) > 0
  );

WITH attempted AS (
    UPDATE messages
    SET is_read = true
    WHERE ses_message_id = 'ses-quarantined'
      AND direction = 'inbound'
      AND security_disposition = 'accepted'
      AND status = 'received'
    RETURNING id
)
SELECT 'MUTATION=' || count(*) FROM attempted;
"#
    );

    let output = run_ephemeral_postgres_sql(&sql);

    assert!(
        output.lines().any(|line| line == "LIST=ses-accepted"),
        "{output}"
    );
    assert!(
        output.lines().any(|line| line == "SEARCH=ses-accepted"),
        "{output}"
    );
    assert!(output.lines().any(|line| line == "MUTATION=0"), "{output}");
    println!("mailbox PostgreSQL shape returns and mutates accepted inbound mail only");
}
