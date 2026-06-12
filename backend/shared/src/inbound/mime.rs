use mailparse::{
    DispositionType, MailAddr, MailHeaderMap, ParsedMail, SingleInfo, addrparse, dateparse,
};

use crate::inbound::limits::IngestLimits;
use crate::inbound::text::select_body_text;
use crate::inbound::types::{
    InboundAttachment, InboundMailbox, InboundRecipient, InboundRecipientKind, ParsedInboundMessage,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InboundParseError {
    #[error("inbound MIME parse failed: {0}")]
    Mime(String),

    #[error("inbound MIME limit exceeded: {limit}")]
    LimitExceeded { limit: &'static str },
}

#[derive(Debug, Default)]
struct ParseState {
    plain_body: Option<String>,
    html_body: Option<String>,
    attachments: Vec<InboundAttachment>,
}

pub fn parse_raw_mime(
    bytes: &[u8],
    limits: IngestLimits,
) -> Result<ParsedInboundMessage, InboundParseError> {
    if bytes.len() > limits.max_raw_mime_bytes {
        return Err(InboundParseError::LimitExceeded {
            limit: "raw_mime_bytes",
        });
    }

    let parsed =
        mailparse::parse_mail(bytes).map_err(|err| InboundParseError::Mime(err.to_string()))?;
    let mut state = ParseState::default();
    walk_part(&parsed, 0, limits, &mut state)?;

    Ok(ParsedInboundMessage {
        from: parse_first_mailbox(&parsed, "From").unwrap_or_else(InboundMailbox::unknown),
        subject: parsed
            .headers
            .get_first_value("Subject")
            .unwrap_or_default(),
        message_date_epoch: parsed
            .headers
            .get_first_value("Date")
            .and_then(|date| dateparse(&date).ok()),
        rfc_message_id: parsed.headers.get_first_value("Message-ID"),
        in_reply_to: parsed.headers.get_first_value("In-Reply-To"),
        reference_ids: parse_references(parsed.headers.get_first_value("References")),
        recipients: parse_recipients(&parsed),
        body_text: select_body_text(state.plain_body.as_deref(), state.html_body.as_deref()),
        attachments: state.attachments,
        size_bytes: bytes.len() as i64,
    })
}

fn walk_part(
    part: &ParsedMail<'_>,
    depth: usize,
    limits: IngestLimits,
    state: &mut ParseState,
) -> Result<(), InboundParseError> {
    if depth > limits.max_mime_depth {
        return Err(InboundParseError::LimitExceeded {
            limit: "mime_depth",
        });
    }

    if !part.subparts.is_empty() {
        for subpart in &part.subparts {
            walk_part(subpart, depth + 1, limits, state)?;
        }
        return Ok(());
    }

    if is_attachment(part) {
        if state.attachments.len() >= limits.max_attachment_count {
            return Err(InboundParseError::LimitExceeded {
                limit: "attachment_count",
            });
        }
        let position = state.attachments.len() as i32;
        state.attachments.push(InboundAttachment {
            position,
            filename: attachment_filename(part).unwrap_or_default(),
            content_type: part.ctype.mimetype.clone(),
            size_bytes: part.get_body_raw().ok().map(|body| body.len() as i64),
            content_id: part.headers.get_first_value("Content-ID"),
        });
        return Ok(());
    }

    if part.ctype.mimetype.eq_ignore_ascii_case("text/plain") {
        let body = part.get_body().unwrap_or_default();
        if state.plain_body.is_none() && !body.trim().is_empty() {
            state.plain_body = Some(body);
        }
    } else if part.ctype.mimetype.eq_ignore_ascii_case("text/html") {
        let body = part.get_body().unwrap_or_default();
        if state.html_body.is_none() && !body.trim().is_empty() {
            state.html_body = Some(body);
        }
    }

    Ok(())
}

fn is_attachment(part: &ParsedMail<'_>) -> bool {
    let disposition = part.get_content_disposition();
    disposition.disposition == DispositionType::Attachment
        || disposition.params.contains_key("filename")
        || part.ctype.params.contains_key("name")
}

fn attachment_filename(part: &ParsedMail<'_>) -> Option<String> {
    let disposition = part.get_content_disposition();
    disposition
        .params
        .get("filename")
        .cloned()
        .or_else(|| part.ctype.params.get("name").cloned())
}

fn parse_first_mailbox(parsed: &ParsedMail<'_>, header: &str) -> Option<InboundMailbox> {
    parsed
        .headers
        .get_first_value(header)
        .and_then(|value| parse_mailboxes(&value).into_iter().next())
}

fn parse_recipients(parsed: &ParsedMail<'_>) -> Vec<InboundRecipient> {
    [
        ("To", InboundRecipientKind::To),
        ("Cc", InboundRecipientKind::Cc),
    ]
    .into_iter()
    .flat_map(|(header, kind)| {
        parsed
            .headers
            .get_first_value(header)
            .map(|value| {
                parse_mailboxes(&value)
                    .into_iter()
                    .enumerate()
                    .map(move |(position, mailbox)| InboundRecipient {
                        kind,
                        mailbox,
                        position: position as i32,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    })
    .collect()
}

fn parse_mailboxes(value: &str) -> Vec<InboundMailbox> {
    addrparse(value)
        .map(|addresses| {
            addresses
                .iter()
                .flat_map(mail_addr_singles)
                .map(mailbox_from_single)
                .collect()
        })
        .unwrap_or_default()
}

fn mail_addr_singles(address: &MailAddr) -> Vec<&SingleInfo> {
    match address {
        MailAddr::Single(single) => vec![single],
        MailAddr::Group(group) => group.addrs.iter().collect(),
    }
}

fn mailbox_from_single(single: &SingleInfo) -> InboundMailbox {
    InboundMailbox {
        address: single.addr.clone(),
        address_normalized: single.addr.to_ascii_lowercase(),
        display_name: single.display_name.clone().unwrap_or_default(),
    }
}

fn parse_references(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split_whitespace()
        .map(str::trim)
        .filter(|reference| !reference.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{InboundParseError, parse_raw_mime};
    use crate::inbound::limits::IngestLimits;
    use crate::inbound::types::{InboundRecipientKind, ParsedInboundMessage};

    fn bytes(name: &str) -> &'static [u8] {
        match name {
            "simple_text.eml" => include_bytes!("../../tests/fixtures/inbound/simple_text.eml"),
            "multipart_alternative.eml" => {
                include_bytes!("../../tests/fixtures/inbound/multipart_alternative.eml")
            }
            "nested_multipart.eml" => {
                include_bytes!("../../tests/fixtures/inbound/nested_multipart.eml")
            }
            "with_attachments.eml" => {
                include_bytes!("../../tests/fixtures/inbound/with_attachments.eml")
            }
            "threaded_reply.eml" => {
                include_bytes!("../../tests/fixtures/inbound/threaded_reply.eml")
            }
            "display_name_spoof.eml" => {
                include_bytes!("../../tests/fixtures/inbound/display_name_spoof.eml")
            }
            "html_only_safe.eml" => {
                include_bytes!("../../tests/fixtures/inbound/html_only_safe.eml")
            }
            "html_only_dangerous.eml" => {
                include_bytes!("../../tests/fixtures/inbound/html_only_dangerous.eml")
            }
            "text_and_html_prefers_text.eml" => {
                include_bytes!("../../tests/fixtures/inbound/text_and_html_prefers_text.eml")
            }
            _ => unreachable!("unknown fixture"),
        }
    }

    #[test]
    fn inbound_mime_parses_simple_text() {
        let message = parse_raw_mime(bytes("simple_text.eml"), IngestLimits::default()).unwrap();

        assert_eq!(message.subject, "Simple text");
        assert!(message.body_text.contains("Hello from plain text."));
        assert_eq!(message.attachments.len(), 0);
    }

    #[test]
    fn inbound_mime_parses_multipart_alternative() {
        let message =
            parse_raw_mime(bytes("multipart_alternative.eml"), IngestLimits::default()).unwrap();

        assert!(message.body_text.contains("Plain alternative body."));
        assert_eq!(message.attachments.len(), 0);
    }

    #[test]
    fn inbound_mime_accepts_nested_multipart_within_limit() {
        let message = parse_raw_mime(
            bytes("nested_multipart.eml"),
            IngestLimits::new(25 * 1024 * 1024, 4, 25),
        )
        .unwrap();

        assert!(message.body_text.contains("Nested plain body."));
    }

    #[test]
    fn inbound_mime_extracts_attachment_refs() {
        let message =
            parse_raw_mime(bytes("with_attachments.eml"), IngestLimits::default()).unwrap();

        assert_eq!(message.attachments.len(), 2);
        assert_eq!(message.attachments[0].filename, "invoice.pdf");
        assert_eq!(message.attachments[0].content_type, "application/pdf");
        assert!(message.body_text.contains("Attached metadata only."));
    }

    #[test]
    fn inbound_mime_rejects_raw_messages_over_size_limit() {
        let error = parse_raw_mime(
            bytes("simple_text.eml"),
            IngestLimits::new(bytes("simple_text.eml").len() - 1, 20, 25),
        )
        .unwrap_err();

        assert_eq!(
            error,
            InboundParseError::LimitExceeded {
                limit: "raw_mime_bytes"
            }
        );
    }

    #[test]
    fn inbound_mime_rejects_multipart_depth_over_limit() {
        let error = parse_raw_mime(
            bytes("nested_multipart.eml"),
            IngestLimits::new(25 * 1024 * 1024, 1, 25),
        )
        .unwrap_err();

        assert_eq!(
            error,
            InboundParseError::LimitExceeded {
                limit: "mime_depth"
            }
        );
    }

    #[test]
    fn inbound_mime_rejects_attachment_count_over_limit() {
        let error = parse_raw_mime(
            bytes("with_attachments.eml"),
            IngestLimits::new(25 * 1024 * 1024, 20, 1),
        )
        .unwrap_err();

        assert_eq!(
            error,
            InboundParseError::LimitExceeded {
                limit: "attachment_count"
            }
        );
    }

    #[test]
    fn inbound_text_mime_uses_html_fallback() {
        let message = parse_raw_mime(bytes("html_only_safe.eml"), IngestLimits::default()).unwrap();

        assert!(message.body_text.contains("Hello from HTML."));
        assert!(message.body_text.contains("- First item"));
        assert!(message.body_text.contains("https://example.test/path"));
        assert!(!message.body_text.contains("<strong>"));
    }

    #[test]
    fn inbound_text_mime_strips_dangerous_html() {
        let message =
            parse_raw_mime(bytes("html_only_dangerous.eml"), IngestLimits::default()).unwrap();

        assert!(message.body_text.contains("Visible body"));
        assert!(message.body_text.contains("javascript:alert(1)"));
        assert!(message.body_text.contains("data:text/html,hello"));
        assert!(!message.body_text.contains("tracker.example"));
        assert!(!message.body_text.contains("alert(\"xss\")"));
        assert!(!message.body_text.contains("background-image"));
    }

    #[test]
    fn inbound_text_mime_prefers_text_plain_over_html() {
        let message = parse_raw_mime(
            bytes("text_and_html_prefers_text.eml"),
            IngestLimits::default(),
        )
        .unwrap();

        assert_eq!(message.body_text, "Preferred plain body.");
    }

    #[test]
    fn inbound_mime_extracts_inbound_metadata() {
        let message = parse_raw_mime(bytes("threaded_reply.eml"), IngestLimits::default()).unwrap();

        assert_sender_metadata(&message);
        assert_thread_metadata(&message);
        assert_inbound_recipients(&message);
    }

    fn assert_sender_metadata(message: &ParsedInboundMessage) {
        assert_eq!(message.from.address, "sender@example.test");
        assert_eq!(message.from.address_normalized, "sender@example.test");
        assert_eq!(message.from.display_name, "Responder");
        assert_eq!(message.subject, "Re: Project thread");
    }

    fn assert_thread_metadata(message: &ParsedInboundMessage) {
        assert_eq!(message.message_date_epoch, Some(1781113200));
        assert_eq!(
            message.rfc_message_id.as_deref(),
            Some("<reply@example.test>")
        );
        assert_eq!(
            message.in_reply_to.as_deref(),
            Some("<previous@example.test>")
        );
        assert_eq!(
            message.reference_ids,
            vec![
                "<root@example.test>".to_string(),
                "<previous@example.test>".to_string()
            ]
        );
    }

    fn assert_inbound_recipients(message: &ParsedInboundMessage) {
        assert_eq!(message.recipients.len(), 3);
        assert_eq!(message.recipients[0].kind, InboundRecipientKind::To);
        assert_eq!(message.recipients[0].mailbox.address, "contact@ahara.io");
        assert_eq!(message.recipients[1].mailbox.address, "chris@ahara.io");
        assert_eq!(message.recipients[2].kind, InboundRecipientKind::Cc);
        assert_eq!(message.recipients[2].mailbox.address, "support@ahara.io");
    }

    #[test]
    fn inbound_mime_does_not_trust_spoofed_display_name_as_identity() {
        let message =
            parse_raw_mime(bytes("display_name_spoof.eml"), IngestLimits::default()).unwrap();

        assert_eq!(message.from.display_name, "contact@ahara.io");
        assert_eq!(message.from.address, "attacker@example.test");
        assert_eq!(message.from.address_normalized, "attacker@example.test");
    }

    #[test]
    fn inbound_mime_attachment_metadata_excludes_body_bytes() {
        let message =
            parse_raw_mime(bytes("with_attachments.eml"), IngestLimits::default()).unwrap();

        assert_eq!(message.attachments.len(), 2);
        assert_eq!(
            message.attachments[0].content_id.as_deref(),
            Some("<invoice-content>")
        );
        assert_eq!(message.attachments[0].size_bytes, Some(14));
    }
}
