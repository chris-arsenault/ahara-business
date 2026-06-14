locals {
  prefix       = "ahara-business"
  domain_name  = "ahara.io"
  product_name = "Ahara Mail"

  frontend_hostname             = "mail.ahara.io"
  api_hostname                  = "api.mail.ahara.io"
  access_api_url                = "https://api.access.ahara.io"
  app_authorizations_table_name = "ahara-business-app-authorizations"

  mail_domain     = local.domain_name
  raw_mail_bucket = "${local.prefix}-raw-mail-${data.aws_caller_identity.current.account_id}"
  raw_mail_prefix = "raw/"

  raw_mail_current_retention_days            = 365
  raw_mail_noncurrent_version_retention_days = 30
  raw_mail_abort_incomplete_multipart_days   = 1
  ses_bounce_topic_name                      = "${local.prefix}-ses-bounces"
  ses_complaint_topic_name                   = "${local.prefix}-ses-complaints"
  mail_alarm_topic_name                      = "${local.prefix}-mail-alarms"
  ses_inbound_mx_record                      = "10 inbound-smtp.us-east-1.amazonaws.com"

  accepted_mail_recipients = [
    "chris@${local.mail_domain}",
    "contact@${local.mail_domain}",
  ]

  receipt_gate_per_recipient_hourly_limit = 120
  receipt_gate_total_hourly_limit         = 240
  receipt_gate_window_seconds             = 3600
  mail_metric_namespace                   = "AharaBusiness/Mail"
  mail_alarm_period_seconds               = 300

  lambda_reserved_concurrency = {
    api              = 20
    receipt_gate     = 2
    ingest           = 5
    send_worker      = 2
    feedback_handler = 2
  }

  mail_lambda_alarm_functions = {
    api              = module.api.function_names["api"]
    receipt_gate     = module.receipt_gate.function_name
    ingest           = module.ingest.function_name
    send_worker      = module.send_worker.function_name
    feedback_handler = module.feedback_handler.function_name
  }

  mail_app_metric_alarms = {
    inbound_failed = {
      metric_name = "InboundFailed"
      handler     = "ingest"
    }
    outbound_failed = {
      metric_name = "OutboundFailed"
      handler     = "send-worker"
    }
    feedback_complained = {
      metric_name = "FeedbackComplained"
      handler     = "feedback-handler"
    }
    inbound_gate_blocked = {
      metric_name = "InboundGateBlocked"
      handler     = "receipt-gate"
    }
    inbound_oversize_rejected = {
      metric_name = "InboundOversizeRejected"
      handler     = "ingest"
    }
    inbound_hourly_bytes_rejected = {
      metric_name = "InboundHourlyBytesRejected"
      handler     = "ingest"
    }
  }

  db_env = {
    DB_HOST     = module.ctx.rds_address
    DB_PORT     = module.ctx.rds_port
    DB_NAME     = nonsensitive(data.aws_ssm_parameter.db_database.value)
    DB_USERNAME = nonsensitive(data.aws_ssm_parameter.db_username.value)
    DB_PASSWORD = nonsensitive(data.aws_ssm_parameter.db_password.value)
  }

  common_env = merge(local.db_env, {
    COGNITO_USER_POOL_ID          = module.ctx.cognito_user_pool_id
    COGNITO_CLIENT_ID             = module.cognito_app.client_id
    COGNITO_DOMAIN                = module.ctx.cognito_domain
    COGNITO_ISSUER                = module.ctx.cognito_issuer
    API_BASE_URL                  = "https://${local.api_hostname}"
    APP_BASE_URL                  = "https://${local.frontend_hostname}"
    MAIL_DOMAIN                   = local.mail_domain
    RAW_MAIL_BUCKET               = aws_s3_bucket.raw_mail.bucket
    RAW_MAIL_PREFIX               = local.raw_mail_prefix
    SES_BOUNCE_TOPIC_ARN          = aws_sns_topic.ses_bounces.arn
    SES_COMPLAINT_TOPIC_ARN       = aws_sns_topic.ses_complaints.arn
    APP_AUTHORIZATIONS_TABLE_NAME = local.app_authorizations_table_name
  })

  receipt_gate_env = {
    MAIL_DOMAIN                             = local.mail_domain
    ACCEPTED_MAIL_RECIPIENTS                = join(",", local.accepted_mail_recipients)
    RECEIPT_GATE_PER_RECIPIENT_HOURLY_LIMIT = tostring(local.receipt_gate_per_recipient_hourly_limit)
    RECEIPT_GATE_TOTAL_HOURLY_LIMIT         = tostring(local.receipt_gate_total_hourly_limit)
    RECEIPT_GATE_WINDOW_SECONDS             = tostring(local.receipt_gate_window_seconds)
  }
}
