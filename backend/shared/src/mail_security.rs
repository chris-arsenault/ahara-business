use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SesScanVerdict {
    Pass,
    Fail,
    Gray,
    ProcessingFailed,
}

impl SesScanVerdict {
    pub fn from_ses_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pass" => Some(Self::Pass),
            "fail" => Some(Self::Fail),
            "gray" => Some(Self::Gray),
            "processing_failed" | "processingfailed" => Some(Self::ProcessingFailed),
            _ => None,
        }
    }

    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Gray => "gray",
            Self::ProcessingFailed => "processing_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailSecurityVerdicts {
    pub spam: Option<SesScanVerdict>,
    pub virus: Option<SesScanVerdict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityDisposition {
    Accepted,
    Quarantined,
    Rejected,
}

impl SecurityDisposition {
    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Quarantined => "quarantined",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityReason {
    Clean,
    MissingSpamVerdict,
    MissingVirusVerdict,
    SpamFailed,
    SpamIndeterminate,
    VirusFailed,
    VirusIndeterminate,
}

impl SecurityReason {
    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::MissingSpamVerdict => "missing_spam_verdict",
            Self::MissingVirusVerdict => "missing_virus_verdict",
            Self::SpamFailed => "spam_failed",
            Self::SpamIndeterminate => "spam_indeterminate",
            Self::VirusFailed => "virus_failed",
            Self::VirusIndeterminate => "virus_indeterminate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailSecurityDecision {
    pub disposition: SecurityDisposition,
    pub reason: SecurityReason,
}

impl MailSecurityDecision {
    pub fn status_value(self) -> &'static str {
        match self.disposition {
            SecurityDisposition::Accepted => "received",
            SecurityDisposition::Quarantined => "quarantined",
            SecurityDisposition::Rejected => "rejected",
        }
    }

    pub fn allows_normal_mailbox(self) -> bool {
        self.disposition == SecurityDisposition::Accepted
    }

    pub fn allows_resend_forward(self) -> bool {
        self.disposition == SecurityDisposition::Accepted
    }

    pub fn allows_original_download(self) -> bool {
        self.disposition == SecurityDisposition::Accepted
    }
}

pub fn classify_inbound_security(verdicts: MailSecurityVerdicts) -> MailSecurityDecision {
    match verdicts.virus {
        Some(SesScanVerdict::Fail) => {
            return MailSecurityDecision {
                disposition: SecurityDisposition::Rejected,
                reason: SecurityReason::VirusFailed,
            };
        }
        Some(SesScanVerdict::Gray | SesScanVerdict::ProcessingFailed) => {
            return MailSecurityDecision {
                disposition: SecurityDisposition::Quarantined,
                reason: SecurityReason::VirusIndeterminate,
            };
        }
        None => {
            return MailSecurityDecision {
                disposition: SecurityDisposition::Quarantined,
                reason: SecurityReason::MissingVirusVerdict,
            };
        }
        Some(SesScanVerdict::Pass) => {}
    }

    match verdicts.spam {
        Some(SesScanVerdict::Pass) => MailSecurityDecision {
            disposition: SecurityDisposition::Accepted,
            reason: SecurityReason::Clean,
        },
        Some(SesScanVerdict::Fail) => MailSecurityDecision {
            disposition: SecurityDisposition::Quarantined,
            reason: SecurityReason::SpamFailed,
        },
        Some(SesScanVerdict::Gray | SesScanVerdict::ProcessingFailed) => MailSecurityDecision {
            disposition: SecurityDisposition::Quarantined,
            reason: SecurityReason::SpamIndeterminate,
        },
        None => MailSecurityDecision {
            disposition: SecurityDisposition::Quarantined,
            reason: SecurityReason::MissingSpamVerdict,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MailSecurityVerdicts, SecurityDisposition, SecurityReason, SesScanVerdict,
        classify_inbound_security,
    };

    #[test]
    fn accepts_only_clean_spam_and_virus_verdicts() {
        let decision = classify_inbound_security(MailSecurityVerdicts {
            spam: Some(SesScanVerdict::Pass),
            virus: Some(SesScanVerdict::Pass),
        });

        assert_eq!(decision.disposition, SecurityDisposition::Accepted);
        assert_eq!(decision.reason, SecurityReason::Clean);
        assert_eq!(decision.status_value(), "received");
        assert!(decision.allows_normal_mailbox());
        assert!(decision.allows_resend_forward());
        assert!(decision.allows_original_download());
    }

    #[test]
    fn rejects_virus_failures_before_spam_handling() {
        let decision = classify_inbound_security(MailSecurityVerdicts {
            spam: Some(SesScanVerdict::Pass),
            virus: Some(SesScanVerdict::Fail),
        });

        assert_eq!(decision.disposition, SecurityDisposition::Rejected);
        assert_eq!(decision.reason, SecurityReason::VirusFailed);
        assert_eq!(decision.status_value(), "rejected");
        assert!(!decision.allows_normal_mailbox());
        assert!(!decision.allows_resend_forward());
        assert!(!decision.allows_original_download());
    }

    #[test]
    fn quarantines_spam_failures_without_rejecting_the_smtp_transaction() {
        let decision = classify_inbound_security(MailSecurityVerdicts {
            spam: Some(SesScanVerdict::Fail),
            virus: Some(SesScanVerdict::Pass),
        });

        assert_eq!(decision.disposition, SecurityDisposition::Quarantined);
        assert_eq!(decision.reason, SecurityReason::SpamFailed);
        assert_eq!(decision.status_value(), "quarantined");
        assert!(!decision.allows_normal_mailbox());
        assert!(!decision.allows_resend_forward());
        assert!(!decision.allows_original_download());
    }

    #[test]
    fn quarantines_indeterminate_or_missing_scan_verdicts() {
        for verdicts in [
            MailSecurityVerdicts {
                spam: Some(SesScanVerdict::Pass),
                virus: Some(SesScanVerdict::Gray),
            },
            MailSecurityVerdicts {
                spam: Some(SesScanVerdict::Pass),
                virus: Some(SesScanVerdict::ProcessingFailed),
            },
            MailSecurityVerdicts {
                spam: Some(SesScanVerdict::ProcessingFailed),
                virus: Some(SesScanVerdict::Pass),
            },
            MailSecurityVerdicts {
                spam: None,
                virus: Some(SesScanVerdict::Pass),
            },
            MailSecurityVerdicts {
                spam: Some(SesScanVerdict::Pass),
                virus: None,
            },
        ] {
            let decision = classify_inbound_security(verdicts);

            assert_eq!(decision.disposition, SecurityDisposition::Quarantined);
            assert!(!decision.allows_normal_mailbox());
            assert!(!decision.allows_resend_forward());
            assert!(!decision.allows_original_download());
        }
    }

    #[test]
    fn parses_ses_verdict_values_to_database_values() {
        assert_eq!(
            SesScanVerdict::from_ses_value("PASS"),
            Some(SesScanVerdict::Pass)
        );
        assert_eq!(
            SesScanVerdict::from_ses_value("processingFailed"),
            Some(SesScanVerdict::ProcessingFailed)
        );
        assert_eq!(SesScanVerdict::Fail.as_db_value(), "fail");
        assert_eq!(SecurityDisposition::Rejected.as_db_value(), "rejected");
        assert_eq!(SecurityReason::VirusFailed.as_db_value(), "virus_failed");
    }
}
