import type {
  MailboxMessageDetail,
  MailboxMessageSummary,
  OutboundMessageDetail,
  OutboundMessageSummary,
} from "./types";

export const unreadMessage: MailboxMessageSummary = {
  id: "message-1",
  thread_id: "thread-1",
  from_address: "sender@example.test",
  from_display_name: "Sender Display",
  subject: "Invoice",
  snippet: "Plaintext invoice body",
  received_at: "2026-01-01 00:00:00+00",
  is_read: false,
  has_attachments: true,
  attachment_count: 2,
  contact_id: null,
  auth_verdict: "pass",
  spam_result: "pass",
  virus_result: "pass",
  security_disposition: "accepted",
};

export const readMessage: MailboxMessageSummary = {
  ...unreadMessage,
  is_read: true,
};

export const messageDetail: MailboxMessageDetail = {
  id: "message-1",
  direction: "inbound",
  status: "received",
  sent_at: null,
  created_at: "2026-01-01 00:00:00+00",
  thread_id: "thread-1",
  rfc_message_id: "<message-1@example.test>",
  in_reply_to: null,
  reference_ids: [],
  from_address: "sender@example.test",
  from_display_name: "Sender Display",
  subject: "Invoice",
  message_date: "2026-01-01 00:00:00+00",
  received_at: "2026-01-01 00:00:00+00",
  body_text:
    "<script>alert(1)</script>\nVisit javascript:alert(1)\nOpen data:text/html;base64,abc",
  recipients: [],
  attachments: [
    {
      id: "attachment-1",
      position: 0,
      filename: "../secret/invoice.pdf",
      display_filename: "../secret/invoice.pdf",
      content_type: "application/pdf",
      size_bytes: 2048,
      content_id: null,
    },
  ],
  is_read: false,
  contact_id: null,
  spf_result: "pass",
  dkim_result: "pass",
  dmarc_result: "pass",
  auth_verdict: "pass",
  spam_result: "pass",
  virus_result: "pass",
  security_disposition: "accepted",
  security_reason: "clean",
};

export const sentMessage: OutboundMessageSummary = {
  id: "outbound-1",
  thread_id: "thread-1",
  status: "queued",
  from_address: "contact@ahara.io",
  subject: "Plain note",
  snippet: "Queued body",
  primary_recipient: "person@example.com",
  recipient_count: 1,
  last_error: null,
  sent_at: null,
  created_at: "2026-01-01 00:00:00+00",
};

export const sentMessageDetail: OutboundMessageDetail = {
  id: "outbound-1",
  source_message_id: null,
  thread_id: "thread-1",
  rfc_message_id: "<outbound-1@ahara.io>",
  in_reply_to: null,
  reference_ids: [],
  status: "queued",
  from_address: "contact@ahara.io",
  from_address_normalized: "contact@ahara.io",
  subject: "Plain note",
  body_text: "Queued body\n<script>alert(1)</script>",
  recipients: [
    {
      kind: "to",
      address: "person@example.com",
      address_normalized: "person@example.com",
      display_name: "",
      position: 0,
    },
  ],
  attachments: [],
  last_error: null,
  sent_at: null,
  created_at: "2026-01-01 00:00:00+00",
};
