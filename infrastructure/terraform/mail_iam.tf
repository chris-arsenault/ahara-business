data "aws_iam_policy_document" "lambda_mail" {
  statement {
    sid    = "ListRawMailPrefix"
    effect = "Allow"

    actions   = ["s3:ListBucket"]
    resources = [aws_s3_bucket.raw_mail.arn]

    condition {
      test     = "StringLike"
      variable = "s3:prefix"
      values   = ["${local.raw_mail_prefix}*"]
    }
  }

  statement {
    sid    = "ReadWriteRawMailObjects"
    effect = "Allow"

    actions = [
      "s3:GetObject",
      "s3:GetObjectVersion",
      "s3:PutObject",
    ]
    resources = ["${aws_s3_bucket.raw_mail.arn}/${local.raw_mail_prefix}*"]
  }

  statement {
    sid    = "SendFromMailIdentity"
    effect = "Allow"

    actions = [
      "ses:SendEmail",
      "ses:SendRawEmail",
    ]
    resources = [aws_ses_domain_identity.mail.arn]
  }
}
