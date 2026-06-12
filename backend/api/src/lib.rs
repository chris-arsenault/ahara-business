use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};
use shared::auth::{AuthVerifier, CognitoJwtVerifier, UserContext};
use shared::config::AppConfig;
use shared::contacts::{
    Contact, ContactsService, CreateContactRequest, PgContactsService, UpdateContactRequest,
};
use shared::db::{DbPool, connect_pool};
use shared::domain_config::{
    AcceptedAddress, CreateAddressRequest, DomainConfig, DomainConfigService,
    PgDomainConfigService, UpdateDomainRequest,
};
use shared::error::{AppError, AppResult};
use shared::forwarding::{
    ForwardingRuleConfig, ForwardingRuleService, PgForwardingRuleService,
    UpsertForwardingRuleRequest,
};
use shared::mailbox::{
    LinkMessageContactRequest, MailboxMessageDetail, MailboxMessageSummary, MailboxQuery,
    MailboxSearchQuery, MailboxService, MailboxThreadDetail, PgMailboxService,
    UpdateMessageStateRequest,
};
use shared::outbound::{
    ComposeMessageRequest, OutboundMessageDetail, OutboundMessageQueued, OutboundMessageSummary,
    OutboundService, PgOutboundService, ReplyMessageRequest,
};
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct ApiState {
    pub config: AppConfig,
    pub db: DbPool,
    pub auth: Arc<dyn AuthVerifier>,
    pub domain_config: Arc<dyn DomainConfigService>,
    pub contacts: Arc<dyn ContactsService>,
    pub mailbox: Arc<dyn MailboxService>,
    pub outbound: Arc<dyn OutboundService>,
    pub forwarding: Arc<dyn ForwardingRuleService>,
}

impl ApiState {
    pub async fn from_env() -> AppResult<Self> {
        let config = AppConfig::from_env()?;
        let db = connect_pool(&config).await?;
        let domain_config = Arc::new(PgDomainConfigService::new(db.clone()));
        let contacts = Arc::new(PgContactsService::new(db.clone()));
        let mailbox = Arc::new(PgMailboxService::new(db.clone()));
        let outbound = Arc::new(PgOutboundService::new(
            db.clone(),
            config.mail.domain.clone(),
        ));
        let forwarding = Arc::new(PgForwardingRuleService::new(db.clone()));
        Ok(Self {
            auth: Arc::new(CognitoJwtVerifier::from_config(&config.cognito)),
            config,
            db,
            domain_config,
            contacts,
            mailbox,
            outbound,
            forwarding,
        })
    }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/me", get(me))
        .route("/domains", get(list_domains))
        .route(
            "/domains/{domain_name}",
            axum::routing::patch(update_domain),
        )
        .route("/contacts", get(list_contacts).post(create_contact))
        .route(
            "/contacts/{contact_id}",
            get(get_contact).patch(update_contact),
        )
        .route("/mailbox/messages", get(list_mailbox_messages))
        .route("/mailbox/messages/{message_id}", get(get_mailbox_message))
        .route(
            "/mailbox/messages/{message_id}/state",
            axum::routing::patch(update_mailbox_message_state),
        )
        .route(
            "/mailbox/messages/{message_id}/contact",
            axum::routing::patch(link_mailbox_message_contact),
        )
        .route("/mailbox/threads/{thread_id}", get(get_mailbox_thread))
        .route("/mailbox/search", get(search_mailbox_messages))
        .route(
            "/outbound/messages/compose",
            axum::routing::post(compose_outbound_message),
        )
        .route(
            "/mailbox/messages/{message_id}/reply",
            axum::routing::post(reply_to_mailbox_message),
        )
        .route("/outbound/messages", get(list_outbound_messages))
        .route("/outbound/messages/{message_id}", get(get_outbound_message))
        .route(
            "/forwarding/rules",
            get(list_forwarding_rules).post(upsert_forwarding_rule),
        )
        .route(
            "/forwarding/rules/{rule_id}",
            axum::routing::delete(deactivate_forwarding_rule),
        )
        .route(
            "/domains/{domain_name}/addresses",
            axum::routing::post(add_address),
        )
        .route(
            "/domains/{domain_name}/addresses/{local_part}",
            axum::routing::delete(deactivate_address),
        )
        .layer(cors_layer())
        .with_state(state)
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": shared::service_name(),
    }))
}

async fn me(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<UserContext>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(user_context(&state, &headers).await?))
}

async fn list_domains(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DomainConfig>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.domain_config.list_domains().await?))
}

async fn update_domain(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(domain_name): Path<String>,
    Json(request): Json<UpdateDomainRequest>,
) -> Result<Json<DomainConfig>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        state
            .domain_config
            .update_domain(&domain_name, request)
            .await?,
    ))
}

async fn add_address(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(domain_name): Path<String>,
    Json(request): Json<CreateAddressRequest>,
) -> Result<Json<AcceptedAddress>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        state
            .domain_config
            .upsert_address(&domain_name, request)
            .await?,
    ))
}

async fn deactivate_address(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((domain_name, local_part)): Path<(String, String)>,
) -> Result<Json<AcceptedAddress>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        state
            .domain_config
            .deactivate_address(&domain_name, &local_part)
            .await?,
    ))
}

async fn list_contacts(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Contact>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.contacts.list_contacts().await?))
}

async fn create_contact(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateContactRequest>,
) -> Result<Json<Contact>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.contacts.create_contact(request).await?))
}

async fn get_contact(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(contact_id): Path<String>,
) -> Result<Json<Contact>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.contacts.get_contact(&contact_id).await?))
}

async fn update_contact(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(contact_id): Path<String>,
    Json(request): Json<UpdateContactRequest>,
) -> Result<Json<Contact>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        state.contacts.update_contact(&contact_id, request).await?,
    ))
}

async fn list_mailbox_messages(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<MailboxQuery>,
) -> Result<Json<Vec<MailboxMessageSummary>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.mailbox.list_messages(query).await?))
}

async fn get_mailbox_message(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(message_id): Path<String>,
) -> Result<Json<MailboxMessageDetail>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.mailbox.get_message(&message_id).await?))
}

async fn get_mailbox_thread(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
) -> Result<Json<MailboxThreadDetail>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.mailbox.get_thread(&thread_id).await?))
}

async fn search_mailbox_messages(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<MailboxSearchQuery>,
) -> Result<Json<Vec<MailboxMessageSummary>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.mailbox.search_messages(query).await?))
}

async fn update_mailbox_message_state(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(message_id): Path<String>,
    Json(request): Json<UpdateMessageStateRequest>,
) -> Result<Json<MailboxMessageSummary>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        state
            .mailbox
            .update_message_state(&message_id, request)
            .await?,
    ))
}

async fn link_mailbox_message_contact(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(message_id): Path<String>,
    Json(request): Json<LinkMessageContactRequest>,
) -> Result<Json<MailboxMessageSummary>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        state
            .mailbox
            .link_message_contact(&message_id, request)
            .await?,
    ))
}

async fn compose_outbound_message(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<ComposeMessageRequest>,
) -> Result<Json<OutboundMessageQueued>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.outbound.compose_message(request).await?))
}

async fn reply_to_mailbox_message(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(message_id): Path<String>,
    Json(request): Json<ReplyMessageRequest>,
) -> Result<Json<OutboundMessageQueued>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        state
            .outbound
            .reply_to_message(&message_id, request)
            .await?,
    ))
}

async fn list_outbound_messages(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<OutboundMessageSummary>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.outbound.list_outbound_messages().await?))
}

async fn get_outbound_message(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(message_id): Path<String>,
) -> Result<Json<OutboundMessageDetail>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        state.outbound.get_outbound_message(&message_id).await?,
    ))
}

async fn list_forwarding_rules(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ForwardingRuleConfig>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.forwarding.list_rules().await?))
}

async fn upsert_forwarding_rule(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<UpsertForwardingRuleRequest>,
) -> Result<Json<ForwardingRuleConfig>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.forwarding.upsert_address_rule(request).await?))
}

async fn deactivate_forwarding_rule(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(rule_id): Path<String>,
) -> Result<Json<ForwardingRuleConfig>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.forwarding.deactivate_rule(&rule_id).await?))
}

async fn require_user(state: &ApiState, headers: &HeaderMap) -> Result<(), AppError> {
    user_context(state, headers).await.map(|_| ())
}

async fn user_context(state: &ApiState, headers: &HeaderMap) -> Result<UserContext, AppError> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    state
        .auth
        .context_from_authorization(auth_header.as_deref())
        .await
}

pub struct ApiError(AppError);

impl From<AppError> for ApiError {
    fn from(value: AppError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let public = self.0.public_error();
        let status =
            StatusCode::from_u16(public.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (
            status,
            axum::Json(json!({
                "code": public.code,
                "message": public.message,
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
struct TestAuthVerifier;

#[cfg(test)]
#[async_trait::async_trait]
impl AuthVerifier for TestAuthVerifier {
    async fn context_from_authorization(
        &self,
        auth_header: Option<&str>,
    ) -> AppResult<UserContext> {
        shared::auth::decode_unverified_claims(shared::auth::extract_bearer(auth_header)?)
    }
}

#[cfg(test)]
impl ApiState {
    pub fn for_tests() -> Self {
        use shared::config::{
            ApiConfig, CognitoConfig, DatabaseConfig, FeedbackConfig, MailConfig,
        };
        use shared::contacts::{Contact, InMemoryContactsService};
        use shared::db::database_url;
        use shared::domain_config::{AcceptedAddress, DomainConfig, InMemoryDomainConfigService};
        use shared::forwarding::InMemoryForwardingRuleService;
        use shared::mailbox::{
            InMemoryMailboxMessage, InMemoryMailboxService, MailboxAttachment, MailboxAuthResult,
            MailboxMessageDetail, MailboxScanResult, MailboxSecurityDisposition,
        };
        use shared::outbound::{InMemoryOutboundService, InMemoryReplySource};
        use shared::routing::RoutingPolicy;
        use uuid::Uuid;

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
        };
        let db = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy(&database_url(&config.database))
            .unwrap();
        let domain_config = Arc::new(InMemoryDomainConfigService::with_domains([DomainConfig {
            domain_name: "ahara.io".to_string(),
            routing_policy: RoutingPolicy::Allowlist,
            active: true,
            addresses: vec![
                AcceptedAddress {
                    local_part: "chris".to_string(),
                    active: true,
                },
                AcceptedAddress {
                    local_part: "contact".to_string(),
                    active: false,
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
        Self {
            config,
            db,
            auth: Arc::new(TestAuthVerifier),
            domain_config,
            contacts,
            mailbox,
            outbound,
            forwarding,
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use base64::Engine;
    use serde_json::json;
    use tower::ServiceExt;

    use super::{ApiState, router};

    #[tokio::test]
    async fn health_route_returns_service_status_without_auth() {
        let response = router(ApiState::for_tests())
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["service"], "ahara-business");
    }

    fn bearer_token(payload: serde_json::Value) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string());
        format!("Bearer {header}.{payload}.signature")
    }

    #[tokio::test]
    async fn me_route_returns_authenticated_user_context() {
        let auth = bearer_token(json!({
            "sub": "user-sub",
            "email": "chris@example.test",
            "cognito:username": "chris"
        }));
        let response = router(ApiState::for_tests())
            .oneshot(
                Request::builder()
                    .uri("/me")
                    .header("authorization", auth)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["sub"], "user-sub");
        assert_eq!(payload["email"], "chris@example.test");
        assert_eq!(payload["username"], "chris");
    }

    #[tokio::test]
    async fn me_route_rejects_missing_auth_metadata() {
        let response = router(ApiState::for_tests())
            .oneshot(Request::builder().uri("/me").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["code"], "unauthorized");
    }

    #[tokio::test]
    async fn domains_route_lists_configured_domains() {
        let response = authenticated_request("GET", "/domains", None).await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload[0]["domain_name"], "ahara.io");
        assert_eq!(payload[0]["routing_policy"], "allowlist");
        assert_eq!(payload[0]["addresses"][0]["local_part"], "chris");
    }

    #[tokio::test]
    async fn domains_route_updates_policy_and_active_flag() {
        let response = authenticated_request(
            "PATCH",
            "/domains/ahara.io",
            Some(json!({
                "routing_policy": "catchall",
                "active": false
            })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload["routing_policy"], "catchall");
        assert_eq!(payload["active"], false);
    }

    #[tokio::test]
    async fn domains_route_adds_and_reactivates_addresses() {
        let added = authenticated_request(
            "POST",
            "/domains/ahara.io/addresses",
            Some(json!({ "local_part": "Support" })),
        )
        .await;
        let reactivated = authenticated_request(
            "POST",
            "/domains/ahara.io/addresses",
            Some(json!({ "local_part": "contact" })),
        )
        .await;

        assert_eq!(added.status(), StatusCode::OK);
        assert_eq!(reactivated.status(), StatusCode::OK);
        assert_eq!(response_json(added).await["local_part"], "support");
        assert_eq!(response_json(reactivated).await["active"], true);
    }

    #[tokio::test]
    async fn domains_route_deactivates_addresses() {
        let response =
            authenticated_request("DELETE", "/domains/ahara.io/addresses/chris", None).await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload["local_part"], "chris");
        assert_eq!(payload["active"], false);
    }

    #[tokio::test]
    async fn domains_routes_require_auth() {
        let response = router(ApiState::for_tests())
            .oneshot(
                Request::builder()
                    .uri("/domains")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn domains_route_rejects_invalid_routing_policy() {
        let response = authenticated_request(
            "PATCH",
            "/domains/ahara.io",
            Some(json!({ "routing_policy": "forward-all" })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn contacts_route_lists_existing_contacts() {
        let response = authenticated_request("GET", "/contacts", None).await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload[0]["id"], "contact-1");
        assert_eq!(payload[0]["display_name"], "Chris");
    }

    #[tokio::test]
    async fn api_success_responses_include_cors_header_for_browser_origins() {
        let auth = bearer_token(json!({
            "sub": "user-sub",
            "email": "chris@example.test"
        }));
        let response = router(ApiState::for_tests())
            .oneshot(
                Request::builder()
                    .uri("/contacts")
                    .header("authorization", auth)
                    .header("origin", "https://mail.ahara.io")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .and_then(|value| value.to_str().ok()),
            Some("*"),
        );
    }

    #[tokio::test]
    async fn contacts_route_creates_contacts_with_normalized_primary_address() {
        let response = authenticated_request(
            "POST",
            "/contacts",
            Some(json!({
                "display_name": "Support",
                "primary_address": "Support@Ahara.IO",
                "notes": "new"
            })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload["display_name"], "Support");
        assert_eq!(payload["primary_address"], "Support@Ahara.IO");
        assert_eq!(payload["primary_address_normalized"], "support@ahara.io");
    }

    #[tokio::test]
    async fn contacts_route_gets_and_updates_contacts() {
        let fetched = authenticated_request("GET", "/contacts/contact-1", None).await;
        let updated = authenticated_request(
            "PATCH",
            "/contacts/contact-1",
            Some(json!({
                "display_name": "Chris A",
                "primary_address": "Chris+A@Example.Test",
                "notes": "updated"
            })),
        )
        .await;

        assert_eq!(fetched.status(), StatusCode::OK);
        assert_eq!(updated.status(), StatusCode::OK);
        assert_eq!(response_json(fetched).await["display_name"], "Chris");
        let updated = response_json(updated).await;
        assert_eq!(updated["display_name"], "Chris A");
        assert_eq!(
            updated["primary_address_normalized"],
            "chris+a@example.test"
        );
    }

    #[tokio::test]
    async fn contacts_routes_require_auth() {
        let response = router(ApiState::for_tests())
            .oneshot(
                Request::builder()
                    .uri("/contacts")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn contacts_route_reports_not_found() {
        let response = authenticated_request("GET", "/contacts/missing", None).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn contacts_route_rejects_invalid_primary_address() {
        let response = authenticated_request(
            "POST",
            "/contacts",
            Some(json!({
                "display_name": "Broken",
                "primary_address": "not-an-address"
            })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn mailbox_read_routes_require_auth() {
        let response = router(ApiState::for_tests())
            .oneshot(
                Request::builder()
                    .uri("/mailbox/messages")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn mailbox_read_lists_accepted_messages() {
        let response = authenticated_request("GET", "/mailbox/messages", None).await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload.as_array().unwrap().len(), 1);
        assert_eq!(payload[0]["id"], "00000000-0000-0000-0000-000000000001");
        assert_eq!(payload[0]["from_address"], "sender@example.test");
        assert_eq!(payload[0]["from_display_name"], "Sender Display");
        assert_eq!(payload[0]["auth_verdict"], "pass");
        assert_eq!(payload[0]["attachment_count"], 1);
    }

    #[tokio::test]
    async fn mailbox_read_detail_returns_plaintext_auth_and_real_sender() {
        let response = authenticated_request(
            "GET",
            "/mailbox/messages/00000000-0000-0000-0000-000000000001",
            None,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload["from_address"], "sender@example.test");
        assert_eq!(payload["from_display_name"], "Sender Display");
        assert_eq!(
            payload["body_text"],
            "Plaintext invoice body with auth verdict details."
        );
        assert_eq!(payload["auth_verdict"], "pass");
        assert_eq!(payload["security_disposition"], "accepted");
        assert_eq!(payload["attachments"][0]["display_filename"], "invoice.pdf");
    }

    #[tokio::test]
    async fn mailbox_read_thread_excludes_quarantined_and_rejected_messages() {
        let response = authenticated_request(
            "GET",
            "/mailbox/threads/00000000-0000-0000-0000-000000000101",
            None,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload["message_count"], 1);
        assert_eq!(
            payload["messages"][0]["id"],
            "00000000-0000-0000-0000-000000000001"
        );
    }

    #[tokio::test]
    async fn mailbox_read_search_returns_accepted_messages_only() {
        let response = authenticated_request("GET", "/mailbox/search?q=invoice", None).await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload.as_array().unwrap().len(), 1);
        assert_eq!(payload[0]["id"], "00000000-0000-0000-0000-000000000001");
    }

    #[tokio::test]
    async fn mailbox_read_reports_not_found_for_missing_or_nonaccepted_messages() {
        let missing = authenticated_request(
            "GET",
            "/mailbox/messages/00000000-0000-0000-0000-000000000999",
            None,
        )
        .await;
        let quarantined = authenticated_request(
            "GET",
            "/mailbox/messages/00000000-0000-0000-0000-000000000002",
            None,
        )
        .await;

        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(quarantined.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn mailbox_state_routes_require_auth() {
        let response = router(ApiState::for_tests())
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/mailbox/messages/00000000-0000-0000-0000-000000000001/state")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "is_read": true }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn mailbox_state_updates_read_and_unread() {
        let read = authenticated_request(
            "PATCH",
            "/mailbox/messages/00000000-0000-0000-0000-000000000001/state",
            Some(json!({ "is_read": true })),
        )
        .await;
        let unread = authenticated_request(
            "PATCH",
            "/mailbox/messages/00000000-0000-0000-0000-000000000001/state",
            Some(json!({ "is_read": false })),
        )
        .await;

        assert_eq!(read.status(), StatusCode::OK);
        assert_eq!(unread.status(), StatusCode::OK);
        assert_eq!(response_json(read).await["is_read"], true);
        assert_eq!(response_json(unread).await["is_read"], false);
    }

    #[tokio::test]
    async fn mailbox_state_links_and_unlinks_contacts_explicitly() {
        let linked = authenticated_request(
            "PATCH",
            "/mailbox/messages/00000000-0000-0000-0000-000000000001/contact",
            Some(json!({ "contact_id": "00000000-0000-0000-0000-000000000201" })),
        )
        .await;
        let unlinked = authenticated_request(
            "PATCH",
            "/mailbox/messages/00000000-0000-0000-0000-000000000001/contact",
            Some(json!({ "contact_id": null })),
        )
        .await;

        assert_eq!(linked.status(), StatusCode::OK);
        assert_eq!(unlinked.status(), StatusCode::OK);
        assert_eq!(
            response_json(linked).await["contact_id"],
            "00000000-0000-0000-0000-000000000201"
        );
        assert!(response_json(unlinked).await["contact_id"].is_null());
    }

    #[tokio::test]
    async fn mailbox_state_refuses_nonaccepted_mutations() {
        let read_quarantined = authenticated_request(
            "PATCH",
            "/mailbox/messages/00000000-0000-0000-0000-000000000002/state",
            Some(json!({ "is_read": true })),
        )
        .await;
        let link_rejected = authenticated_request(
            "PATCH",
            "/mailbox/messages/00000000-0000-0000-0000-000000000003/contact",
            Some(json!({ "contact_id": "00000000-0000-0000-0000-000000000201" })),
        )
        .await;

        assert_eq!(read_quarantined.status(), StatusCode::NOT_FOUND);
        assert_eq!(link_rejected.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn mailbox_state_rejects_invalid_contact_ids() {
        let response = authenticated_request(
            "PATCH",
            "/mailbox/messages/00000000-0000-0000-0000-000000000001/contact",
            Some(json!({ "contact_id": "Sender Display" })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn outbound_api_compose_queues_message_and_exposes_status() {
        let state = ApiState::for_tests();
        let response = authenticated_request_with_state(
            state.clone(),
            "POST",
            "/outbound/messages/compose",
            Some(json!({
                "from_address": "contact@ahara.io",
                "to": ["person@example.com"],
                "bcc": ["hidden@example.com"],
                "subject": "Plain note",
                "body_text": "text-only body"
            })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let queued = response_json(response).await;
        assert_eq!(queued["status"], "queued");
        assert_eq!(queued["recipients"].as_array().unwrap().len(), 2);

        let listed =
            authenticated_request_with_state(state.clone(), "GET", "/outbound/messages", None)
                .await;
        assert_eq!(listed.status(), StatusCode::OK);
        let listed = response_json(listed).await;
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert_eq!(listed[0]["id"], queued["message_id"]);
        assert_eq!(listed[0]["status"], "queued");
        assert_eq!(listed[0]["primary_recipient"], "person@example.com");
        assert_eq!(listed[0]["recipient_count"], 2);

        let detail_path = format!(
            "/outbound/messages/{}",
            queued["message_id"].as_str().unwrap()
        );
        let detail = authenticated_request_with_state(state, "GET", &detail_path, None).await;
        assert_eq!(detail.status(), StatusCode::OK);
        let detail = response_json(detail).await;
        assert_eq!(detail["status"], "queued");
        assert_eq!(detail["subject"], "Plain note");
        assert_eq!(detail["body_text"], "text-only body");
    }

    #[tokio::test]
    async fn outbound_api_reply_queues_against_accepted_mailbox_message() {
        let state = ApiState::for_tests();
        let response = authenticated_request_with_state(
            state.clone(),
            "POST",
            "/mailbox/messages/00000000-0000-0000-0000-000000000001/reply",
            Some(json!({
                "from_address": "contact@ahara.io",
                "body_text": "reply body"
            })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let queued = response_json(response).await;
        assert_eq!(queued["status"], "queued");
        let detail_path = format!(
            "/outbound/messages/{}",
            queued["message_id"].as_str().unwrap()
        );
        let detail = authenticated_request_with_state(state, "GET", &detail_path, None).await;
        assert_eq!(detail.status(), StatusCode::OK);
        let detail = response_json(detail).await;
        assert_eq!(detail["subject"], "Re: Invoice");
        assert_eq!(detail["in_reply_to"], "<accepted@example.test>");
        assert_eq!(detail["recipients"][0]["address"], "sender@example.test");
    }

    #[tokio::test]
    async fn outbound_api_routes_require_auth() {
        let response = router(ApiState::for_tests())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/outbound/messages/compose")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "from_address": "contact@ahara.io",
                            "to": ["person@example.com"],
                            "subject": "Plain note",
                            "body_text": "body"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn outbound_api_rejects_unowned_from_address() {
        let response = authenticated_request(
            "POST",
            "/outbound/messages/compose",
            Some(json!({
                "from_address": "person@example.com",
                "to": ["target@example.com"],
                "subject": "Nope",
                "body_text": "body"
            })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn forwarding_api_upserts_lists_and_deactivates_rules() {
        let state = ApiState::for_tests();
        let created = authenticated_request_with_state(
            state.clone(),
            "POST",
            "/forwarding/rules",
            Some(json!({
                "domain_name": "Ahara.IO",
                "local_part": "Contact",
                "target_address": "Target@Example.COM"
            })),
        )
        .await;

        assert_eq!(created.status(), StatusCode::OK);
        let created = response_json(created).await;
        assert_eq!(created["domain_name"], "ahara.io");
        assert_eq!(created["local_part"], "contact");
        assert_eq!(created["target_address_normalized"], "target@example.com");
        assert_eq!(created["active"], true);

        let listed =
            authenticated_request_with_state(state.clone(), "GET", "/forwarding/rules", None).await;
        assert_eq!(listed.status(), StatusCode::OK);
        let listed = response_json(listed).await;
        assert_eq!(listed.as_array().unwrap().len(), 1);

        let delete_path = format!("/forwarding/rules/{}", created["id"].as_str().unwrap());
        let deactivated =
            authenticated_request_with_state(state, "DELETE", &delete_path, None).await;
        assert_eq!(deactivated.status(), StatusCode::OK);
        assert_eq!(response_json(deactivated).await["active"], false);
    }

    #[tokio::test]
    async fn forwarding_api_routes_require_auth() {
        let response = router(ApiState::for_tests())
            .oneshot(
                Request::builder()
                    .uri("/forwarding/rules")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn forwarding_api_rejects_unknown_source_address() {
        let response = authenticated_request(
            "POST",
            "/forwarding/rules",
            Some(json!({
                "domain_name": "ahara.io",
                "local_part": "missing",
                "target_address": "target@example.com"
            })),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    async fn authenticated_request(
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> axum::response::Response {
        authenticated_request_with_state(ApiState::for_tests(), method, uri, body).await
    }

    async fn authenticated_request_with_state(
        state: ApiState,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> axum::response::Response {
        let auth = bearer_token(json!({
            "sub": "user-sub",
            "email": "chris@example.test"
        }));
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", auth);
        let body = match body {
            Some(value) => {
                builder = builder.header("content-type", "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };

        router(state)
            .oneshot(builder.body(body).unwrap())
            .await
            .unwrap()
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }
}
