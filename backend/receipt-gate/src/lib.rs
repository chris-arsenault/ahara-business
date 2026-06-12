use serde_json::{Value, json};
use shared::error::AppResult;
use shared::inbound::receipt_gate::{ReceiptGate, ReceiptGateDecision};
#[cfg(test)]
use shared::observability::mail_metric_payload;
use shared::observability::{CountMetric, emit_mail_metric};

pub async fn handle_event(
    payload: Value,
    request_id: &str,
    mail_domain: &str,
    gate: &ReceiptGate,
) -> AppResult<Value> {
    let decision = gate.evaluate(payload)?;
    let metrics = receipt_gate_operational_metrics(&decision)?;
    emit_mail_metric("receipt-gate", mail_domain, &metrics)?;
    log_decision(request_id, &decision);

    Ok(json!({
        "disposition": decision.disposition,
    }))
}

fn log_decision(request_id: &str, decision: &ReceiptGateDecision) {
    tracing::info!(
        request_id = %request_id,
        service = shared::service_name(),
        handler = "receipt-gate",
        disposition = decision.disposition.as_ses_value(),
        processed = decision.summary.processed,
        allowed = decision.summary.allowed,
        blocked = decision.summary.blocked,
        block_reason = ?decision.summary.block_reason,
        "receipt gate completed"
    );
}

fn receipt_gate_operational_metrics(decision: &ReceiptGateDecision) -> AppResult<Vec<CountMetric>> {
    Ok(vec![
        CountMetric::new("InboundGateProcessed", decision.summary.processed as u64)?,
        CountMetric::new("InboundGateAllowed", decision.summary.allowed as u64)?,
        CountMetric::new("InboundGateBlocked", decision.summary.blocked as u64)?,
    ])
}

#[cfg(test)]
fn receipt_gate_operational_metric_payload(
    mail_domain: &str,
    decision: &ReceiptGateDecision,
) -> AppResult<Value> {
    mail_metric_payload(
        "receipt-gate",
        mail_domain,
        &receipt_gate_operational_metrics(decision)?,
    )
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use shared::inbound::receipt_gate::{ReceiptGate, ReceiptGateConfig, ReceiptGateDisposition};

    use super::{handle_event, receipt_gate_operational_metric_payload};

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

    #[tokio::test]
    async fn receipt_gate_handler_returns_continue_for_accepted_recipient() {
        let response = handle_event(
            event(vec!["contact+demo@ahara.io"]),
            "request-1",
            "ahara.io",
            &ReceiptGate::default(),
        )
        .await
        .unwrap();

        assert_eq!(response["disposition"], "CONTINUE");
    }

    #[tokio::test]
    async fn receipt_gate_handler_returns_stop_rule_set_for_unknown_recipient() {
        let response = handle_event(
            event(vec!["unknown@ahara.io"]),
            "request-1",
            "ahara.io",
            &ReceiptGate::default(),
        )
        .await
        .unwrap();

        assert_eq!(response["disposition"], "STOP_RULE_SET");
    }

    #[tokio::test]
    async fn receipt_gate_handler_returns_stop_rule_set_for_rate_block() {
        let gate = gate_with_limits(1, 10);
        assert_eq!(
            handle_event(
                event(vec!["contact@ahara.io"]),
                "request-1",
                "ahara.io",
                &gate
            )
            .await
            .unwrap()["disposition"],
            "CONTINUE"
        );

        let response = handle_event(
            event(vec!["contact@ahara.io"]),
            "request-2",
            "ahara.io",
            &gate,
        )
        .await
        .unwrap();

        assert_eq!(response["disposition"], "STOP_RULE_SET");
    }

    #[tokio::test]
    async fn receipt_gate_handler_response_omits_mail_content_and_addresses() {
        let response = handle_event(
            event(vec!["contact@ahara.io"]),
            "request-1",
            "ahara.io",
            &ReceiptGate::default(),
        )
        .await
        .unwrap();
        let body = serde_json::to_string(&response).unwrap();

        assert_eq!(
            serde_json::from_value::<ReceiptGateDisposition>(response["disposition"].clone())
                .unwrap(),
            ReceiptGateDisposition::Continue
        );
        assert!(!body.contains("sender@example.test"));
        assert!(!body.contains("contact@ahara.io"));
        assert!(!body.contains("Sensitive subject"));
        assert!(!body.contains("raw/"));
    }

    #[tokio::test]
    async fn operational_metric_payload_is_count_only() {
        let gate = ReceiptGate::default();
        let decision = gate
            .evaluate_at(event(vec!["contact@ahara.io"]), 1000)
            .unwrap();
        let metrics = receipt_gate_operational_metric_payload("ahara.io", &decision).unwrap();
        let serialized = serde_json::to_string(&metrics).unwrap();

        assert!(serialized.contains("InboundGateBlocked"));
        assert!(!serialized.contains("sender@example.test"));
        assert!(!serialized.contains("contact@ahara.io"));
        assert!(!serialized.contains("Sensitive subject"));
        assert!(!serialized.contains("raw/"));
    }
}
