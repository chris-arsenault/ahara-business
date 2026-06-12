# Deploy

This project follows the shared Ahara platform workflow. Local deploys use
`scripts/deploy.sh`; CI replicates the same build, migration, and Terraform
steps through the shared reusable workflow.

## Prerequisites

- AWS credentials for the project deploy role.
- Platform CLI tools on `PATH`, especially `db-migrate`.
- Rust Lambda build tooling, `cargo lambda`, Terraform, Node.js, and pnpm.
- Access to the shared Terraform state bucket.
- Database parameters present in SSM under `/ahara/db/ahara-business/*`.

## Local validation

Run the same local checks expected before deployment:

```bash
make ci
make build
```

`make ci` runs Rust lint/test checks, frontend lint/type/test checks, and
Terraform formatting. `make build` creates Rust Lambda artifacts and the
frontend bundle consumed by Terraform.

## Platform migrations

Run the platform migration flow before applying infrastructure that depends on
new schema:

```bash
db-migrate
```

The local deploy script runs this step automatically. CI uses the shared
platform migration action instead of calling the local script.

## Terraform

Local Terraform uses the shared state bucket and the project state key declared
in `infrastructure/terraform/main.tf`.

```bash
terraform -chdir=infrastructure/terraform init -reconfigure \
  -backend-config="bucket=${STATE_BUCKET:-tfstate-559098897826}" \
  -backend-config="region=${STATE_REGION:-us-east-1}" \
  -backend-config="use_lockfile=true"

terraform -chdir=infrastructure/terraform validate
terraform -chdir=infrastructure/terraform plan
terraform -chdir=infrastructure/terraform apply
```

Use `scripts/deploy.sh` for the full local build, migration, init, and apply
path:

```bash
scripts/deploy.sh
```

## CI difference

CI must not call `scripts/deploy.sh`. The shared workflow reads `platform.yml`,
builds the declared Rust Lambda artifacts, builds the frontend, runs platform
migrations, and applies Terraform with CI OIDC credentials. Repo-specific
integration checks belong in `rust_extra_ci_commands`.

## AWS credentials

Local deploys require credentials that can read/write the shared state bucket,
read project SSM database parameters, deploy Lambda artifacts, manage the
project ALB/API/frontend resources, manage the project SES/S3/SNS/Route53 mail
resources, and apply the existing shared module resources. Prefer role-based
credentials; never commit secrets.

## Release outputs

After apply, capture these outputs for the smoke procedure:

```bash
terraform -chdir=infrastructure/terraform output api_url
terraform -chdir=infrastructure/terraform output frontend_url
terraform -chdir=infrastructure/terraform output receipt_rule_set_name
terraform -chdir=infrastructure/terraform output receipt_gate_function_name
terraform -chdir=infrastructure/terraform output ingest_function_name
terraform -chdir=infrastructure/terraform output raw_mail_bucket_name
terraform -chdir=infrastructure/terraform output mail_mx_record
terraform -chdir=infrastructure/terraform output alarm_topic_arn
```

Then run the controlled [smoke check](smoke-check.md).
