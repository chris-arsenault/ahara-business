# Smoke Check

Run this after a successful deploy to verify the first active mail domain in a
controlled way. Use test-only messages and remove any temporary forwarding rule
created during the check.

## Preflight

Collect Terraform outputs:

```bash
terraform -chdir=infrastructure/terraform output api_url
terraform -chdir=infrastructure/terraform output frontend_url
terraform -chdir=infrastructure/terraform output receipt_rule_set_name
terraform -chdir=infrastructure/terraform output mail_mx_record
terraform -chdir=infrastructure/terraform output raw_mail_bucket_name
terraform -chdir=infrastructure/terraform output alarm_topic_arn
```

Confirm these before sending mail:

- Database migrations are current.
- Cognito login works for the operator account.
- The alarm SNS topic has the intended operator subscription outside
  Terraform.
- SES identity verification and DKIM verification are successful for
  `ahara.io`.
- The active receipt rule set matches the Terraform output.
- The receipt rule recipients are address-scoped to `chris@ahara.io` and
  `contact@ahara.io`.
- The MX record for `ahara.io` points at the SES inbound endpoint.

## Controlled Receive

1. Send one clean plaintext test message to an allowlisted address.
2. Verify a raw object appears under the raw-mail S3 prefix.
3. Verify one accepted inbound mailbox row exists with status `received`,
   security disposition `accepted`, and pass auth/verdict fields.
4. Verify the ingest metrics include one `InboundAccepted` increment and no
   failed or flood-control increments.

## Flood Controls

Use controlled messages or Lambda test events only. Do not run a volume test
from the public internet.

- Unknown recipient: send or invoke the receipt gate with an address outside
  the allowlist and verify `STOP_RULE_SET`, no raw S3 object, and
  `InboundGateBlocked`.
- Oversize raw object: send or stage a test object over 10 MiB for an
  allowlisted recipient and verify a minimal rejected audit row with
  `limit_exceeded_raw_mail_object_bytes`, no normal mailbox row, and
  `InboundOversizeRejected`.
- Hourly byte cap: lower the controlled test environment limit or stage enough
  test events to cross the cap, then verify a minimal rejected audit row with
  `limit_exceeded_recent_raw_mail_bytes`, no forwarding side effect, and
  `InboundHourlyBytesRejected`.

The app must not send app-side bounces for these cases.

## Read

Open the authenticated UI from `frontend_url` and verify:

- The clean received message appears for the authenticated user.
- The body is plaintext only.
- HTML from the original message does not execute or render as trusted markup.
- Sender, auth verdicts, read/unread state, and contact link are visible.
- Quarantined or rejected messages are not mixed into the normal mailbox view.

## Send

1. Compose a text-only message to a controlled recipient.
2. Verify the outbound row is queued.
3. Run or wait for the send worker.
4. Verify the outbound row reaches sent state with an SES provider message id.
5. Verify `OutboundSent` increments and `OutboundFailed` does not.

## Forward

1. Create one address-scoped forwarding rule for a controlled address.
2. Send clean pass-auth inbound mail to that address.
3. Verify exactly one outbound forward is queued/sent.
4. Verify the forward uses `Reply-To` for the original sender.
5. Remove or deactivate the forwarding rule.

## Bounce And Complaint

Publish controlled SNS-shaped feedback events or use the SES simulator where
available:

- Bounce: verify outbound status changes to bounced and the recipient is added
  to suppression.
- Complaint: verify outbound status changes to complained and the recipient is
  added to suppression.
- Verify suppressed recipients are skipped by the send worker and
  `FeedbackBounced`, `FeedbackComplained`, and
  `FeedbackSuppressedRecipients` metrics increment as expected.

## Cleanup

- Remove temporary forwarding rules and test suppression rows if they should
  not remain.
- Confirm no unexpected CloudWatch alarms remain in alarm state.
- Keep the MX record and active receipt rule set in place only if the domain is
  ready to receive live mail.
