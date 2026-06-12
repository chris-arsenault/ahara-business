resource "aws_ses_receipt_rule_set" "mail" {
  rule_set_name = "${local.prefix}-mail"
}

resource "aws_ses_active_receipt_rule_set" "mail" {
  rule_set_name = aws_ses_receipt_rule_set.mail.rule_set_name
}

resource "aws_lambda_permission" "ses_receipt_gate" {
  statement_id   = "AllowSesReceiptGateInvoke"
  action         = "lambda:InvokeFunction"
  function_name  = module.receipt_gate.function_name
  principal      = "ses.amazonaws.com"
  source_account = data.aws_caller_identity.current.account_id
}

resource "aws_lambda_permission" "ses_ingest" {
  statement_id   = "AllowSesReceiptRuleInvoke"
  action         = "lambda:InvokeFunction"
  function_name  = module.ingest.function_name
  principal      = "ses.amazonaws.com"
  source_account = data.aws_caller_identity.current.account_id
}

resource "aws_ses_receipt_rule" "raw_mail_ingest" {
  name          = "${local.prefix}-raw-mail-ingest"
  rule_set_name = aws_ses_receipt_rule_set.mail.rule_set_name
  recipients    = local.accepted_mail_recipients
  enabled       = true
  scan_enabled  = true
  tls_policy    = "Require"

  lambda_action {
    function_arn    = module.receipt_gate.function_arn
    invocation_type = "RequestResponse"
    position        = 1
  }

  s3_action {
    bucket_name       = aws_s3_bucket.raw_mail.bucket
    object_key_prefix = local.raw_mail_prefix
    position          = 2
  }

  lambda_action {
    function_arn    = module.ingest.function_arn
    invocation_type = "Event"
    position        = 3
  }

  depends_on = [
    aws_lambda_permission.ses_receipt_gate,
    aws_lambda_permission.ses_ingest,
    aws_s3_bucket_policy.raw_mail_ses_write,
  ]
}
