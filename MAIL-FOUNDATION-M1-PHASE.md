# M1 - Shared Cognito Strong Auth

This expands M1 from `MAIL-FOUNDATION-PLAN.md` into execution-ready steps. Run
only these steps for M1, in order. M1 carries one `[DECISION]` because ADR-0003
settles the requirement for strong shared Cognito auth, but the milestone text
still names two possible mechanisms: required TOTP or passkey/WebAuthn support.

Exit gate: `make ci` green in `ahara-business`; `make ci` green in
`ahara-infra`; Cognito strong-auth Terraform plans cleanly; the
`ahara-business` Cognito app-client ID is still the client used by frontend
runtime config, pre-auth user-access gating, and ALB `jwt-validation`.

1. Confirm the shared Cognito strong-auth mechanism.  [DECISION]
   - File(s): none; this step records the user-owned semantic choice before
     changing shared authentication behavior.
   - Reference behavior: ADR-0002 makes the public authenticated surface
     load-bearing, and ADR-0003 requires shared Cognito to provide a strong-auth
     posture. AWS Cognito supports required MFA with TOTP; WebAuthn passkeys can
     satisfy MFA only when the user pool WebAuthn configuration enables that
     factor behavior. The current repo does not already encode either mechanism.
   - Change: no code change. Confirm that M1 should implement required TOTP MFA
     as the concrete mechanism. If the answer is passkey/WebAuthn instead, stop
     and revise this phase file before execution; do not improvise a Terraform
     workaround for provider support.
   - Verify: none; the executor stops here because this is a semantic product
     and security decision.

2. Configure the shared Cognito user pool for required TOTP MFA.  [depends on #1]
   - File(s): `../ahara-infra/infrastructure/terraform/services/modules/cognito/main.tf`.
   - Reference behavior: ADR-0003 keeps the shared Cognito user pool as the
     strong-auth boundary. AWS Cognito required MFA is represented by
     `MfaConfiguration = ON`, and TOTP is represented by
     `SoftwareTokenMfaConfiguration.Enabled = true`. Keep existing app-client
     auth flows and the pre-auth Lambda trigger intact.
   - Change: add `mfa_configuration = "ON"` and
     `software_token_mfa_configuration { enabled = true }` to
     `aws_cognito_user_pool.pool`. Do not add SMS MFA, email MFA, OTP first
     factors, a second user pool, or project-specific auth code.
   - Verify: from `../ahara-infra`, run
     `! rg 'mfa_configuration\\s*=\\s*"ON"|software_token_mfa_configuration' infrastructure/terraform/services/modules/cognito/main.tf`
     before editing. After editing, run
     `terraform fmt -check -recursive infrastructure/terraform/services`,
     `rg 'mfa_configuration\\s*=\\s*"ON"' infrastructure/terraform/services/modules/cognito/main.tf`, and
     `rg 'software_token_mfa_configuration' infrastructure/terraform/services/modules/cognito/main.tf`.
     Red before: the user pool has no required MFA/TOTP settings. Green after.

3. Register the `ahara-business` app client with pre-auth gating.  [depends on #2]
   - File(s): `../ahara-infra/infrastructure/terraform/control/project-ahara-business.tf`,
     `infrastructure/terraform/cognito.tf`.
   - Reference behavior: `../ahara/INTEGRATION.md` Step 6 keeps app-client
     creation in each project through `ahara-tf-patterns/modules/cognito-app`.
     `../ahara-infra/infrastructure/terraform/services/auth-trigger.tf` reads
     `/ahara/auth-trigger/clients/*` to map external client IDs to app keys
     before the pre-auth Lambda checks the `websites-user-access` table.
   - Change: keep `module "cognito_app"` as the only Cognito client creator.
     Add a project-owned SSM parameter in `ahara-business` Terraform at
     `/ahara/auth-trigger/clients/ahara-business-app` with value
     `module.cognito_app.client_id`. Add only the scoped deployer permission this
     requires in `project-ahara-business.tf`: include `ssm-write` and
     `ssm_additional_parameter_paths = ["ahara/auth-trigger/clients/ahara-business-app"]`.
     Do not create another app client or bypass the pre-auth Lambda.
   - Verify: before editing, run
     `! rg 'ahara/auth-trigger/clients/ahara-business-app' ../ahara-infra/infrastructure/terraform/control/project-ahara-business.tf infrastructure/terraform/cognito.tf`.
     After editing, run
     `terraform fmt -check -recursive infrastructure/terraform/`,
     `cd ../ahara-infra && terraform fmt -check -recursive infrastructure/terraform/control`,
     `rg '"ssm-write"' ../ahara-infra/infrastructure/terraform/control/project-ahara-business.tf`,
     `rg 'ahara/auth-trigger/clients/ahara-business-app' ../ahara-infra/infrastructure/terraform/control/project-ahara-business.tf infrastructure/terraform/cognito.tf`, and
     `rg 'value\\s*=\\s*module\\.cognito_app\\.client_id' infrastructure/terraform/cognito.tf`.
     Red before: the auth-trigger client registration path is absent. Green after.

4. Add pre-auth compatibility tests for external app clients.  [depends on #3]
   - File(s): `../ahara-infra/backend/auth-trigger/src/main.rs`.
   - Reference behavior: the pre-auth Lambda gates sign-in by reading
     `callerContext.clientId`, resolving that client ID to an app key, and
     checking the user's `apps` map. Strong-auth settings must not change the
     app-client ID contract that `ahara-business` publishes in step 3.
   - Change: extract the client-map/user-app authorization decision into a
     small pure helper inside `auth-trigger` and cover it with unit tests:
     seeded admin bypass is allowed, an unknown client is denied, a known client
     without user app access is denied, and a known client with user app access
     is allowed. Keep runtime AWS SSM/DynamoDB calls and the Lambda event shape
     unchanged.
   - Verify: before editing, run
     `! rg 'authorizes_known_external_app_client' ../ahara-infra/backend/auth-trigger/src/main.rs`.
     After editing, run
     `cd ../ahara-infra/backend && cargo fmt --check && cargo test -p auth-trigger authorizes_known_external_app_client`.
     Red before: the named compatibility test/contract does not exist. Green after.

5. Verify the Cognito/JWT auth contract remains wired end to end.  [depends on #2, #3, #4]
   - File(s): none; this is a contract verification step across the files
     changed above and the existing platform modules.
   - Reference behavior: `../ahara/INTEGRATION.md` uses `cognito-app` for the
     frontend client, frontend runtime config for the pool/client IDs, and
     `alb-api` `jwt-validation` for authenticated API routes. ALB
     `jwt-validation` validates issuer/JWKS from shared Cognito and does not
     replace the user-pool MFA or pre-auth responsibilities.
   - Change: no code change unless one of these checks fails because a previous
     M1 step broke the contract.
   - Verify: run
     `rg 'module "cognito_app"' infrastructure/terraform/cognito.tf`,
     `rg 'cognitoClientId\\s*=\\s*module\\.cognito_app\\.client_id' infrastructure/terraform/frontend.tf`,
     `rg 'authenticated\\s*=\\s*true' infrastructure/terraform/lambdas.tf`,
     `rg 'jwt-validation' ../ahara-tf-patterns/modules/alb-api/main.tf`, and
     `rg 'auth-trigger/clients' ../ahara-infra/infrastructure/terraform/services/auth-trigger.tf infrastructure/terraform/cognito.tf`.
     Red before: step 3's project auth-trigger client registration is absent.
     Green after.

6. Run the M1 exit gate.  [depends on #1, #2, #3, #4, #5]
   - File(s): none; this is the phase gate.
   - Reference behavior: M1 exit in `MAIL-FOUNDATION-PLAN.md` and
     `../ahara-infra/AGENTS.md`.
   - Change: no code change.
   - Verify: run `make ci` from `/home/dev/repos/ahara-business`, run `make ci`
     from `/home/dev/repos/ahara-infra`, then run
     `terraform -chdir=/home/dev/repos/ahara-infra/infrastructure/terraform init -backend=false`
     and
     `terraform -chdir=/home/dev/repos/ahara-infra/infrastructure/terraform plan -refresh=false -target=module.services.module.cognito.aws_cognito_user_pool.pool -out=/tmp/ahara-business-m1-cognito.tfplan`.
     Finally capture `git status --short` in both repos. Red before: shared
     Cognito has no required strong-auth setting and `ahara-business` is not
     registered in the pre-auth external client map. Green after.
