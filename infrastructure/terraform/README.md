# Terraform

Project-owned Terraform root for the app and mail infrastructure.

The root uses the shared Ahara state bucket with
`projects/ahara-business.tfstate`, AWS provider defaults, and platform tags. It
discovers shared platform resources through `ahara-tf-patterns` modules and SSM
lookups.

Current modules:

| File | Purpose |
| ---- | ---- |
| `cognito.tf` | Shared Cognito app client |
| `frontend.tf` | Website module for the SPA bundle and runtime config |
| `app_authorizations.tf` | Seeded platform app-authorization item for the operator account |
| `lambdas.tf` | Platform context, ALB API Lambda, and standalone worker Lambdas |
| `mail_alarms.tf` | SES reputation, Lambda volume, and raw-mail bucket alarms |
| `mail_data.tf` | Account context for globally unique mail resource names |
| `mail_feedback.tf` | SES bounce and complaint feedback topics wired to the feedback handler |
| `mail_iam.tf` | Scoped Lambda runtime permissions for raw mail and SES sending |
| `mail_receiving.tf` | Active SES receipt rule set with receipt gate, S3 storage, and ingest invocation |
| `mail_sending.tf` | EventBridge schedule and Lambda permission for the outbound send worker |
| `mail_ses.tf` | SES domain identity, verification TXT record, and DKIM CNAMEs |
| `mail_storage.tf` | Private raw MIME S3 bucket, lifecycle, encryption, and SES write policy |
| `ssm.tf` | Shared database SSM parameter lookups |
| `outputs.tf` | Website, API, Cognito, raw mail bucket, SES identity, receipt rule set, feedback topic, and alarm topic outputs |

The active receipt rule set routes `chris@ahara.io` and `contact@ahara.io`
through the receipt gate, raw-mail S3 storage, and async ingest Lambda. Route 53
publishes the domain MX record for SES inbound in `us-east-1`.

## Verification

```bash
terraform fmt -check -recursive infrastructure/terraform/
```
