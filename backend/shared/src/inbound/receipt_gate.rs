use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

const DEFAULT_ACCEPTED_RECIPIENTS: [&str; 2] = ["chris@ahara.io", "contact@ahara.io"];
const DEFAULT_PER_RECIPIENT_HOURLY_LIMIT: usize = 120;
const DEFAULT_TOTAL_HOURLY_LIMIT: usize = 240;
const DEFAULT_WINDOW_SECONDS: i64 = 60 * 60;

#[derive(Debug, Clone)]
pub struct ReceiptGate {
    config: ReceiptGateConfig,
    state: Arc<Mutex<ReceiptGateState>>,
}

impl ReceiptGate {
    pub fn new(config: ReceiptGateConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(ReceiptGateState::default())),
        }
    }

    pub fn from_env() -> AppResult<Self> {
        Ok(Self::new(ReceiptGateConfig::from_env()?))
    }

    pub fn evaluate(&self, payload: Value) -> AppResult<ReceiptGateDecision> {
        self.evaluate_at(payload, current_epoch_seconds())
    }

    pub fn evaluate_at(
        &self,
        payload: Value,
        now_epoch_seconds: i64,
    ) -> AppResult<ReceiptGateDecision> {
        let event = parse_gate_event(payload)?;
        let mut candidate_recipients = Vec::new();
        for record in &event.records {
            for recipient in &record.recipients {
                let Some(canonical) = self.config.canonical_recipient(recipient) else {
                    return Ok(ReceiptGateDecision::blocked(
                        event.records.len(),
                        ReceiptGateBlockReason::UnknownRecipient,
                    ));
                };
                candidate_recipients.push(canonical);
            }
        }

        let mut candidate_counts = BTreeMap::<String, usize>::new();
        for recipient in &candidate_recipients {
            *candidate_counts.entry(recipient.clone()).or_default() += 1;
        }

        let mut state = self.state.lock().unwrap();
        state.prune(now_epoch_seconds, self.config.window_seconds);
        let current_counts = state.recipient_counts();

        for (recipient, candidate_count) in &candidate_counts {
            let current_count = current_counts.get(recipient).copied().unwrap_or_default();
            if current_count.saturating_add(*candidate_count) > self.config.per_recipient_limit {
                return Ok(ReceiptGateDecision::blocked(
                    event.records.len(),
                    ReceiptGateBlockReason::PerRecipientLimit,
                ));
            }
        }

        if state
            .accepted_recipient_events
            .len()
            .saturating_add(candidate_recipients.len())
            > self.config.total_limit
        {
            return Ok(ReceiptGateDecision::blocked(
                event.records.len(),
                ReceiptGateBlockReason::TotalLimit,
            ));
        }

        for recipient in candidate_recipients {
            state
                .accepted_recipient_events
                .push_back(RecordedRecipient {
                    recipient,
                    epoch_seconds: now_epoch_seconds,
                });
        }

        Ok(ReceiptGateDecision::allowed(event.records.len()))
    }
}

impl Default for ReceiptGate {
    fn default() -> Self {
        Self::new(ReceiptGateConfig::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptGateConfig {
    accepted_recipients: BTreeSet<String>,
    per_recipient_limit: usize,
    total_limit: usize,
    window_seconds: i64,
}

impl ReceiptGateConfig {
    pub fn new(
        accepted_recipients: Vec<String>,
        per_recipient_limit: usize,
        total_limit: usize,
        window_seconds: i64,
    ) -> AppResult<Self> {
        if per_recipient_limit == 0 {
            return Err(AppError::Validation(
                "receipt gate per-recipient limit must be positive".to_string(),
            ));
        }
        if total_limit == 0 {
            return Err(AppError::Validation(
                "receipt gate total limit must be positive".to_string(),
            ));
        }
        if window_seconds <= 0 {
            return Err(AppError::Validation(
                "receipt gate window must be positive".to_string(),
            ));
        }

        let accepted_recipients = accepted_recipients
            .into_iter()
            .map(|recipient| normalize_address(&recipient))
            .filter(|recipient| !recipient.is_empty())
            .collect::<BTreeSet<_>>();
        if accepted_recipients.is_empty() {
            return Err(AppError::Validation(
                "receipt gate accepted recipients must not be empty".to_string(),
            ));
        }

        Ok(Self {
            accepted_recipients,
            per_recipient_limit,
            total_limit,
            window_seconds,
        })
    }

    pub fn from_env() -> AppResult<Self> {
        let accepted_recipients = env::var("ACCEPTED_MAIL_RECIPIENTS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(|recipient| recipient.trim().to_string())
                    .filter(|recipient| !recipient.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(default_accepted_recipients);
        let per_recipient_limit = parse_usize_env(
            "RECEIPT_GATE_PER_RECIPIENT_HOURLY_LIMIT",
            DEFAULT_PER_RECIPIENT_HOURLY_LIMIT,
        )?;
        let total_limit = parse_usize_env(
            "RECEIPT_GATE_TOTAL_HOURLY_LIMIT",
            DEFAULT_TOTAL_HOURLY_LIMIT,
        )?;
        let window_seconds = parse_i64_env("RECEIPT_GATE_WINDOW_SECONDS", DEFAULT_WINDOW_SECONDS)?;

        Self::new(
            accepted_recipients,
            per_recipient_limit,
            total_limit,
            window_seconds,
        )
    }

    fn canonical_recipient(&self, recipient: &str) -> Option<String> {
        let normalized = normalize_address(recipient);
        if self.accepted_recipients.contains(&normalized) {
            return Some(normalized);
        }

        let (local_part, domain) = normalized.split_once('@')?;
        let (base_local_part, _) = local_part.split_once('+')?;
        let canonical = format!("{base_local_part}@{domain}");
        self.accepted_recipients
            .contains(&canonical)
            .then_some(canonical)
    }
}

impl Default for ReceiptGateConfig {
    fn default() -> Self {
        Self::new(
            default_accepted_recipients(),
            DEFAULT_PER_RECIPIENT_HOURLY_LIMIT,
            DEFAULT_TOTAL_HOURLY_LIMIT,
            DEFAULT_WINDOW_SECONDS,
        )
        .expect("default receipt gate config is valid")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptGateDecision {
    pub disposition: ReceiptGateDisposition,
    pub summary: ReceiptGateSummary,
}

impl ReceiptGateDecision {
    fn allowed(processed: usize) -> Self {
        Self {
            disposition: ReceiptGateDisposition::Continue,
            summary: ReceiptGateSummary {
                processed,
                allowed: processed,
                blocked: 0,
                block_reason: None,
            },
        }
    }

    fn blocked(processed: usize, reason: ReceiptGateBlockReason) -> Self {
        Self {
            disposition: ReceiptGateDisposition::StopRuleSet,
            summary: ReceiptGateSummary {
                processed,
                allowed: 0,
                blocked: processed,
                block_reason: Some(reason),
            },
        }
    }

    pub fn ses_response(&self) -> Value {
        serde_json::json!({
            "disposition": self.disposition,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReceiptGateDisposition {
    Continue,
    StopRuleSet,
}

impl ReceiptGateDisposition {
    pub fn as_ses_value(self) -> &'static str {
        match self {
            Self::Continue => "CONTINUE",
            Self::StopRuleSet => "STOP_RULE_SET",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptGateSummary {
    pub processed: usize,
    pub allowed: usize,
    pub blocked: usize,
    pub block_reason: Option<ReceiptGateBlockReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptGateBlockReason {
    UnknownRecipient,
    PerRecipientLimit,
    TotalLimit,
}

#[derive(Debug, Default)]
struct ReceiptGateState {
    accepted_recipient_events: VecDeque<RecordedRecipient>,
}

impl ReceiptGateState {
    fn prune(&mut self, now_epoch_seconds: i64, window_seconds: i64) {
        let earliest = now_epoch_seconds.saturating_sub(window_seconds);
        while self
            .accepted_recipient_events
            .front()
            .is_some_and(|record| record.epoch_seconds < earliest)
        {
            self.accepted_recipient_events.pop_front();
        }
    }

    fn recipient_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for record in &self.accepted_recipient_events {
            *counts.entry(record.recipient.clone()).or_default() += 1;
        }
        counts
    }
}

#[derive(Debug)]
struct RecordedRecipient {
    recipient: String,
    epoch_seconds: i64,
}

#[derive(Debug, Deserialize)]
struct GateEvent {
    #[serde(rename = "Records")]
    records: Vec<GateRecord>,
}

#[derive(Debug, Deserialize)]
struct GateRecord {
    #[serde(rename = "eventSource")]
    event_source: String,
    ses: GateSes,
}

#[derive(Debug, Deserialize)]
struct GateSes {
    mail: GateMail,
    receipt: GateReceipt,
}

#[derive(Debug, Deserialize)]
struct GateMail {
    #[serde(rename = "messageId")]
    message_id: String,
    destination: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GateReceipt {
    recipients: Option<Vec<String>>,
}

#[derive(Debug)]
struct GateReceiptRecord {
    recipients: Vec<String>,
}

#[derive(Debug)]
struct ParsedGateEvent {
    records: Vec<GateReceiptRecord>,
}

fn parse_gate_event(payload: Value) -> AppResult<ParsedGateEvent> {
    let event: GateEvent = serde_json::from_value(payload)
        .map_err(|err| AppError::Validation(format!("invalid SES receipt event: {err}")))?;
    if event.records.is_empty() {
        return Err(AppError::Validation(
            "SES receipt event must contain at least one record".to_string(),
        ));
    }

    let records = event
        .records
        .into_iter()
        .enumerate()
        .map(|(index, record)| {
            if record.event_source != "aws:ses" {
                return Err(AppError::Validation(format!(
                    "record {index} is not an SES event"
                )));
            }
            if record.ses.mail.message_id.trim().is_empty() {
                return Err(AppError::Validation(format!(
                    "record {index} missing SES message id"
                )));
            }
            let recipients = record
                .ses
                .receipt
                .recipients
                .or(record.ses.mail.destination)
                .unwrap_or_default()
                .into_iter()
                .map(|recipient| recipient.trim().to_string())
                .filter(|recipient| !recipient.is_empty())
                .collect::<Vec<_>>();
            if recipients.is_empty() {
                return Err(AppError::Validation(format!(
                    "record {index} must include at least one recipient"
                )));
            }

            Ok(GateReceiptRecord { recipients })
        })
        .collect::<AppResult<Vec<_>>>()?;

    Ok(ParsedGateEvent { records })
}

fn parse_usize_env(name: &'static str, default: usize) -> AppResult<usize> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|err| AppError::Validation(format!("{name} must be a usize: {err}")))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn parse_i64_env(name: &'static str, default: i64) -> AppResult<i64> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|err| AppError::Validation(format!("{name} must be an i64: {err}")))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn default_accepted_recipients() -> Vec<String> {
    DEFAULT_ACCEPTED_RECIPIENTS
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

fn normalize_address(address: &str) -> String {
    address.trim().to_ascii_lowercase()
}

fn current_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{ReceiptGate, ReceiptGateBlockReason, ReceiptGateConfig, ReceiptGateDisposition};

    fn event(recipients: Vec<&str>) -> Value {
        json!({
            "Records": [{
                "eventSource": "aws:ses",
                "ses": {
                    "mail": {
                        "messageId": "ses-message-1",
                        "timestamp": "2026-06-10T18:00:00.000Z",
                        "source": "sender@example.test",
                        "destination": recipients,
                        "commonHeaders": {
                            "from": ["Sender <sender@example.test>"],
                            "subject": "Sensitive subject"
                        }
                    },
                    "receipt": {
                        "recipients": recipients,
                        "spamVerdict": { "status": "PASS" },
                        "virusVerdict": { "status": "PASS" },
                        "action": {
                            "type": "Lambda",
                            "functionArn": "arn:aws:lambda:us-east-1:123456789012:function:gate"
                        }
                    }
                }
            }]
        })
    }

    fn gate_with_limits(per_recipient_limit: usize, total_limit: usize) -> ReceiptGate {
        ReceiptGate::new(
            ReceiptGateConfig::new(
                vec!["chris@ahara.io".to_string(), "contact@ahara.io".to_string()],
                per_recipient_limit,
                total_limit,
                3600,
            )
            .unwrap(),
        )
    }

    #[test]
    fn receipt_gate_allows_confirmed_recipient() {
        let decision = ReceiptGate::default()
            .evaluate_at(event(vec!["contact@ahara.io"]), 1000)
            .unwrap();

        assert_eq!(decision.disposition, ReceiptGateDisposition::Continue);
        assert_eq!(decision.summary.processed, 1);
        assert_eq!(decision.summary.allowed, 1);
        assert_eq!(decision.ses_response()["disposition"], "CONTINUE");
    }

    #[test]
    fn receipt_gate_allows_plus_address_variant() {
        let decision = ReceiptGate::default()
            .evaluate_at(event(vec!["Contact+demo@Ahara.io"]), 1000)
            .unwrap();

        assert_eq!(decision.disposition, ReceiptGateDisposition::Continue);
        assert_eq!(decision.summary.blocked, 0);
    }

    #[test]
    fn receipt_gate_blocks_unknown_recipient() {
        let decision = ReceiptGate::default()
            .evaluate_at(event(vec!["unknown@ahara.io"]), 1000)
            .unwrap();

        assert_eq!(decision.disposition, ReceiptGateDisposition::StopRuleSet);
        assert_eq!(decision.summary.blocked, 1);
        assert_eq!(
            decision.summary.block_reason,
            Some(ReceiptGateBlockReason::UnknownRecipient)
        );
        assert_eq!(decision.ses_response()["disposition"], "STOP_RULE_SET");
    }

    #[test]
    fn receipt_gate_blocks_per_recipient_rate_limit() {
        let gate = gate_with_limits(1, 10);

        assert_eq!(
            gate.evaluate_at(event(vec!["contact@ahara.io"]), 1000)
                .unwrap()
                .disposition,
            ReceiptGateDisposition::Continue
        );
        let blocked = gate
            .evaluate_at(event(vec!["contact@ahara.io"]), 1001)
            .unwrap();

        assert_eq!(blocked.disposition, ReceiptGateDisposition::StopRuleSet);
        assert_eq!(
            blocked.summary.block_reason,
            Some(ReceiptGateBlockReason::PerRecipientLimit)
        );
    }

    #[test]
    fn receipt_gate_blocks_total_rate_limit() {
        let gate = gate_with_limits(10, 1);

        assert_eq!(
            gate.evaluate_at(event(vec!["contact@ahara.io"]), 1000)
                .unwrap()
                .disposition,
            ReceiptGateDisposition::Continue
        );
        let blocked = gate
            .evaluate_at(event(vec!["chris@ahara.io"]), 1001)
            .unwrap();

        assert_eq!(blocked.disposition, ReceiptGateDisposition::StopRuleSet);
        assert_eq!(
            blocked.summary.block_reason,
            Some(ReceiptGateBlockReason::TotalLimit)
        );
    }

    #[test]
    fn receipt_gate_prunes_old_rate_window_entries() {
        let gate = gate_with_limits(1, 1);

        assert_eq!(
            gate.evaluate_at(event(vec!["contact@ahara.io"]), 1000)
                .unwrap()
                .disposition,
            ReceiptGateDisposition::Continue
        );
        assert_eq!(
            gate.evaluate_at(event(vec!["contact@ahara.io"]), 5000)
                .unwrap()
                .disposition,
            ReceiptGateDisposition::Continue
        );
    }

    #[test]
    fn receipt_gate_response_omits_mail_content_and_addresses() {
        let decision = ReceiptGate::default()
            .evaluate_at(event(vec!["contact@ahara.io"]), 1000)
            .unwrap();
        let body = serde_json::to_string(&decision.ses_response()).unwrap();
        let summary = serde_json::to_string(&decision.summary).unwrap();

        for serialized in [body, summary] {
            assert!(!serialized.contains("sender@example.test"));
            assert!(!serialized.contains("contact@ahara.io"));
            assert!(!serialized.contains("Sensitive subject"));
            assert!(!serialized.contains("From:"));
            assert!(!serialized.contains("raw/"));
        }
    }
}
