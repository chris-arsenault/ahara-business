resource "aws_sns_topic" "ses_bounces" {
  name = local.ses_bounce_topic_name
}

resource "aws_sns_topic" "ses_complaints" {
  name = local.ses_complaint_topic_name
}

data "aws_iam_policy_document" "ses_bounces_publish" {
  statement {
    sid    = "AllowSesFeedbackPublish"
    effect = "Allow"

    principals {
      type        = "Service"
      identifiers = ["ses.amazonaws.com"]
    }

    actions   = ["sns:Publish"]
    resources = [aws_sns_topic.ses_bounces.arn]

    condition {
      test     = "StringEquals"
      variable = "AWS:SourceAccount"
      values   = [data.aws_caller_identity.current.account_id]
    }
  }
}

data "aws_iam_policy_document" "ses_complaints_publish" {
  statement {
    sid    = "AllowSesFeedbackPublish"
    effect = "Allow"

    principals {
      type        = "Service"
      identifiers = ["ses.amazonaws.com"]
    }

    actions   = ["sns:Publish"]
    resources = [aws_sns_topic.ses_complaints.arn]

    condition {
      test     = "StringEquals"
      variable = "AWS:SourceAccount"
      values   = [data.aws_caller_identity.current.account_id]
    }
  }
}

resource "aws_sns_topic_policy" "ses_bounces" {
  arn    = aws_sns_topic.ses_bounces.arn
  policy = data.aws_iam_policy_document.ses_bounces_publish.json
}

resource "aws_sns_topic_policy" "ses_complaints" {
  arn    = aws_sns_topic.ses_complaints.arn
  policy = data.aws_iam_policy_document.ses_complaints_publish.json
}

resource "aws_lambda_permission" "sns_bounces_feedback_handler" {
  statement_id  = "AllowSnsBounceInvoke"
  action        = "lambda:InvokeFunction"
  function_name = module.feedback_handler.function_name
  principal     = "sns.amazonaws.com"
  source_arn    = aws_sns_topic.ses_bounces.arn
}

resource "aws_lambda_permission" "sns_complaints_feedback_handler" {
  statement_id  = "AllowSnsComplaintInvoke"
  action        = "lambda:InvokeFunction"
  function_name = module.feedback_handler.function_name
  principal     = "sns.amazonaws.com"
  source_arn    = aws_sns_topic.ses_complaints.arn
}

resource "aws_sns_topic_subscription" "ses_bounces_feedback_handler" {
  topic_arn = aws_sns_topic.ses_bounces.arn
  protocol  = "lambda"
  endpoint  = module.feedback_handler.function_arn

  depends_on = [aws_lambda_permission.sns_bounces_feedback_handler]
}

resource "aws_sns_topic_subscription" "ses_complaints_feedback_handler" {
  topic_arn = aws_sns_topic.ses_complaints.arn
  protocol  = "lambda"
  endpoint  = module.feedback_handler.function_arn

  depends_on = [aws_lambda_permission.sns_complaints_feedback_handler]
}

resource "aws_ses_identity_notification_topic" "bounces" {
  identity                 = aws_ses_domain_identity.mail.domain
  notification_type        = "Bounce"
  topic_arn                = aws_sns_topic.ses_bounces.arn
  include_original_headers = false
}

resource "aws_ses_identity_notification_topic" "complaints" {
  identity                 = aws_ses_domain_identity.mail.domain
  notification_type        = "Complaint"
  topic_arn                = aws_sns_topic.ses_complaints.arn
  include_original_headers = false
}
