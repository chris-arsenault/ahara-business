use lambda_runtime::{Error, LambdaEvent, service_fn};

async fn handler(event: LambdaEvent<serde_json::Value>) -> Result<serde_json::Value, Error> {
    let (payload, context) = event.into_parts();
    let config = shared::config::AppConfig::from_env()
        .map_err(shared::error::AppError::from)
        .map_err(Error::from)?;
    let service = ingest::build_service(&config).await.map_err(Error::from)?;
    ingest::handle_event_with_service(payload, &context.request_id, &config.mail.domain, &service)
        .await
        .map_err(Error::from)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::init_tracing();
    lambda_runtime::run(service_fn(handler)).await
}
