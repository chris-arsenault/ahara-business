use std::sync::Arc;

use shared::app_authorizations::{AppAuthorizationUser, InMemoryAppAuthorizationService};
use shared::attachments::{InMemoryAttachmentService, MailboxAttachmentDownload};
use shared::auth::{AuthVerifier, UserContext};
use shared::config::{
    ApiConfig, AppAuthorizationConfig, AppConfig, CognitoConfig, DatabaseConfig, FeedbackConfig,
    MailConfig,
};
use shared::contacts::{Contact, InMemoryContactsService};
use shared::db::database_url;
use shared::domain_config::{AcceptedAddress, DomainConfig, InMemoryDomainConfigService};
use shared::error::{AppError, AppResult};
use shared::finance::{
    CreateFinanceExpenseRequest, CreateFinanceReceivableRequest, ExpenseKind, ExpenseStatus,
    FinanceCategoryTotal, FinanceExpense, FinanceExpenseQuery, FinanceReceivable,
    FinanceReceivableQuery, FinanceService, FinanceSummary, FinanceSummaryQuery,
    FinanceVendorTotal, ReceivableStatus, RecurrenceInterval, UpdateFinanceExpenseRequest,
    UpdateFinanceReceivableRequest,
};
use shared::forwarding::InMemoryForwardingRuleService;
use shared::mailbox::{
    InMemoryMailboxMessage, InMemoryMailboxService, MailboxAttachment, MailboxAuthResult,
    MailboxMessageDetail, MailboxScanResult, MailboxSecurityDisposition,
};
use shared::outbound::{InMemoryOutboundService, InMemoryReplySource};
use shared::ports::{RawMailMetadata, RawMailObject, RawMailStore};
use shared::routing::RoutingPolicy;
use uuid::Uuid;

use crate::ApiState;

struct TestAuthVerifier;

#[async_trait::async_trait]
impl AuthVerifier for TestAuthVerifier {
    async fn context_from_authorization(
        &self,
        auth_header: Option<&str>,
    ) -> AppResult<UserContext> {
        shared::auth::decode_unverified_claims(shared::auth::extract_bearer(auth_header)?)
    }
}

struct TestRawMailStore;

#[async_trait::async_trait]
impl RawMailStore for TestRawMailStore {
    async fn get_raw_mail_metadata(&self, key: &str) -> AppResult<RawMailMetadata> {
        Err(AppError::NotFound(format!("raw mail object {key}")))
    }

    async fn get_raw_mail(&self, key: &str) -> AppResult<RawMailObject> {
        Err(AppError::NotFound(format!("raw mail object {key}")))
    }

    async fn put_raw_mail(&self, _object: RawMailObject) -> AppResult<()> {
        Ok(())
    }

    async fn delete_raw_mail(&self, _key: &str) -> AppResult<()> {
        Ok(())
    }
}

struct TestFinanceService;

#[async_trait::async_trait]
impl FinanceService for TestFinanceService {
    async fn list_expenses(&self, _query: FinanceExpenseQuery) -> AppResult<Vec<FinanceExpense>> {
        Ok(vec![expense("expense-1", "AWS", "cloud", 12_000, 7500)])
    }

    async fn create_expense(
        &self,
        request: CreateFinanceExpenseRequest,
    ) -> AppResult<FinanceExpense> {
        Ok(expense(
            "expense-2",
            request.vendor_name.as_deref().unwrap_or(""),
            &request.category,
            request.amount_cents,
            request.business_use_percent_bps.unwrap_or(10000),
        ))
    }

    async fn update_expense(
        &self,
        expense_id: &str,
        request: UpdateFinanceExpenseRequest,
    ) -> AppResult<FinanceExpense> {
        let mut item = expense(expense_id, "AWS", "cloud", 12_000, 7500);
        item.status = request.status.unwrap_or(item.status);
        Ok(item)
    }

    async fn list_receivables(
        &self,
        _query: FinanceReceivableQuery,
    ) -> AppResult<Vec<FinanceReceivable>> {
        Ok(vec![receivable("receivable-1", "Client session", 25_000)])
    }

    async fn create_receivable(
        &self,
        request: CreateFinanceReceivableRequest,
    ) -> AppResult<FinanceReceivable> {
        Ok(receivable(
            "receivable-2",
            &request.title,
            request.amount_cents,
        ))
    }

    async fn update_receivable(
        &self,
        receivable_id: &str,
        request: UpdateFinanceReceivableRequest,
    ) -> AppResult<FinanceReceivable> {
        let mut item = receivable(receivable_id, "Client session", 25_000);
        item.status = request.status.unwrap_or(item.status);
        Ok(item)
    }

    async fn summary(&self, query: FinanceSummaryQuery) -> AppResult<FinanceSummary> {
        Ok(FinanceSummary {
            tax_year: query.tax_year.unwrap_or(2026),
            gross_expense_cents: 12_000,
            business_expense_cents: 9_000,
            personal_expense_cents: 3_000,
            receivable_owed_cents: 25_000,
            receivable_paid_cents: 0,
            category_totals: vec![FinanceCategoryTotal {
                category: "cloud".to_string(),
                gross_cents: 12_000,
                business_cents: 9_000,
                personal_cents: 3_000,
            }],
            vendor_totals: vec![FinanceVendorTotal {
                vendor_name: "AWS".to_string(),
                gross_cents: 12_000,
                business_cents: 9_000,
                personal_cents: 3_000,
            }],
        })
    }
}

fn expense(
    id: impl Into<String>,
    vendor_name: &str,
    category: &str,
    amount_cents: i64,
    bps: i32,
) -> FinanceExpense {
    let business_amount_cents = amount_cents * i64::from(bps) / 10000;
    FinanceExpense {
        id: id.into(),
        title: "Cloud hosting".to_string(),
        vendor_name: vendor_name.to_string(),
        category: category.to_string(),
        expense_kind: ExpenseKind::Recurring,
        recurrence_interval: RecurrenceInterval::Monthly,
        status: ExpenseStatus::Active,
        amount_cents,
        business_amount_cents,
        personal_amount_cents: amount_cents - business_amount_cents,
        currency: "USD".to_string(),
        incurred_on: "2026-06-01".to_string(),
        service_period_start: Some("2026-06-01".to_string()),
        service_period_end: Some("2026-06-30".to_string()),
        business_use_percent_bps: bps,
        source_message_id: None,
        source_attachment_id: None,
        source_asset_id: None,
        notes: "shared service allocation".to_string(),
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
    }
}

fn receivable(id: impl Into<String>, title: &str, amount_cents: i64) -> FinanceReceivable {
    FinanceReceivable {
        id: id.into(),
        contact_id: Some("contact-1".to_string()),
        title: title.to_string(),
        status: ReceivableStatus::Owed,
        amount_cents,
        currency: "USD".to_string(),
        issued_on: Some("2026-06-01".to_string()),
        due_on: Some("2026-06-15".to_string()),
        paid_on: None,
        source_message_id: None,
        source_booking_id: None,
        source_asset_id: None,
        external_reference: "Venmo note".to_string(),
        notes: "manual status only".to_string(),
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
    }
}

impl ApiState {
    pub fn for_tests() -> Self {
        let config = AppConfig {
            database: DatabaseConfig {
                host: "localhost".to_string(),
                port: 5432,
                name: "ahara_business".to_string(),
                username: "app".to_string(),
                password: "password".to_string(),
            },
            mail: MailConfig {
                domain: "ahara.io".to_string(),
                raw_mail_bucket: "ahara-business-raw-mail-test".to_string(),
                raw_mail_prefix: "raw/".to_string(),
            },
            feedback: FeedbackConfig {
                bounce_topic_arn: "arn:aws:sns:::bounces".to_string(),
                complaint_topic_arn: "arn:aws:sns:::complaints".to_string(),
            },
            api: ApiConfig {
                api_base_url: "https://api.example.test".to_string(),
                app_base_url: "https://app.example.test".to_string(),
            },
            cognito: CognitoConfig {
                user_pool_id: "us-east-1_pool".to_string(),
                client_id: "client-123".to_string(),
                domain: "auth.example.test".to_string(),
                issuer: "https://issuer.example.test".to_string(),
            },
            app_authorizations: AppAuthorizationConfig {
                table_name: "ahara-business-app-authorizations".to_string(),
            },
        };
        let db = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy(&database_url(&config.database))
            .unwrap();
        let domain_config = Arc::new(InMemoryDomainConfigService::with_domains([DomainConfig {
            domain_name: "ahara.io".to_string(),
            routing_policy: RoutingPolicy::Allowlist,
            active: true,
            raw_retention_days: Some(90),
            addresses: vec![
                AcceptedAddress {
                    local_part: "chris".to_string(),
                    active: true,
                    raw_retention_days: None,
                },
                AcceptedAddress {
                    local_part: "contact".to_string(),
                    active: false,
                    raw_retention_days: Some(30),
                },
            ],
        }]));
        let contacts = Arc::new(InMemoryContactsService::with_contacts([Contact {
            id: "contact-1".to_string(),
            display_name: "Chris".to_string(),
            primary_address: Some("Chris@Example.Test".to_string()),
            primary_address_normalized: Some("chris@example.test".to_string()),
            notes: "existing".to_string(),
        }]));
        let forwarding = Arc::new(InMemoryForwardingRuleService::with_addresses([
            ("ahara.io".to_string(), "chris".to_string()),
            ("ahara.io".to_string(), "contact".to_string()),
        ]));
        let app_authorizations = Arc::new(InMemoryAppAuthorizationService::with_users([
            AppAuthorizationUser {
                username: "chris".to_string(),
                email: Some("chris@example.test".to_string()),
                display_name: Some("Chris".to_string()),
                apps: [("ahara-business-app".to_string(), "admin".to_string())]
                    .into_iter()
                    .collect(),
            },
        ]));
        let accepted_message = MailboxMessageDetail {
            id: "00000000-0000-0000-0000-000000000001".to_string(),
            thread_id: Some("00000000-0000-0000-0000-000000000101".to_string()),
            rfc_message_id: Some("<accepted@example.test>".to_string()),
            in_reply_to: None,
            reference_ids: vec![],
            from_address: "sender@example.test".to_string(),
            from_display_name: "Sender Display".to_string(),
            subject: "Invoice".to_string(),
            message_date: Some("2026-01-01 00:00:00+00".to_string()),
            received_at: Some("2026-01-01 00:00:00+00".to_string()),
            body_text: "Plaintext invoice body with auth verdict details.".to_string(),
            recipients: vec![],
            attachments: vec![MailboxAttachment {
                id: "00000000-0000-0000-0000-000000000301".to_string(),
                position: 0,
                filename: "../invoice.pdf".to_string(),
                display_filename: "invoice.pdf".to_string(),
                content_type: "application/pdf".to_string(),
                size_bytes: Some(12),
                content_id: None,
            }],
            is_read: false,
            contact_id: None,
            spf_result: Some(MailboxAuthResult::Pass),
            dkim_result: Some(MailboxAuthResult::Pass),
            dmarc_result: Some(MailboxAuthResult::Pass),
            auth_verdict: Some(MailboxAuthResult::Pass),
            spam_result: Some(MailboxScanResult::Pass),
            virus_result: Some(MailboxScanResult::Pass),
            security_disposition: MailboxSecurityDisposition::Accepted,
            security_reason: Some("clean".to_string()),
        };
        let quarantined_message = MailboxMessageDetail {
            id: "00000000-0000-0000-0000-000000000002".to_string(),
            thread_id: Some("00000000-0000-0000-0000-000000000101".to_string()),
            security_disposition: MailboxSecurityDisposition::Quarantined,
            security_reason: Some("spam_failed".to_string()),
            body_text: "Quarantined invoice body".to_string(),
            ..accepted_message.clone()
        };
        let rejected_message = MailboxMessageDetail {
            id: "00000000-0000-0000-0000-000000000003".to_string(),
            thread_id: Some("00000000-0000-0000-0000-000000000101".to_string()),
            security_disposition: MailboxSecurityDisposition::Rejected,
            security_reason: Some("virus_failed".to_string()),
            body_text: "Rejected invoice body".to_string(),
            ..accepted_message.clone()
        };
        let outbound = Arc::new(InMemoryOutboundService::new(config.mail.domain.clone()));
        outbound.seed_reply_source(InMemoryReplySource {
            id: Uuid::parse_str(&accepted_message.id).unwrap(),
            thread_id: accepted_message
                .thread_id
                .as_deref()
                .map(Uuid::parse_str)
                .transpose()
                .unwrap(),
            rfc_message_id: accepted_message.rfc_message_id.clone(),
            reference_ids: accepted_message.reference_ids.clone(),
            from_address: accepted_message.from_address.clone(),
            subject: accepted_message.subject.clone(),
        });
        let mailbox = Arc::new(InMemoryMailboxService::with_messages([
            InMemoryMailboxMessage::accepted(accepted_message),
            InMemoryMailboxMessage {
                direction: "inbound".to_string(),
                status: "quarantined".to_string(),
                normalized_subject: "invoice".to_string(),
                last_activity_at: Some("2026-01-01 00:01:00+00".to_string()),
                detail: quarantined_message,
            },
            InMemoryMailboxMessage {
                direction: "inbound".to_string(),
                status: "rejected".to_string(),
                normalized_subject: "invoice".to_string(),
                last_activity_at: Some("2026-01-01 00:02:00+00".to_string()),
                detail: rejected_message,
            },
        ]));
        let attachments = Arc::new(InMemoryAttachmentService::with_downloads([
            MailboxAttachmentDownload {
                id: "00000000-0000-0000-0000-000000000301".to_string(),
                message_id: "00000000-0000-0000-0000-000000000001".to_string(),
                filename: "../invoice.pdf".to_string(),
                display_filename: "invoice.pdf".to_string(),
                content_type: "application/pdf".to_string(),
                size_bytes: 12,
                content_id: None,
                content_base64: "cGRmLWNvbnRlbnQ=".to_string(),
            },
        ]));
        Self {
            config,
            db,
            auth: Arc::new(TestAuthVerifier),
            domain_config,
            contacts,
            mailbox,
            attachments,
            raw_mail_store: Arc::new(TestRawMailStore),
            outbound,
            forwarding,
            app_authorizations,
            finance: Arc::new(TestFinanceService),
        }
    }
}
