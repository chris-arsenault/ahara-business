use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppResult;
use crate::forwarding::{ForwardingPlanner, ForwardingPlannerMessage};
use crate::inbound::limits::IngestLimits;
use crate::inbound::mime::{InboundParseError, parse_raw_mime};
use crate::inbound::repository::{
    InboundRepository, PersistInboundMessageRequest, PersistRejectedInboundRequest, rejected_audit,
};
use crate::inbound::routing::{InboundRoutingLookup, RoutingDecision, resolve_inbound_route};
use crate::inbound::security::classify_receipt_security;
use crate::inbound::ses_event::{
    InboundReceipt, RawMailLocation, parse_ses_receipt_event_with_raw_mail_location,
};
use crate::inbound::types::{InboundMailbox, InboundSecurityRecord, ParsedInboundMessage};
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
        let raw_size_bytes = raw_size_to_i64(raw_metadata.size_bytes);
        if raw_metadata.size_bytes > self.limits.max_raw_mail_object_bytes {
            return self
                .persist_rejected_raw_mail_control(
                    receipt,
                    security_decision.auth,
                    security_decision.security,
                    "limit_exceeded_raw_mail_object_bytes",
                    raw_size_bytes,
                    FloodControlRejection::Oversize,
                )
                .await;
        }

        let recent_raw_mail_bytes = self
            .repository
            .recent_raw_mail_bytes(self.limits.recent_raw_mail_window_seconds)
            .await?;
        if recent_raw_mail_bytes.saturating_add(raw_size_bytes)
            > raw_size_to_i64(self.limits.max_recent_raw_mail_bytes)
        {
            return self
                .persist_rejected_raw_mail_control(
                    receipt,
                    security_decision.auth,
                    security_decision.security,
                    "limit_exceeded_recent_raw_mail_bytes",
                    raw_size_bytes,
                    FloodControlRejection::HourlyBytes,
                )
                .await;
        }

        let raw = match self
            .raw_mail_store
            .get_raw_mail(&receipt.raw_mail.key)
            .await
        {
            Ok(raw) => raw,
            Err(err) => {
                return Err(err);
            }
        };

        let parsed = match parse_raw_mime(&raw.bytes, self.limits) {
            Ok(parsed) => parsed,
            Err(error) => {
                let reason = parse_rejection_reason(&error);
                let persisted = self
                    .repository
                    .persist_rejected_inbound(PersistRejectedInboundRequest {
                        auth: security_decision.auth,
                        audit: rejected_audit(
                            receipt.ses_message_id,
                            Some(receipt.raw_mail.key),
                            receipt.recipients,
                            None,
                            rejected_record(security_decision.security),
                            reason,
                            Some(raw.bytes.len() as i64),
                        ),
                    })
                    .await?;
                return Ok(IngestOutcome::Rejected {
                    idempotent: persisted.idempotent,
                    flood_control: None,
                });
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
                let mut security = security_decision.security;
                security.disposition = SecurityDisposition::Rejected;
                let persisted = self
                    .repository
                    .persist_rejected_inbound(PersistRejectedInboundRequest {
                        auth: security_decision.auth,
                        audit: rejected_audit(
                            receipt.ses_message_id,
                            Some(receipt.raw_mail.key),
                            receipt.recipients,
                            Some(parsed.from),
                            security,
                            rejection.reason.as_db_reason(),
                            Some(parsed.size_bytes),
                        ),
                    })
                    .await?;
                return Ok(IngestOutcome::Rejected {
                    idempotent: persisted.idempotent,
                    flood_control: None,
                });
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
        if disposition == SecurityDisposition::Accepted
            && !persisted.idempotent
            && let Some(planner) = &self.forwarding_planner
        {
            planner
                .process_message(ForwardingPlannerMessage {
                    message_id: persisted.id.to_string(),
                    thread_id: None,
                    rfc_message_id: parsed_for_forwarding.rfc_message_id,
                    reference_ids: parsed_for_forwarding.reference_ids,
                    from_address: parsed_for_forwarding.from.address,
                    subject: parsed_for_forwarding.subject,
                    body_text: parsed_for_forwarding.body_text,
                    auth: auth_for_forwarding,
                    security_disposition: disposition,
                })
                .await?;
        }

        Ok(match disposition {
            SecurityDisposition::Accepted => IngestOutcome::Accepted {
                idempotent: persisted.idempotent,
            },
            SecurityDisposition::Quarantined => IngestOutcome::Quarantined {
                idempotent: persisted.idempotent,
            },
            SecurityDisposition::Rejected => IngestOutcome::Rejected {
                idempotent: persisted.idempotent,
                flood_control: None,
            },
        })
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
                audit: rejected_audit(
                    receipt.ses_message_id,
                    Some(receipt.raw_mail.key),
                    receipt.recipients,
                    Some(parsed.from.clone()),
                    security,
                    SecurityReason::VirusFailed.as_db_value(),
                    Some(parsed.size_bytes),
                ),
            })
            .await?;

        Ok(IngestOutcome::Rejected {
            idempotent: persisted.idempotent,
            flood_control: None,
        })
    }

    async fn persist_rejected_raw_mail_control(
        &self,
        receipt: InboundReceipt,
        auth: crate::inbound::types::InboundAuthResults,
        mut security: InboundSecurityRecord,
        rejection_reason: &'static str,
        size_bytes: i64,
        flood_control: FloodControlRejection,
    ) -> AppResult<IngestOutcome> {
        security.disposition = SecurityDisposition::Rejected;
        let persisted = self
            .repository
            .persist_rejected_inbound(PersistRejectedInboundRequest {
                auth,
                audit: rejected_audit(
                    receipt.ses_message_id,
                    Some(receipt.raw_mail.key),
                    receipt.recipients,
                    None,
                    security,
                    rejection_reason,
                    Some(size_bytes),
                ),
            })
            .await?;

        Ok(IngestOutcome::Rejected {
            idempotent: persisted.idempotent,
            flood_control: Some(flood_control),
        })
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

#[allow(dead_code)]
fn _assert_no_display_name_identity(sender: &InboundMailbox) -> &str {
    &sender.address_normalized
}
