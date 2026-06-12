#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    shared::init_tracing();
    tracing::info!("api starting");
    let state = api::ApiState::from_env()
        .await
        .map_err(|err| lambda_http::Error::from(err.to_string()))?;
    lambda_http::run(api::router(state)).await
}
