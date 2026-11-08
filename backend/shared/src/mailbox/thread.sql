SELECT id, direction, status,
       sent_at::text AS sent_at, created_at::text AS created_at,
       thread_id, rfc_message_id, in_reply_to, reference_ids,
       from_address, from_display_name, subject,
       message_date::text AS message_date, received_at::text AS received_at,
       body_text, is_read, contact_id, spf_result, dkim_result, dmarc_result,
       auth_verdict, spam_result, virus_result, security_disposition, security_reason
FROM messages
WHERE thread_id = $1
  AND security_disposition = 'accepted'
  AND ((direction = 'inbound' AND status = 'received') OR direction = 'outbound')
ORDER BY COALESCE(received_at, sent_at, created_at), created_at, id
