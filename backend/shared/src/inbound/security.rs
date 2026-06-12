use crate::inbound::ses_event::InboundReceiptVerdicts;
use crate::inbound::types::{AuthResult, InboundAuthResults, InboundSecurityRecord};
use crate::mail_security::{
    MailSecurityDecision, MailSecurityVerdicts, SesScanVerdict, classify_inbound_security,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundSecurityDecision {
    pub auth: InboundAuthResults,
    pub security: InboundSecurityRecord,
    pub decision: MailSecurityDecision,
}

pub fn classify_receipt_security(verdicts: &InboundReceiptVerdicts) -> InboundSecurityDecision {
    let spam = map_ses_scan(verdicts.spam.as_deref());
    let virus = map_ses_scan(verdicts.virus.as_deref());
    let decision = classify_inbound_security(MailSecurityVerdicts { spam, virus });
    let auth = InboundAuthResults {
        spf: map_ses_auth(verdicts.spf.as_deref()),
        dkim: map_ses_auth(verdicts.dkim.as_deref()),
        dmarc: map_ses_auth(verdicts.dmarc.as_deref()),
        auth_verdict: None,
    };
    let auth = InboundAuthResults {
        auth_verdict: overall_auth_verdict(&auth),
        ..auth
    };

    InboundSecurityDecision {
        auth,
        security: InboundSecurityRecord {
            disposition: decision.disposition,
            reason: decision.reason,
            spam_result: spam.map(|verdict| verdict.as_db_value().to_string()),
            virus_result: virus.map(|verdict| verdict.as_db_value().to_string()),
        },
        decision,
    }
}

pub fn map_ses_auth(value: Option<&str>) -> Option<AuthResult> {
    value.and_then(AuthResult::from_ses_value)
}

pub fn map_ses_scan(value: Option<&str>) -> Option<SesScanVerdict> {
    value.and_then(SesScanVerdict::from_ses_value)
}

fn overall_auth_verdict(auth: &InboundAuthResults) -> Option<AuthResult> {
    let values = [auth.spf, auth.dkim, auth.dmarc]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    if values.iter().any(|value| {
        matches!(
            value,
            AuthResult::Fail | AuthResult::Softfail | AuthResult::Temperror | AuthResult::Permerror
        )
    }) {
        return Some(AuthResult::Fail);
    }
    if values.contains(&AuthResult::Pass) {
        return Some(AuthResult::Pass);
    }
    if values.contains(&AuthResult::Neutral) {
        return Some(AuthResult::Neutral);
    }
    Some(AuthResult::None)
}

#[cfg(test)]
mod tests {
    use crate::inbound::security::{classify_receipt_security, map_ses_auth, map_ses_scan};
    use crate::inbound::ses_event::InboundReceiptVerdicts;
    use crate::inbound::types::AuthResult;
    use crate::mail_security::{SecurityDisposition, SecurityReason, SesScanVerdict};

    fn verdicts(spam: Option<&str>, virus: Option<&str>) -> InboundReceiptVerdicts {
        InboundReceiptVerdicts {
            spf: Some("PASS".to_string()),
            dkim: Some("PASS".to_string()),
            dmarc: Some("PASS".to_string()),
            spam: spam.map(ToString::to_string),
            virus: virus.map(ToString::to_string),
        }
    }

    #[test]
    fn inbound_security_accepts_clean_mail() {
        let decision = classify_receipt_security(&verdicts(Some("PASS"), Some("PASS")));

        assert_eq!(decision.security.disposition, SecurityDisposition::Accepted);
        assert_eq!(decision.security.reason, SecurityReason::Clean);
        assert_eq!(decision.security.spam_result.as_deref(), Some("pass"));
        assert_eq!(decision.security.virus_result.as_deref(), Some("pass"));
        assert_eq!(decision.decision.status_value(), "received");
    }

    #[test]
    fn inbound_security_quarantines_spam() {
        let decision = classify_receipt_security(&verdicts(Some("FAIL"), Some("PASS")));

        assert_eq!(
            decision.security.disposition,
            SecurityDisposition::Quarantined
        );
        assert_eq!(decision.security.reason, SecurityReason::SpamFailed);
        assert_eq!(decision.decision.status_value(), "quarantined");
    }

    #[test]
    fn inbound_security_quarantines_missing_or_unknown_scan_values() {
        for verdicts in [
            verdicts(None, Some("PASS")),
            verdicts(Some("PASS"), None),
            verdicts(Some("unexpected"), Some("PASS")),
        ] {
            let decision = classify_receipt_security(&verdicts);

            assert_eq!(
                decision.security.disposition,
                SecurityDisposition::Quarantined
            );
        }
    }

    #[test]
    fn inbound_security_rejects_virus_failures() {
        let decision = classify_receipt_security(&verdicts(Some("PASS"), Some("FAIL")));

        assert_eq!(decision.security.disposition, SecurityDisposition::Rejected);
        assert_eq!(decision.security.reason, SecurityReason::VirusFailed);
        assert_eq!(decision.decision.status_value(), "rejected");
    }

    #[test]
    fn inbound_security_normalizes_auth_and_scan_values_to_database_values() {
        assert_eq!(map_ses_auth(Some("SOFTFAIL")), Some(AuthResult::Softfail));
        assert_eq!(map_ses_auth(Some("unexpected")), None);
        assert_eq!(
            map_ses_scan(Some("processingFailed")),
            Some(SesScanVerdict::ProcessingFailed)
        );
        assert_eq!(AuthResult::Temperror.as_db_value(), "temperror");
        assert_eq!(
            SesScanVerdict::ProcessingFailed.as_db_value(),
            "processing_failed"
        );
    }

    #[test]
    fn inbound_security_computes_overall_auth_verdict() {
        let mut input = verdicts(Some("PASS"), Some("PASS"));
        input.spf = Some("PASS".to_string());
        input.dkim = Some("PASS".to_string());
        input.dmarc = Some("PASS".to_string());
        assert_eq!(
            classify_receipt_security(&input).auth.auth_verdict,
            Some(AuthResult::Pass)
        );

        input.dkim = Some("FAIL".to_string());
        assert_eq!(
            classify_receipt_security(&input).auth.auth_verdict,
            Some(AuthResult::Fail)
        );
    }
}
