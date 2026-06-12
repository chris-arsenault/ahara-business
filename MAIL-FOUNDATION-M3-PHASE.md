# M3 - Mail AWS Infrastructure

This phase expands only M3 from `MAIL-FOUNDATION-PLAN.md`. The goal is to add
the project-owned AWS mail primitives needed for SES/S3/SNS transport while
respecting the latest routability constraint: do not make inbound mail publicly
routable yet.

M3 routability guard:
- Do not add Route53 MX records.
- Do not add `aws_ses_active_receipt_rule_set`.
- It is acceptable to create a dormant project receipt rule set and disabled
  receipt rule so the S3 + ingest wiring is reviewable before activation.
- The system must remain unroutable for inbound mail at the end of M3.

Reference context:
- `MAIL-FOUNDATION-PLAN.md`
- `mail-foundation-spec.md`
- `docs/adr/0004-project-owned-mail-infrastructure.md`
- `../ahara/INTEGRATION.md`
- `../ahara-tf-patterns/modules/lambda/`
- `../ahara-tf-patterns/modules/alb-api/`
- `../ahara-infra/infrastructure/terraform/control/modules/managed-project/`
- `../ahara-infra/infrastructure/terraform/control/modules/policy-library/`

Exit gate:
- `make ci` in `ahara-business`.
- If Step 2 changes `ahara-infra`, run its affected control-layer checks before
  the project plan.
- Terraform plan for `ahara-business` shows scoped SES/S3/SNS/Lambda/
  CloudWatch resources only.
- Terraform plan and source scan show no MX records and no active SES receipt
  rule set.

## Step 1 - [DECISION] Choose raw MIME lifecycle retention defaults

Files:
- None

Decision:
- Accepted on 2026-06-10: expire current raw MIME objects after 365 days,
  expire noncurrent object versions after 30 days, and abort incomplete
  multipart uploads after 1 day.

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M3 names raw MIME lifecycle retention defaults as
  an explicit decision.
- `mail-foundation-spec.md` keeps original raw MIME private in S3 as the escape
  hatch for attachments and parsing recovery, and leaves retention "per infra".
- ADR-0004 puts raw MIME storage under project-owned Terraform.

Change:
- Use the confirmed lifecycle defaults: current objects 365 days, noncurrent
  versions 30 days, incomplete multipart uploads 1 day.

Verify:
- No command. This is a required lifecycle decision before S3 lifecycle DDL.

## Step 2 - Add scoped deployer support for bare SES identity and alarms

Files:
- `../ahara-infra/infrastructure/terraform/control/modules/policy-library/ses/variables.tf`
- `../ahara-infra/infrastructure/terraform/control/modules/policy-library/ses/iam.tf`
- `../ahara-infra/infrastructure/terraform/control/modules/policy-library/cloudwatch-alarms/variables.tf`
- `../ahara-infra/infrastructure/terraform/control/modules/policy-library/cloudwatch-alarms/iam.tf`
- `../ahara-infra/infrastructure/terraform/control/modules/policy-library/cloudwatch-alarms/outputs.tf`
- `../ahara-infra/infrastructure/terraform/control/modules/managed-project/variables.tf`
- `../ahara-infra/infrastructure/terraform/control/modules/managed-project/policy-map.tf`
- `../ahara-infra/infrastructure/terraform/control/project-ahara-business.tf`

Reference behavior:
- ADR-0004 requires project-owned mail infrastructure with least-privilege
  deployer policy primitives.
- The current `ses` primitive scopes destructive identity actions to
  `identity/${prefix}-*`, but M3's actual mail identity is the bare domain
  `ahara.io`.
- M3 requires CloudWatch alarms; the current `ahara-business` deployer policy
  does not include a CloudWatch alarm primitive.
- `MAIL-FOUNDATION-PLAN.md` M3 exit requires no broad deployer grants.

Change:
- Extend the `ses` primitive with an optional exact-domain identity allowlist,
  defaulting to empty. Include `ahara.io` for `project_ahara_business` so
  Terraform can manage that exact SES identity without opening all identities.
- Add a `cloudwatch-alarms` primitive scoped to alarms named
  `${prefix}-*`, using wildcard resources only for CloudWatch actions that do
  not support alarm ARNs.
- Register `cloudwatch-alarms` in the managed-project policy map and add it to
  `project_ahara_business.policy_modules`.

Verify:
- Before the change:
  `! rg "additional_ses_identity|cloudwatch-alarms" ../ahara-infra/infrastructure/terraform/control`
- After the change:
  `terraform fmt -check -recursive ../ahara-infra/infrastructure/terraform/control`
  and
  `rg "additional_ses_identity|cloudwatch-alarms|ahara.io" ../ahara-infra/infrastructure/terraform/control`

## Step 3 - Add mail infrastructure locals and account context

Files:
- `infrastructure/terraform/locals.tf`
- `infrastructure/terraform/mail_data.tf`

Reference behavior:
- `mail-foundation-spec.md` configured-domain routing uses the accepted domain
  `ahara.io`.
- M2 seed data established application-side accepted addresses for `ahara.io`
  without any public route.
- S3 bucket names are global, so project buckets should include the AWS account
  id while remaining under the `ahara-business-*` namespace for deployer
  policy scoping.

Change:
- Add `data "aws_caller_identity" "current"`.
- Add mail locals for:
  - `mail_domain = local.domain_name`
  - raw MIME bucket name under `${local.prefix}-*`
  - raw MIME object prefix
  - lifecycle values from Step 1
  - feedback/alarm topic names under `${local.prefix}-*`

Verify:
- Before the change:
  `! rg "aws_caller_identity|raw_mail_bucket|mail_domain|raw_mail_prefix" infrastructure/terraform`
- After the change:
  `terraform fmt -check -recursive infrastructure/terraform`
  and
  `rg "aws_caller_identity|raw_mail_bucket|mail_domain|raw_mail_prefix" infrastructure/terraform`

## Step 4 - Add private raw MIME S3 storage  [depends on #1, #3]

Files:
- `infrastructure/terraform/mail_storage.tf`

Reference behavior:
- `mail-foundation-spec.md` requires raw MIME to remain private S3 data with
  public access blocked, encryption at rest, and lifecycle retention.
- ADR-0004 keeps raw MIME storage in project Terraform.
- `../ahara-infra` private-storage deployer primitive scopes buckets to
  `${prefix}-*`.

Change:
- Add the raw MIME bucket, public-access block, SSE-S3 encryption, versioning,
  lifecycle rule from Step 1, and bucket policy allowing SES to write only to
  the raw MIME prefix.
- Do not add website hosting, public ACLs, public bucket policy statements, or
  browser CORS.

Verify:
- Before the change:
  `test ! -f infrastructure/terraform/mail_storage.tf`
- After the change:
  `rg "aws_s3_bucket|aws_s3_bucket_public_access_block|aws_s3_bucket_server_side_encryption_configuration|aws_s3_bucket_lifecycle_configuration" infrastructure/terraform/mail_storage.tf`
  and
  `! rg "PublicRead|block_public_acls\\s*=\\s*false|block_public_policy\\s*=\\s*false|ignore_public_acls\\s*=\\s*false|restrict_public_buckets\\s*=\\s*false" infrastructure/terraform/mail_storage.tf`

## Step 5 - Add SES domain identity and DKIM DNS  [depends on #2, #3]

Files:
- `infrastructure/terraform/mail_ses.tf`

Reference behavior:
- `mail-foundation-spec.md` requires sending from configured-domain addresses
  through SES and per-domain DKIM/verification visibility.
- ADR-0004 keeps SES identities and DNS verification in project Terraform.
- The latest routability guard allows verification/DKIM DNS but forbids MX.

Change:
- Add SES domain identity for `local.mail_domain`.
- Add Route53 TXT verification record for SES domain verification.
- Add SES DKIM resources and Route53 DKIM CNAME records.
- Add SES identity verification dependency if the provider resource is
  available in the current AWS provider.
- Do not add MX records.

Verify:
- Before the change:
  `! rg "aws_ses_domain_identity|aws_ses_domain_dkim" infrastructure/terraform`
- After the change:
  `rg "aws_ses_domain_identity|aws_ses_domain_dkim|_domainkey|_amazonses" infrastructure/terraform/mail_ses.tf`
  and
  `! rg "type\\s*=\\s*\"MX\"" infrastructure/terraform`

## Step 6 - Add dormant SES receipt rule wiring  [depends on #4, #5]

Files:
- `infrastructure/terraform/mail_receiving.tf`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M3 requires receipt rules wired to S3 storage plus
  ingest Lambda invocation.
- `mail-foundation-spec.md` pipeline is MX -> SES -> S3 raw MIME -> ingest
  Lambda, but current user direction forbids public inbound routing until spam
  and reputation controls are ready.

Change:
- Add a project receipt rule set and a disabled receipt rule for
  `local.mail_domain`.
- Wire the disabled receipt rule to write raw MIME into the raw bucket/prefix
  and invoke `module.ingest`.
- Add `aws_lambda_permission` allowing SES to invoke the ingest Lambda for the
  receipt rule.
- Do not add `aws_ses_active_receipt_rule_set`.
- Do not add Route53 MX records.

Verify:
- Before the change:
  `! rg "aws_ses_receipt_rule_set|aws_ses_receipt_rule" infrastructure/terraform`
- After the change:
  `rg "aws_ses_receipt_rule_set|aws_ses_receipt_rule|enabled\\s*=\\s*false|s3_action|lambda_action|aws_lambda_permission" infrastructure/terraform/mail_receiving.tf`
  and
  `! rg "aws_ses_active_receipt_rule_set|type\\s*=\\s*\"MX\"" infrastructure/terraform`

## Step 7 - Add feedback SNS topics and handler wiring  [depends on #5]

Files:
- `infrastructure/terraform/mail_feedback.tf`

Reference behavior:
- `mail-foundation-spec.md` requires SES bounce/complaint SNS events to feed a
  handler that updates suppression and message status later.
- M3 requires SNS feedback topics and Lambda permissions.
- ADR-0004 keeps SNS topics and mail-processing Lambdas under project
  Terraform.

Change:
- Add separate SNS topics for SES bounces and complaints.
- Add SNS subscriptions targeting `module.feedback_handler.function_arn`.
- Add `aws_lambda_permission` resources allowing SNS to invoke the feedback
  handler for those topics.
- Configure the SES identity notification topics for Bounce and Complaint
  events.

Verify:
- Before the change:
  `! rg "aws_sns_topic|aws_sns_topic_subscription|aws_ses_identity_notification_topic" infrastructure/terraform`
- After the change:
  `rg "aws_sns_topic|aws_sns_topic_subscription|aws_ses_identity_notification_topic|Bounce|Complaint|feedback_handler" infrastructure/terraform/mail_feedback.tf`

## Step 8 - Add Lambda runtime mail permissions and environment  [depends on #4, #5, #7]

Files:
- `infrastructure/terraform/mail_iam.tf`
- `infrastructure/terraform/lambdas.tf`
- `infrastructure/terraform/locals.tf`

Reference behavior:
- `../ahara-tf-patterns/modules/alb-api` owns the shared Lambda execution role
  and accepts a single optional inline IAM policy.
- M0 wired standalone worker Lambdas to reuse the API Lambda role.
- `mail-foundation-spec.md` requires ingest to read raw MIME from S3 and send
  worker to send through SES later.

Change:
- Add a Lambda inline IAM policy document scoped to:
  - raw MIME bucket list/read/write only where needed
  - SES send actions scoped to the project mail identity
  - SNS publish only if required by the feedback/operational wiring
- Pass the policy to `module "api"` through `iam_policy`.
- Add mail environment variables to `local.common_env`, including raw bucket,
  raw prefix, mail domain, and feedback topic ARNs.

Verify:
- Before the change:
  `! rg "iam_policy|RAW_MAIL_BUCKET|MAIL_DOMAIN|ses:SendRawEmail|s3:GetObject" infrastructure/terraform/lambdas.tf infrastructure/terraform/locals.tf infrastructure/terraform/mail_iam.tf 2>/dev/null`
- After the change:
  `rg "iam_policy|RAW_MAIL_BUCKET|MAIL_DOMAIN|ses:SendRawEmail|s3:GetObject" infrastructure/terraform`
  and
  `! rg "Action\\s*=\\s*\\[?\\s*\"\\*\"|Resource\\s*=\\s*\\[?\\s*\"\\*\"" infrastructure/terraform/mail_iam.tf`

## Step 9 - Add SES and blast-radius alarms  [depends on #2, #7]

Files:
- `infrastructure/terraform/mail_alarms.tf`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M3 requires CloudWatch alarms for SES reputation
  and cost/blast-radius controls.
- `mail-foundation-spec.md` requires bounce/complaint rate tracking and
  operational cost controls.

Change:
- Add a project alarm SNS topic or reuse the feedback/alarm topic locals from
  Step 3.
- Add CloudWatch alarms for SES reputation bounce rate and complaint rate.
- Add blast-radius alarms for unexpected Lambda invocation volume on ingest and
  send worker, plus a raw MIME bucket growth alarm if the metric is available
  without extra services.
- Keep alarm names under `${local.prefix}-*`.

Verify:
- Before the change:
  `! rg "aws_cloudwatch_metric_alarm" infrastructure/terraform`
- After the change:
  `rg "aws_cloudwatch_metric_alarm|Reputation.BounceRate|Reputation.ComplaintRate|Invocations" infrastructure/terraform/mail_alarms.tf`

## Step 10 - Add outputs and Terraform README updates  [depends on #4, #5, #6, #7, #9]

Files:
- `infrastructure/terraform/outputs.tf`
- `infrastructure/terraform/README.md`

Reference behavior:
- The existing Terraform README documents current modules and says SES/S3/SNS
  resources are planned for M3.
- M3 adds operationally important resource names that should be discoverable
  after apply.

Change:
- Add outputs for raw MIME bucket name/ARN, SES identity ARN, dormant receipt
  rule set name, feedback topic ARNs, and alarm topic ARN.
- Update the Terraform README to describe the M3 mail infrastructure and the
  explicit no-MX/no-active-receipt-rule guard.

Verify:
- Before the change:
  `! rg "raw_mail_bucket|ses_identity|receipt_rule_set|feedback_topic|alarm_topic" infrastructure/terraform/outputs.tf infrastructure/terraform/README.md`
- After the change:
  `rg "raw_mail_bucket|ses_identity|receipt_rule_set|feedback_topic|alarm_topic|no MX|active receipt" infrastructure/terraform/outputs.tf infrastructure/terraform/README.md`

## Step 11 - Run M3 exit gate  [depends on #1, #2, #3, #4, #5, #6, #7, #8, #9, #10]

Files:
- None

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M3 exit requires `make ci` green and a Terraform
  plan showing scoped SES/S3/SNS/Lambda resources only with no broad deployer
  grants.
- Latest routability guard requires no MX and no active SES receipt rule set.

Change:
- No code changes in this step.

Verify:
- Run affected shared-infra checks if Step 2 changed `ahara-infra`:
  `terraform fmt -check -recursive ../ahara-infra/infrastructure/terraform/control`
  and a control-layer Terraform plan using the normal credential wrapper.
- Run project checks:
  `make ci`
- Run project Terraform plan using the normal credential wrapper:
  `/opt/sulion/bin/with-cred -- terraform -chdir=infrastructure/terraform plan -refresh=false -no-color`
- Show the relevant plan output proving:
  - SES identity/DKIM resources are scoped to `ahara.io`
  - raw MIME bucket is under the `ahara-business-*` namespace
  - SNS topics and Lambda permissions are under `ahara-business-*`
  - CloudWatch alarm names are under `ahara-business-*`
  - no `aws_ses_active_receipt_rule_set`
  - no Route53 `MX` record
- Source scan after plan:
  `! rg "aws_ses_active_receipt_rule_set|type\\s*=\\s*\"MX\"" infrastructure/terraform`
