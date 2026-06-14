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

  statement {
    sid    = "ManageAppAuthorizationRecords"
    effect = "Allow"

    actions = [
      "dynamodb:DeleteItem",
      "dynamodb:GetItem",
      "dynamodb:PutItem",
      "dynamodb:Scan",
    ]
    resources = [
      "arn:aws:dynamodb:*:${data.aws_caller_identity.current.account_id}:table/${local.app_authorizations_table_name}",
    ]
  }

  statement {
    sid    = "ReconcileAppAuthorizationCognitoUsers"
    effect = "Allow"

    actions = [
      "cognito-idp:AdminCreateUser",
      "cognito-idp:AdminDisableUser",
      "cognito-idp:AdminEnableUser",
      "cognito-idp:AdminGetUser",
      "cognito-idp:AdminSetUserPassword",
    ]
    resources = [module.ctx.cognito_user_pool_arn]
  }
}
