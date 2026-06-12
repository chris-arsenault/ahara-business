use lambda_runtime::{Error, LambdaEvent, service_fn};
use shared::inbound::receipt_gate::ReceiptGate;

async fn handler(
    event: LambdaEvent<serde_json::Value>,
    gate: ReceiptGate,
    mail_domain: String,
) -> Result<serde_json::Value, Error> {
    let (payload, context) = event.into_parts();
    receipt_gate::handle_event(payload, &context.request_id, &mail_domain, &gate)
        .await
        .map_err(Error::from)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::init_tracing();
    let gate = ReceiptGate::from_env().map_err(Error::from)?;
    let mail_domain = std::env::var("MAIL_DOMAIN").unwrap_or_else(|_| "ahara.io".to_string());
    lambda_runtime::run(service_fn(move |event| {
        handler(event, gate.clone(), mail_domain.clone())
    }))
    .await
}
