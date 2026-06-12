resource "aws_s3_bucket" "raw_mail" {
  bucket = local.raw_mail_bucket
}

resource "aws_s3_bucket_public_access_block" "raw_mail" {
  bucket = aws_s3_bucket.raw_mail.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_server_side_encryption_configuration" "raw_mail" {
  bucket = aws_s3_bucket.raw_mail.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
    bucket_key_enabled = true
  }
}

resource "aws_s3_bucket_versioning" "raw_mail" {
  bucket = aws_s3_bucket.raw_mail.id

  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_lifecycle_configuration" "raw_mail" {
  bucket = aws_s3_bucket.raw_mail.id

  rule {
    id     = "raw-mail-retention"
    status = "Enabled"

    filter {
      prefix = local.raw_mail_prefix
    }

    expiration {
      days = local.raw_mail_current_retention_days
    }

    noncurrent_version_expiration {
      noncurrent_days = local.raw_mail_noncurrent_version_retention_days
    }

    abort_incomplete_multipart_upload {
      days_after_initiation = local.raw_mail_abort_incomplete_multipart_days
    }
  }
}

data "aws_iam_policy_document" "raw_mail_ses_write" {
  statement {
    sid    = "AllowSesReceiptRuleWrite"
    effect = "Allow"

    principals {
      type        = "Service"
      identifiers = ["ses.amazonaws.com"]
    }

    actions   = ["s3:PutObject"]
    resources = ["${aws_s3_bucket.raw_mail.arn}/${local.raw_mail_prefix}*"]

    condition {
      test     = "StringEquals"
      variable = "AWS:SourceAccount"
      values   = [data.aws_caller_identity.current.account_id]
    }
  }
}

resource "aws_s3_bucket_policy" "raw_mail_ses_write" {
  bucket = aws_s3_bucket.raw_mail.id
  policy = data.aws_iam_policy_document.raw_mail_ses_write.json
}
