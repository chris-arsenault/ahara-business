resource "aws_sns_topic" "mail_alarms" {
  name = local.mail_alarm_topic_name
}

resource "aws_cloudwatch_metric_alarm" "ses_bounce_rate" {
  alarm_name          = "${local.prefix}-ses-bounce-rate"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 1
  metric_name         = "Reputation.BounceRate"
  namespace           = "AWS/SES"
  period              = 300
  statistic           = "Average"
  threshold           = 0.05
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.mail_alarms.arn]
}

resource "aws_cloudwatch_metric_alarm" "ses_complaint_rate" {
  alarm_name          = "${local.prefix}-ses-complaint-rate"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 1
  metric_name         = "Reputation.ComplaintRate"
  namespace           = "AWS/SES"
  period              = 300
  statistic           = "Average"
  threshold           = 0.001
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.mail_alarms.arn]
}

resource "aws_cloudwatch_metric_alarm" "lambda_errors" {
  for_each = local.mail_lambda_alarm_functions

  alarm_name          = "${local.prefix}-${replace(each.key, "_", "-")}-errors"
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 1
  metric_name         = "Errors"
  namespace           = "AWS/Lambda"
  period              = local.mail_alarm_period_seconds
  statistic           = "Sum"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.mail_alarms.arn]

  dimensions = {
    FunctionName = each.value
  }
}

resource "aws_cloudwatch_metric_alarm" "lambda_throttles" {
  for_each = local.mail_lambda_alarm_functions

  alarm_name          = "${local.prefix}-${replace(each.key, "_", "-")}-throttles"
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 1
  metric_name         = "Throttles"
  namespace           = "AWS/Lambda"
  period              = local.mail_alarm_period_seconds
  statistic           = "Sum"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.mail_alarms.arn]

  dimensions = {
    FunctionName = each.value
  }
}

resource "aws_cloudwatch_metric_alarm" "app_metric" {
  for_each = local.mail_app_metric_alarms

  alarm_name          = "${local.prefix}-${replace(each.key, "_", "-")}"
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 1
  metric_name         = each.value.metric_name
  namespace           = local.mail_metric_namespace
  period              = local.mail_alarm_period_seconds
  statistic           = "Sum"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.mail_alarms.arn]

  dimensions = {
    Service    = local.prefix
    Handler    = each.value.handler
    MailDomain = local.mail_domain
  }
}

resource "aws_cloudwatch_metric_alarm" "ingest_invocations" {
  alarm_name          = "${local.prefix}-ingest-invocations"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 1
  metric_name         = "Invocations"
  namespace           = "AWS/Lambda"
  period              = 300
  statistic           = "Sum"
  threshold           = 1000
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.mail_alarms.arn]

  dimensions = {
    FunctionName = module.ingest.function_name
  }
}

resource "aws_cloudwatch_metric_alarm" "send_worker_invocations" {
  alarm_name          = "${local.prefix}-send-worker-invocations"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 1
  metric_name         = "Invocations"
  namespace           = "AWS/Lambda"
  period              = 300
  statistic           = "Sum"
  threshold           = 1000
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.mail_alarms.arn]

  dimensions = {
    FunctionName = module.send_worker.function_name
  }
}

resource "aws_cloudwatch_metric_alarm" "raw_mail_bucket_size" {
  alarm_name          = "${local.prefix}-raw-mail-bucket-size"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 1
  metric_name         = "BucketSizeBytes"
  namespace           = "AWS/S3"
  period              = 86400
  statistic           = "Average"
  threshold           = 1073741824
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.mail_alarms.arn]

  dimensions = {
    BucketName  = aws_s3_bucket.raw_mail.bucket
    StorageType = "StandardStorage"
  }
}
