use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use shared::db::{INITIAL_ROUTING_SEED, MAIL_MODEL_MIGRATION};

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
        "ahara-business-inbound-ingest-{}-{suffix}-{sequence}",
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
fn inbound_ingest_model_supports_m5_persistence_shapes() {
    let container = setup_postgres();
    run_psql(&container.name, MAIL_MODEL_MIGRATION);
    run_psql(&container.name, INITIAL_ROUTING_SEED);

    run_psql(
        &container.name,
        "
        INSERT INTO contacts (display_name, primary_address, primary_address_normalized)
        VALUES ('Sender', 'sender@example.test', 'sender@example.test');

        WITH route AS (
            SELECT domains.id AS domain_id, addresses.id AS address_id
            FROM domains
            JOIN addresses ON addresses.domain_id = domains.id
            WHERE domains.domain_name = 'ahara.io'
              AND addresses.local_part = 'contact'
        ),
        inserted_thread AS (
            INSERT INTO threads (normalized_subject, participants, message_count)
            VALUES ('attachments', '[\"sender@example.test\",\"contact@ahara.io\"]'::jsonb, 1)
            RETURNING id
        ),
        inserted_message AS (
            INSERT INTO messages (
                direction, ses_message_id, rfc_message_id, thread_id,
                from_address, from_address_normalized, from_display_name, subject,
                matched_domain_id, matched_address_id, matched_local_part, body_text,
                s3_raw_key, spf_result, dkim_result, dmarc_result, auth_verdict,
                spam_result, virus_result, security_disposition, security_reason,
                contact_id, status, has_attachments, attachment_count, size_bytes, received_at
            )
            SELECT
                'inbound', 'ses-accepted', '<attachments@example.test>', inserted_thread.id,
                'sender@example.test', 'sender@example.test', 'Sender', 'Attachments',
                route.domain_id, route.address_id, 'contact', 'Attached metadata only.',
                'raw/ses-accepted', 'pass', 'pass', 'pass', 'pass',
                'pass', 'pass', 'accepted', 'clean',
                contacts.id, 'received', true, 2, 1024, now()
            FROM route, inserted_thread, contacts
            WHERE contacts.primary_address_normalized = 'sender@example.test'
            RETURNING id
        )
        INSERT INTO recipients (message_id, kind, address, address_normalized, display_name, position)
        SELECT id, 'to', 'contact@ahara.io', 'contact@ahara.io', 'Contact', 0
        FROM inserted_message;

        INSERT INTO attachment_refs (message_id, position, filename, content_type, size_bytes, content_id)
        SELECT id, 0, 'invoice.pdf', 'application/pdf', 14, '<invoice-content>'
        FROM messages
        WHERE ses_message_id = 'ses-accepted';
        INSERT INTO attachment_refs (message_id, position, filename, content_type, size_bytes)
        SELECT id, 1, 'notes.txt', 'text/plain', 16
        FROM messages
        WHERE ses_message_id = 'ses-accepted';
        ",
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*)
             FROM messages
             WHERE ses_message_id = 'ses-accepted'
               AND status = 'received'
               AND security_disposition = 'accepted'
               AND contact_id IS NOT NULL
               AND has_attachments = true
               AND attachment_count = 2;"
        ),
        1
    );
    assert_eq!(
        scalar_i64(&container.name, "SELECT count(*) FROM attachment_refs;"),
        2
    );
    println!(
        "accepted inbound message shape persisted with recipients, attachments, contact, thread"
    );

    run_psql(
        &container.name,
        "
        INSERT INTO messages (
            direction, ses_message_id, rfc_message_id,
            from_address, from_address_normalized, from_display_name, subject,
            body_text, s3_raw_key, spf_result, dkim_result, dmarc_result, auth_verdict,
            spam_result, virus_result, security_disposition, security_reason,
            status, has_attachments, attachment_count, size_bytes, received_at
        )
        VALUES (
            'inbound', 'ses-spam', '<spam@example.test>',
            'sender@example.test', 'sender@example.test', 'Sender', 'Spam',
            'Spam body', 'raw/ses-spam', 'pass', 'pass', 'pass', 'pass',
            'fail', 'pass', 'quarantined', 'spam_failed',
            'quarantined', false, 0, 512, now()
        );
        ",
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*)
             FROM messages
             WHERE ses_message_id = 'ses-spam'
               AND status = 'quarantined'
               AND security_disposition = 'quarantined'
               AND spam_result = 'fail';"
        ),
        1
    );
    println!("spam disposition persists as quarantined and not received");

    run_psql(
        &container.name,
        "
        INSERT INTO messages (
            direction, ses_message_id,
            from_address, from_address_normalized, from_display_name,
            body_text, s3_raw_key, spf_result, dkim_result, dmarc_result, auth_verdict,
            spam_result, virus_result, security_disposition, security_reason,
            status, has_attachments, attachment_count, size_bytes, received_at
        )
        VALUES (
            'inbound', 'ses-virus',
            'sender@example.test', 'sender@example.test', 'Sender',
            '', 'raw/ses-virus', 'pass', 'pass', 'pass', 'pass',
            'pass', 'fail', 'rejected', 'virus_failed',
            'rejected', false, 0, 4096, now()
        );
        INSERT INTO recipients (message_id, kind, address, address_normalized, display_name, position)
        SELECT id, 'to', 'contact@ahara.io', 'contact@ahara.io', '', 0
        FROM messages
        WHERE ses_message_id = 'ses-virus';
        ",
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*)
             FROM messages
             WHERE ses_message_id = 'ses-virus'
               AND body_text = ''
               AND status = 'rejected'
               AND security_disposition = 'rejected'
               AND attachment_count = 0;"
        ),
        1
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*)
             FROM attachment_refs
             JOIN messages ON messages.id = attachment_refs.message_id
             WHERE messages.ses_message_id = 'ses-virus';"
        ),
        0
    );
    println!("virus disposition persists as rejected minimal audit without attachment refs");

    run_psql(
        &container.name,
        "
        INSERT INTO messages (
            direction, ses_message_id,
            from_address, from_address_normalized, from_display_name,
            body_text, s3_raw_key, security_disposition, security_reason,
            status, has_attachments, attachment_count, size_bytes, received_at
        )
        VALUES (
            'inbound', 'ses-old',
            'sender@example.test', 'sender@example.test', 'Sender',
            '', 'raw/ses-old', 'rejected', 'clean',
            'rejected', false, 0, 9999, now() - interval '2 hours'
        );
        ",
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT COALESCE(SUM(size_bytes), 0)::BIGINT
             FROM messages
             WHERE direction = 'inbound'
               AND size_bytes IS NOT NULL
               AND received_at >= now() - (3600::DOUBLE PRECISION * interval '1 second');"
        ),
        5632
    );
    println!("recent raw byte total excludes old inbound audit rows");

    run_psql(
        &container.name,
        "
        WITH source_thread AS (
            SELECT thread_id FROM messages WHERE ses_message_id = 'ses-accepted'
        )
        UPDATE threads
        SET message_count = message_count + 1,
            participants = '[\"sender@example.test\",\"contact@ahara.io\",\"support@ahara.io\"]'::jsonb
        FROM source_thread
        WHERE threads.id = source_thread.thread_id;
        ",
    );
    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*) FROM threads WHERE message_count = 2;"
        ),
        1
    );
    println!("thread update shape supports reply association");

    assert_eq!(
        scalar_i64(
            &container.name,
            "SELECT count(*) FROM messages WHERE ses_message_id IN ('ses-accepted', 'ses-spam', 'ses-virus');"
        ),
        3
    );
}
