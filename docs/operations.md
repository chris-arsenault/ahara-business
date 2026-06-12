# Operations

## S3 and IAM hardening review

The mail infrastructure is project-owned and uses scoped platform resources.
This review records the M8 release hardening checks for inbound mail.
Use the controlled [smoke check](smoke-check.md) after deployment.

### Raw MIME storage

- Raw MIME lands in the project raw-mail bucket under the configured raw
  prefix only.
- The raw MIME bucket public-access block disables public ACLs, public bucket
  policies, and public bucket exposure.
- The raw MIME bucket uses server-side encryption with AES-256 and bucket keys.
- Bucket versioning is enabled.
- Lifecycle controls expire current raw MIME objects after the configured
  retention period, expire noncurrent versions, and abort incomplete multipart
  uploads.

### Scoped write and read access

- `AllowSesReceiptRuleWrite` allows SES to write only to the raw MIME prefix
  and only from this AWS account.
- `ListRawMailPrefix` allows Lambda list access only for the raw MIME prefix.
- `ReadWriteRawMailObjects` allows Lambda raw object access only under the raw
  MIME prefix.
- `SendFromMailIdentity` allows outbound send only from the project SES domain
  identity.
- `AllowSesFeedbackPublish` allows SES feedback publish only to the bounce and
  complaint topics and only from this AWS account.

### Inbound flood controls

- SES receipt routing is address-scoped to `chris@ahara.io` and
  `contact@ahara.io`; SES also matches plus-address labels for those explicit
  addresses.
- The pre-S3 receipt gate runs synchronously before storage. It stops unknown
  recipients and applies count-only rolling limits of 120 messages per accepted
  recipient per hour and 240 accepted-recipient messages per hour.
- The receipt gate does not fetch S3, parse MIME, persist mailbox rows, send
  mail, or log sender-controlled mail content.
- Post-S3 ingest performs a raw object `HeadObject` before body download and
  rejects objects over 10 MiB without parsing the message.
- Post-S3 ingest applies a rolling-byte cap of 50 MiB per hour from inbound
  audit rows. Messages rejected by the rolling-byte cap are stored only as
  minimal rejected audit rows.

### Activation

- MX activation points `ahara.io` at the SES inbound endpoint for us-east-1.
- The project SES receipt rule set is active.
- The active rule invokes the receipt gate first, writes accepted mail to S3
  second, and invokes async ingest third.
- The rule remains address-scoped and does not use domain-wide catch-all
  recipients.
