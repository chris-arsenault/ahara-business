module "cognito_app" {
  source  = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/cognito-app"
  name    = "${local.prefix}-app"
  cognito = module.ctx.cognito
}

resource "aws_ssm_parameter" "auth_trigger_client" {
  name  = "/ahara/auth-trigger/clients/ahara-business-app"
  type  = "String"
  value = module.cognito_app.client_id
}
