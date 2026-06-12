resource "aws_ses_domain_identity" "mail" {
  domain = local.mail_domain
}

resource "aws_route53_record" "ses_domain_verification" {
  zone_id         = module.ctx.route53_zone_id
  name            = "_amazonses.${local.mail_domain}"
  type            = "TXT"
  ttl             = 600
  records         = [aws_ses_domain_identity.mail.verification_token]
  allow_overwrite = true
}

resource "aws_ses_domain_identity_verification" "mail" {
  domain = aws_ses_domain_identity.mail.id

  depends_on = [aws_route53_record.ses_domain_verification]
}

resource "aws_ses_domain_dkim" "mail" {
  domain = aws_ses_domain_identity.mail.domain
}

resource "aws_route53_record" "mail_mx" {
  zone_id         = module.ctx.route53_zone_id
  name            = local.mail_domain
  type            = "MX"
  ttl             = 300
  records         = [local.ses_inbound_mx_record]
  allow_overwrite = true
}

resource "aws_route53_record" "ses_dkim" {
  for_each = {
    first  = aws_ses_domain_dkim.mail.dkim_tokens[0]
    second = aws_ses_domain_dkim.mail.dkim_tokens[1]
    third  = aws_ses_domain_dkim.mail.dkim_tokens[2]
  }

  zone_id         = module.ctx.route53_zone_id
  name            = "${each.value}._domainkey.${local.mail_domain}"
  type            = "CNAME"
  ttl             = 600
  records         = ["${each.value}.dkim.amazonses.com"]
  allow_overwrite = true
}
