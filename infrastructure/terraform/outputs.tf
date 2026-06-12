output "frontend_url" {
  value = "https://${local.frontend_hostname}"
}

output "api_url" {
  value = "https://${local.api_hostname}"
}

output "product_name" {
  value = local.product_name
}

output "cognito_domain" {
  value = module.ctx.cognito_domain
}

output "api_function_name" {
  value = module.api.function_names["api"]
}

output "ingest_function_name" {
  value = module.ingest.function_name
}

output "receipt_gate_function_name" {
  value = module.receipt_gate.function_name
}

output "send_worker_function_name" {
  value = module.send_worker.function_name
}

output "feedback_handler_function_name" {
  value = module.feedback_handler.function_name
}

output "lambda_reserved_concurrency" {
  value = local.lambda_reserved_concurrency
}

output "raw_mail_bucket_name" {
  value = aws_s3_bucket.raw_mail.bucket
}

output "raw_mail_bucket_arn" {
  value = aws_s3_bucket.raw_mail.arn
}

output "ses_identity_arn" {
  value = aws_ses_domain_identity.mail.arn
}

output "receipt_rule_set_name" {
  value = aws_ses_receipt_rule_set.mail.rule_set_name
}

output "mail_mx_record" {
  value = {
    name    = aws_route53_record.mail_mx.name
    records = aws_route53_record.mail_mx.records
  }
}

output "route53_zone_id" {
  value = module.ctx.route53_zone_id
}

output "bounce_feedback_topic_arn" {
  value = aws_sns_topic.ses_bounces.arn
}

output "complaint_feedback_topic_arn" {
  value = aws_sns_topic.ses_complaints.arn
}

output "alarm_topic_arn" {
  value = aws_sns_topic.mail_alarms.arn
}

output "lambda_error_alarm_names" {
  value = {
    for key, alarm in aws_cloudwatch_metric_alarm.lambda_errors :
    key => alarm.alarm_name
  }
}

output "lambda_throttle_alarm_names" {
  value = {
    for key, alarm in aws_cloudwatch_metric_alarm.lambda_throttles :
    key => alarm.alarm_name
  }
}

output "mail_app_metric_alarm_names" {
  value = {
    for key, alarm in aws_cloudwatch_metric_alarm.app_metric :
    key => alarm.alarm_name
  }
}
