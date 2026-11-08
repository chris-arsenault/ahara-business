use super::{
    MailboxAttachment, MailboxAuthResult, MailboxMessageDetail, MailboxScanResult,
    MailboxSecurityDisposition,
};

pub(super) fn accepted_message() -> MailboxMessageDetail {
    MailboxMessageDetail {
        id: "00000000-0000-0000-0000-000000000001".to_string(),
        direction: "inbound".to_string(),
        status: "received".to_string(),
        sent_at: None,
        created_at: None,
        thread_id: Some("00000000-0000-0000-0000-000000000101".to_string()),
        rfc_message_id: Some("<accepted@example.test>".to_string()),
        in_reply_to: None,
        reference_ids: vec![],
        from_address: "sender@example.test".to_string(),
        from_display_name: "Sender Display".to_string(),
        subject: "Invoice".to_string(),
        message_date: Some("2026-01-01 00:00:00+00".to_string()),
        received_at: Some("2026-01-01 00:00:00+00".to_string()),
        body_text: "Plaintext invoice body with auth verdict details.".to_string(),
        recipients: vec![],
        attachments: vec![MailboxAttachment {
            id: "00000000-0000-0000-0000-000000000301".to_string(),
            position: 0,
            filename: "../invoice.pdf".to_string(),
            display_filename: "invoice.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            size_bytes: Some(12),
            content_id: None,
        }],
        is_read: false,
        contact_id: None,
        spf_result: Some(MailboxAuthResult::Pass),
        dkim_result: Some(MailboxAuthResult::Pass),
        dmarc_result: Some(MailboxAuthResult::Pass),
        auth_verdict: Some(MailboxAuthResult::Pass),
        spam_result: Some(MailboxScanResult::Pass),
        virus_result: Some(MailboxScanResult::Pass),
        security_disposition: MailboxSecurityDisposition::Accepted,
        security_reason: Some("clean".to_string()),
    }
}
