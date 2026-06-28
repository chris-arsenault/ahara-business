module "ctx" {
  source = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/platform-context"
}

module "api" {
  source   = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/alb-api"
  prefix   = local.prefix
  hostname = local.api_hostname

  vpc     = module.ctx.vpc
  alb     = module.ctx.alb
  cognito = module.ctx.cognito

  environment = local.common_env
  iam_policy  = [data.aws_iam_policy_document.lambda_mail.json]

  lambdas = {
    api = {
      binary                         = "${path.module}/../../backend/target/lambda/api/bootstrap"
      reserved_concurrent_executions = local.lambda_reserved_concurrency.api
      routes = [
        { priority = 320, paths = ["/health"], methods = ["GET"], authenticated = false },
        { priority = 321, paths = ["/*"], authenticated = true },
      ]
    }
  }
}

module "ingest" {
  source   = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/lambda"
  name     = "${local.prefix}-ingest"
  binary   = "${path.module}/../../backend/target/lambda/ingest/bootstrap"
  role_arn = module.api.role_arn

  vpc                            = module.ctx.vpc
  environment                    = local.common_env
  reserved_concurrent_executions = local.lambda_reserved_concurrency.ingest
}

module "receipt_gate" {
  source   = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/lambda"
  name     = "${local.prefix}-receipt-gate"
  binary   = "${path.module}/../../backend/target/lambda/receipt-gate/bootstrap"
  role_arn = module.api.role_arn

  vpc                            = module.ctx.vpc
  environment                    = local.receipt_gate_env
  reserved_concurrent_executions = local.lambda_reserved_concurrency.receipt_gate
}

module "send_worker" {
  source   = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/lambda"
  name     = "${local.prefix}-send-worker"
  binary   = "${path.module}/../../backend/target/lambda/send-worker/bootstrap"
  role_arn = module.api.role_arn

  vpc                            = module.ctx.vpc
  environment                    = local.common_env
  reserved_concurrent_executions = local.lambda_reserved_concurrency.send_worker
}

module "feedback_handler" {
  source   = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/lambda"
  name     = "${local.prefix}-feedback-handler"
  binary   = "${path.module}/../../backend/target/lambda/feedback-handler/bootstrap"
  role_arn = module.api.role_arn

  vpc                            = module.ctx.vpc
  environment                    = local.common_env
  reserved_concurrent_executions = local.lambda_reserved_concurrency.feedback_handler
}
