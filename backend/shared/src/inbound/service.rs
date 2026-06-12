use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppResult;
use crate::forwarding::{ForwardingPlanner, ForwardingPlannerMessage};
use crate::inbound::limits::IngestLimits;
use crate::inbound::mime::{InboundParseError, parse_raw_mime};
use crate::inbound::repository::{
    InboundRepository, PersistInboundMessageRequest, PersistRejectedInboundRequest,
    RejectedAuditRequest, rejected_audit,
};
use crate::inbound::routing::{InboundRoutingLookup, RoutingDecision, resolve_inbound_route};
use crate::inbound::security::classify_receipt_security;
use crate::inbound::ses_event::{
    InboundReceipt, RawMailLocation, parse_ses_receipt_event_with_raw_mail_location,
};
use crate::inbound::types::{
    InboundAuthResults, InboundMailbox, InboundSecurityRecord, ParsedInboundMessage,
    PersistedInboundMessage,
};
use crate::mail_security::{SecurityDisposition, SecurityReason};
use crate::ports::RawMailStore;

#[derive(Clone)]
pub struct InboundIngestService {
    raw_mail_store: Arc<dyn RawMailStore>,
    routing_lookup: Arc<dyn InboundRoutingLookup>,
    repository: Arc<dyn InboundRepository>,
    limits: IngestLimits,
    raw_mail_location: Option<RawMailLocation>,
    forwarding_planner: Option<Arc<ForwardingPlanner>>,
}

impl InboundIngestService {
    pub fn new(
        raw_mail_store: Arc<dyn RawMailStore>,
        routing_lookup: Arc<dyn InboundRoutingLookup>,
        repository: Arc<dyn InboundRepository>,
        limits: IngestLimits,
    ) -> Self {
        Self {
            raw_mail_store,
            routing_lookup,
            repository,
            limits,
            raw_mail_location: None,
            forwarding_planner: None,
        }
    }

    pub fn with_raw_mail_location(
        mut self,
        bucket: impl Into<String>,
        prefix: impl Into<String>,
    ) -> Self {
        self.raw_mail_location = Some(RawMailLocation::new(bucket, prefix));
        self
    }

    pub fn with_forwarding_planner(mut self, planner: Arc<ForwardingPlanner>) -> Self {
        self.forwarding_planner = Some(planner);
        self
    }

    pub async fn process_receipt_event(&self, payload: Value) -> AppResult<IngestSummary> {
        let receipts = parse_ses_receipt_event_with_raw_mail_location(
            payload,
            self.raw_mail_location.as_ref(),
        )?;
        let mut summary = IngestSummary::default();
        for receipt in receipts {
            summary.processed += 1;
            match self.process_receipt(receipt).await {
                Ok(outcome) => summary.record(outcome),
                Err(_) => summary.failed += 1,
            }
        }
        Ok(summary)
    }

    async fn process_receipt(&self, receipt: InboundReceipt) -> AppResult<IngestOutcome> {
        let security_decision = classify_receipt_security(&receipt.verdicts);
        let raw_metadata = self
            .raw_mail_store
            .get_raw_mail_metadata(&receipt.raw_mail.key)
            .await?;
        if let Some(outcome) = self
            .reject_raw_mail_control_if_needed(
                &receipt,
                &security_decision.auth,
                &security_decision.security,
                raw_metadata.size_bytes,
            )
            .await?
        {
            return Ok(outcome);
        }

        let raw = self
            .raw_mail_store
            .get_raw_mail(&receipt.raw_mail.key)
            .await?;

        let parsed = match parse_raw_mime(&raw.bytes, self.limits) {
            Ok(parsed) => parsed,
            Err(error) => {
                return self
                    .persist_parse_rejection(
                        receipt,
                        security_decision.auth,
                        security_decision.security,
                        &error,
                        raw.bytes.len(),
                    )
                    .await;
            }
        };

        if security_decision.security.disposition == SecurityDisposition::Rejected {
            return self
                .persist_rejected(
                    receipt,
                    &parsed,
                    security_decision.auth,
                    security_decision.security,
                )
                .await;
        }

        let route =
            resolve_inbound_route(&receipt.recipients, self.routing_lookup.as_ref()).await?;
        let routing = match route {
            RoutingDecision::Accepted(routing) => routing,
            RoutingDecision::Rejected(rejection) => {
                return self
                    .persist_routing_rejection(
                        receipt,
                        parsed,
                        security_decision.auth,
                        security_decision.security,
                        rejection.reason.as_db_reason(),
                    )
                    .await;
            }
        };

        let disposition = security_decision.security.disposition;
        let parsed_for_forwarding = parsed.clone();
        let auth_for_forwarding = security_decision.auth.clone();
        let persisted = self
            .repository
            .persist_inbound(PersistInboundMessageRequest {
                receipt,
                parsed,
                auth: security_decision.auth,
                security: security_decision.security,
                routing,
            })
            .await?;
        self.plan_forwarding(
            &persisted,
            parsed_for_forwarding,
            auth_for_forwarding,
            disposition,
        )
        .await?;

        Ok(outcome_from_disposition(disposition, persisted.idempotent))
    }

    async fn reject_raw_mail_control_if_needed(
        &self,
        receipt: &InboundReceipt,
        auth: &InboundAuthResults,
        security: &InboundSecurityRecord,
        raw_object_bytes: usize,
    ) -> AppResult<Option<IngestOutcome>> {
        let raw_size_bytes = raw_size_to_i64(raw_object_bytes);
        if raw_object_bytes > self.limits.max_raw_mail_object_bytes {
            return self
                .persist_rejected_raw_mail_control(RawMailControlRejection {
                    receipt: receipt.clone(),
                    auth: auth.clone(),
                    security: security.clone(),
                    rejection_reason: "limit_exceeded_raw_mail_object_bytes",
                    size_bytes: raw_size_bytes,
                    flood_control: FloodControlRejection::Oversize,
                })
                .await
                .map(Some);
        }

        let recent_raw_mail_bytes = self
            .repository
            .recent_raw_mail_bytes(self.limits.recent_raw_mail_window_seconds)
            .await?;
        if recent_raw_mail_bytes.saturating_add(raw_size_bytes)
            > raw_size_to_i64(self.limits.max_recent_raw_mail_bytes)
        {
            return self
                .persist_rejected_raw_mail_control(RawMailControlRejection {
                    receipt: receipt.clone(),
                    auth: auth.clone(),
                    security: security.clone(),
                    rejection_reason: "limit_exceeded_recent_raw_mail_bytes",
                    size_bytes: raw_size_bytes,
                    flood_control: FloodControlRejection::HourlyBytes,
                })
                .await
                .map(Some);
        }

        Ok(None)
    }

    async fn persist_parse_rejection(
        &self,
        receipt: InboundReceipt,
        auth: InboundAuthResults,
        security: InboundSecurityRecord,
        error: &InboundParseError,
        raw_size_bytes: usize,
    ) -> AppResult<IngestOutcome> {
        let persisted = self
            .repository
            .persist_rejected_inbound(PersistRejectedInboundRequest {
                auth,
                audit: rejected_audit(RejectedAuditRequest {
                    ses_message_id: receipt.ses_message_id,
                    s3_raw_key: Some(receipt.raw_mail.key),
                    envelope_recipients: receipt.recipients,
                    from: None,
                    security: rejected_record(security),
                    rejection_reason: parse_rejection_reason(error),
                    size_bytes: Some(raw_size_to_i64(raw_size_bytes)),
                }),
            })
            .await?;

        Ok(rejected_outcome(persisted.idempotent, None))
    }

    async fn persist_routing_rejection(
        &self,
        receipt: InboundReceipt,
        parsed: ParsedInboundMessage,
        auth: InboundAuthResults,
        security: InboundSecurityRecord,
        rejection_reason: impl Into<String>,
    ) -> AppResult<IngestOutcome> {
        let persisted = self
            .repository
            .persist_rejected_inbound(PersistRejectedInboundRequest {
                auth,
                audit: rejected_audit(RejectedAuditRequest {
                    ses_message_id: receipt.ses_message_id,
                    s3_raw_key: Some(receipt.raw_mail.key),
                    envelope_recipients: receipt.recipients,
                    from: Some(parsed.from),
                    security: rejected_record(security),
                    rejection_reason: rejection_reason.into(),
                    size_bytes: Some(parsed.size_bytes),
                }),
            })
            .await?;

        Ok(rejected_outcome(persisted.idempotent, None))
    }

    async fn plan_forwarding(
        &self,
        persisted: &PersistedInboundMessage,
        parsed: ParsedInboundMessage,
        auth: InboundAuthResults,
        disposition: SecurityDisposition,
    ) -> AppResult<()> {
        if disposition != SecurityDisposition::Accepted || persisted.idempotent {
            return Ok(());
        }

        let Some(planner) = &self.forwarding_planner else {
            return Ok(());
        };

        planner
            .process_message(ForwardingPlannerMessage {
                message_id: persisted.id.to_string(),
                thread_id: None,
                rfc_message_id: parsed.rfc_message_id,
                reference_ids: parsed.reference_ids,
                from_address: parsed.from.address,
                subject: parsed.subject,
                body_text: parsed.body_text,
                auth,
                security_disposition: disposition,
            })
            .await
            .map(|_| ())
    }

    async fn persist_rejected(
        &self,
        receipt: InboundReceipt,
        parsed: &ParsedInboundMessage,
        auth: crate::inbound::types::InboundAuthResults,
        security: InboundSecurityRecord,
    ) -> AppResult<IngestOutcome> {
        let persisted = self
            .repository
            .persist_rejected_inbound(PersistRejectedInboundRequest {
                auth,
                audit: rejected_audit(RejectedAuditRequest {
                    ses_message_id: receipt.ses_message_id,
                    s3_raw_key: Some(receipt.raw_mail.key),
                    envelope_recipients: receipt.recipients,
                    from: Some(parsed.from.clone()),
                    security,
                    rejection_reason: SecurityReason::VirusFailed.as_db_value().to_string(),
                    size_bytes: Some(parsed.size_bytes),
                }),
            })
            .await?;

        Ok(rejected_outcome(persisted.idempotent, None))
    }

    async fn persist_rejected_raw_mail_control(
        &self,
        rejection: RawMailControlRejection,
    ) -> AppResult<IngestOutcome> {
        let RawMailControlRejection {
            receipt,
            auth,
            mut security,
            rejection_reason,
            size_bytes,
            flood_control,
        } = rejection;
        security.disposition = SecurityDisposition::Rejected;
        let persisted = self
            .repository
            .persist_rejected_inbound(PersistRejectedInboundRequest {
                auth,
                audit: rejected_audit(RejectedAuditRequest {
                    ses_message_id: receipt.ses_message_id,
                    s3_raw_key: Some(receipt.raw_mail.key),
                    envelope_recipients: receipt.recipients,
                    from: None,
                    security,
                    rejection_reason: rejection_reason.to_string(),
                    size_bytes: Some(size_bytes),
                }),
            })
            .await?;

        Ok(rejected_outcome(persisted.idempotent, Some(flood_control)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IngestOutcome {
    Accepted {
        idempotent: bool,
    },
    Quarantined {
        idempotent: bool,
    },
    Rejected {
        idempotent: bool,
        flood_control: Option<FloodControlRejection>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FloodControlRejection {
    Oversize,
    HourlyBytes,
}

#[derive(Debug, Clone)]
struct RawMailControlRejection {
    receipt: InboundReceipt,
    auth: InboundAuthResults,
    security: InboundSecurityRecord,
    rejection_reason: &'static str,
    size_bytes: i64,
    flood_control: FloodControlRejection,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestSummary {
    pub processed: usize,
    pub accepted: usize,
    pub quarantined: usize,
    pub rejected: usize,
    pub oversize_rejected: usize,
    pub hourly_bytes_rejected: usize,
    pub idempotent: usize,
    pub failed: usize,
}

impl IngestSummary {
    fn record(&mut self, outcome: IngestOutcome) {
        match outcome {
            IngestOutcome::Accepted { idempotent } => {
                self.accepted += 1;
                if idempotent {
                    self.idempotent += 1;
                }
            }
            IngestOutcome::Quarantined { idempotent } => {
                self.quarantined += 1;
                if idempotent {
                    self.idempotent += 1;
                }
            }
            IngestOutcome::Rejected {
                idempotent,
                flood_control,
            } => {
                self.rejected += 1;
                match flood_control {
                    Some(FloodControlRejection::Oversize) => self.oversize_rejected += 1,
                    Some(FloodControlRejection::HourlyBytes) => self.hourly_bytes_rejected += 1,
                    None => {}
                }
                if idempotent {
                    self.idempotent += 1;
                }
            }
        }
    }
}

fn parse_rejection_reason(error: &InboundParseError) -> String {
    match error {
        InboundParseError::LimitExceeded { limit } => format!("limit_exceeded_{limit}"),
        InboundParseError::Mime(_) => "mime_parse_failed".to_string(),
    }
}

fn raw_size_to_i64(size_bytes: usize) -> i64 {
    i64::try_from(size_bytes).unwrap_or(i64::MAX)
}

fn rejected_record(mut security: InboundSecurityRecord) -> InboundSecurityRecord {
    security.disposition = SecurityDisposition::Rejected;
    security
}

fn rejected_outcome(
    idempotent: bool,
    flood_control: Option<FloodControlRejection>,
) -> IngestOutcome {
    IngestOutcome::Rejected {
        idempotent,
        flood_control,
    }
}

fn outcome_from_disposition(disposition: SecurityDisposition, idempotent: bool) -> IngestOutcome {
    match disposition {
        SecurityDisposition::Accepted => IngestOutcome::Accepted { idempotent },
        SecurityDisposition::Quarantined => IngestOutcome::Quarantined { idempotent },
        SecurityDisposition::Rejected => rejected_outcome(idempotent, None),
    }
}

#[allow(dead_code)]
fn _assert_no_display_name_identity(sender: &InboundMailbox) -> &str {
    &sender.address_normalized
}
