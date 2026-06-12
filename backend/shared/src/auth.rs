use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize};

use crate::config::CognitoConfig;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserContext {
    pub sub: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    email: Option<String>,
    username: Option<String>,
    #[serde(rename = "cognito:username")]
    cognito_username: Option<String>,
    #[serde(default, rename = "cognito:groups")]
    cognito_groups: Vec<String>,
    token_use: Option<String>,
    client_id: Option<String>,
}

#[async_trait]
pub trait AuthVerifier: Send + Sync {
    async fn context_from_authorization(&self, auth_header: Option<&str>)
    -> AppResult<UserContext>;
}

#[async_trait]
pub trait JwksProvider: Send + Sync {
    async fn jwks(&self) -> AppResult<Arc<JwkSet>>;
}

#[derive(Clone)]
pub struct CognitoJwtVerifier {
    issuer: String,
    client_id: String,
    jwks_provider: Arc<dyn JwksProvider>,
}

impl CognitoJwtVerifier {
    pub fn from_config(config: &CognitoConfig) -> Self {
        Self::new(
            config.issuer.clone(),
            config.client_id.clone(),
            Arc::new(HttpJwksProvider::new(&config.issuer)),
        )
    }

    pub fn new(
        issuer: impl Into<String>,
        client_id: impl Into<String>,
        jwks_provider: Arc<dyn JwksProvider>,
    ) -> Self {
        Self {
            issuer: issuer.into(),
            client_id: client_id.into(),
            jwks_provider,
        }
    }

    async fn verify_token(&self, token: &str) -> AppResult<UserContext> {
        let header = decode_header(token)
            .map_err(|_| AppError::Unauthorized("invalid bearer token".to_string()))?;
        let kid = header
            .kid
            .ok_or_else(|| AppError::Unauthorized("bearer token is missing key id".to_string()))?;
        let jwks = self.jwks_provider.jwks().await?;
        let jwk = jwks
            .find(&kid)
            .ok_or_else(|| AppError::Unauthorized("unknown bearer token key".to_string()))?;
        let decoding_key = DecodingKey::from_jwk(jwk)
            .map_err(|_| AppError::Unauthorized("invalid bearer token key".to_string()))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["exp", "iss", "sub"]);
        validation.validate_aud = false;

        let claims = decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|_| AppError::Unauthorized("invalid bearer token".to_string()))?
            .claims;
        if claims.token_use.as_deref() != Some("access") {
            return Err(AppError::Unauthorized(
                "bearer token is not an access token".to_string(),
            ));
        }
        if claims.client_id.as_deref() != Some(self.client_id.as_str()) {
            return Err(AppError::Unauthorized(
                "bearer token was issued for another client".to_string(),
            ));
        }

        Ok(claims.into_user_context())
    }
}

#[async_trait]
impl AuthVerifier for CognitoJwtVerifier {
    async fn context_from_authorization(
        &self,
        auth_header: Option<&str>,
    ) -> AppResult<UserContext> {
        self.verify_token(extract_bearer(auth_header)?).await
    }
}

pub struct HttpJwksProvider {
    url: String,
    client: reqwest::Client,
    cached: Mutex<Option<Arc<JwkSet>>>,
}

impl HttpJwksProvider {
    pub fn new(issuer: &str) -> Self {
        Self {
            url: format!("{}/.well-known/jwks.json", issuer.trim_end_matches('/')),
            client: reqwest::Client::new(),
            cached: Mutex::new(None),
        }
    }
}

#[async_trait]
impl JwksProvider for HttpJwksProvider {
    async fn jwks(&self) -> AppResult<Arc<JwkSet>> {
        if let Some(cached) = self.cached.lock().unwrap().clone() {
            return Ok(cached);
        }

        let response =
            self.client
                .get(&self.url)
                .send()
                .await
                .map_err(|err| AppError::ExternalService {
                    service: "cognito_jwks",
                    message: err.to_string(),
                })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::ExternalService {
                service: "cognito_jwks",
                message: format!("JWKS fetch returned HTTP {status}"),
            });
        }
        let jwks =
            Arc::new(
                response
                    .json::<JwkSet>()
                    .await
                    .map_err(|err| AppError::ExternalService {
                        service: "cognito_jwks",
                        message: err.to_string(),
                    })?,
            );
        *self.cached.lock().unwrap() = Some(jwks.clone());
        Ok(jwks)
    }
}

pub fn extract_bearer(auth_header: Option<&str>) -> AppResult<&str> {
    let header =
        auth_header.ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;

    header
        .strip_prefix("Bearer ")
        .or_else(|| header.strip_prefix("bearer "))
        .ok_or_else(|| AppError::Unauthorized("missing Bearer token".into()))
}

pub fn decode_unverified_claims(token: &str) -> AppResult<UserContext> {
    let payload_b64 = token
        .split('.')
        .nth(1)
        .ok_or_else(|| AppError::Unauthorized("malformed token".into()))?;
    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| AppError::Unauthorized("invalid token encoding".into()))?;
    let claims: Claims = serde_json::from_slice(&payload_bytes)
        .map_err(|_| AppError::Unauthorized("invalid token claims".into()))?;

    Ok(claims.into_user_context())
}

impl Claims {
    fn into_user_context(self) -> UserContext {
        UserContext {
            sub: self.sub,
            email: self.email,
            username: self.username.or(self.cognito_username),
            groups: self.cognito_groups,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use base64::Engine;
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde_json::json;

    use crate::error::AppError;

    use super::{
        AuthVerifier, CognitoJwtVerifier, JwksProvider, decode_unverified_claims, extract_bearer,
    };

    const ISSUER: &str = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_pool";
    const CLIENT_ID: &str = "client-123";
    const TEST_JWKS: &str = r#"{
      "keys": [{
        "kty": "RSA",
        "kid": "test-key-1",
        "use": "sig",
        "alg": "RS256",
        "n": "njA4IKr42Y7IrdpgPNFMhkX-xTudIQfFmXxwuN65FpFYtnPdV478KMpoiyeIFXz4dPiMiE1JnBhlWEehzMP4FdINb6J9ktFjJfCVozL4s2fu9RLb-qpRlZqepn9yOYW3F-tRjGwBJKNA20XBbcAUrGPAtZAAHR6iKyPdWkt9VGXtIWRfcAypZAbWRMdyht7mwRDf17Q8P6DLIRi-fsWVdHNg0KmYUYbpVqmY3j0cBg50TmZ_JsxMnyidwELPbnbHicI42UxM-EkHx34UD6sL83z5wBiy_VanRRJq7CMsT7vCSg4qcVtjDpTAHbBp-wpapalzurBCIMiidrsAXmAfkQ",
        "e": "AQAB"
      }]
    }"#;
    const TEST_PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQCeMDggqvjZjsit
2mA80UyGRf7FO50hB8WZfHC43rkWkVi2c91XjvwoymiLJ4gVfPh0+IyITUmcGGVY
R6HMw/gV0g1von2S0WMl8JWjMvizZ+71Etv6qlGVmp6mf3I5hbcX61GMbAEko0Db
RcFtwBSsY8C1kAAdHqIrI91aS31UZe0hZF9wDKlkBtZEx3KG3ubBEN/XtDw/oMsh
GL5+xZV0c2DQqZhRhulWqZjePRwGDnROZn8mzEyfKJ3AQs9udseJwjjZTEz4SQfH
fhQPqwvzfPnAGLL9VqdFEmrsIyxPu8JKDipxW2MOlMAdsGn7ClqlqXO6sEIgyKJ2
uwBeYB+RAgMBAAECggEAB5D5eYDw1gGKq/VYVnn3+PfLhIyVUSY8t3Q1aHccXEd6
4LM9c/nLXUbp/2nv8n/5QSlx5pBON1cO/E0YBwho/kYygIbBqEgEP/IFF+Eqwyem
Zt0bKri10UjFhbCheoqtfZZMXsHI8wMtNklcw9trQbmkI8StzlBcsMdbR/14+Ud/
16UWUTAVN23cMqOT5Uhyde+qEyuxngtinpKTKTV/7jeImm8TouB5bru9X7WfExQ4
H6azOB1w1GLPFBPZDYBbhUay053hjLshV/rwz0/YRfFFEmNQZ6phnfaZSs0/zy37
l5MB0LzlSZDJfm+4yOvbMVVhH0yJaaA52fUoR/clHwKBgQDflLzu85s2E+j1XK5w
8vkJ+Cxt9ymAf3iK6VYMeVNanHjWQmTpRfdqmXqqDGr7UwahrDNhW32OHsIj6LVJ
zKe3Om2gDmKwLdaI+EIgWHLwZd8IS3RT/TkMEKZgpuFUFjx0JwmQvsl8RjslPV1z
JQdIU7JFM9M0iH/C+efz5/9KXwKBgQC1ICABu2ZeMWl5m1My6mlyt7dCAm+jLqTF
ThSYk0eWqwpf7bBnglPHnhhcE8RT0Hrh7InsEPKpWQaWBraMfO+CDWABY8y+//s1
B/gcyic2SJ8gTZDYX/gAEUNoEIFjcGstPro8V0QQJl3Yxw4gMlD4236F6WfYKdw3
smnLMVO8DwKBgQCQKgYRRb7lBb2GyHYqmmD+jqmHVoHKO2dsmrxDWs/mc1JvRWxw
Bg9dCw3PLCanW4fBI5oVwrqYsziXkuuiZHYYbXJWbDAyTbwxoXJyDNZAME+5t32Q
0ozAPNQrKi/M2nGsq6c9T+f3XAmzH3hsUIn7lwwyFxKuov1OqXlpCkTQnQKBgGNx
peENkOC6ZFyeCQn1ZbvUXkthpwWDAHhLrEcw5ac1dVbB246ZIYKBrIIxYCNcXXtZ
MUho7bJI7LLGMMfleGKBEWrx7mIXjUbKf1DfNLQ7HxLPQ21pE3KGB+pE1aVQ/acz
v3CNwRLU3cW9VGYc+hQH/ulrAtbN9NinniovhPfFAoGAZNIY6w4Ta1G7I/1OvnKX
PKKONW5Ztw+nZJc+KG/MsyGAzjgyRI+s6c4ZAqC726pE+FCWIxQb6lRv3vO6zV+g
omZFC5ifqBliDuvEWxTb6oLV9dIuavMzJ0OWOCUqRLR0CyxlE5WkL7reHJ0COanI
RBbcohTrKyNMlvcmAtCC2aM=
-----END PRIVATE KEY-----"#;

    struct StaticJwksProvider {
        jwks: Arc<jsonwebtoken::jwk::JwkSet>,
    }

    #[async_trait]
    impl JwksProvider for StaticJwksProvider {
        async fn jwks(&self) -> crate::error::AppResult<Arc<jsonwebtoken::jwk::JwkSet>> {
            Ok(self.jwks.clone())
        }
    }

    fn token(payload: serde_json::Value) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string());
        format!("{header}.{payload}.signature")
    }

    fn signed_token(mut payload: serde_json::Value) -> String {
        payload["iss"] = json!(ISSUER);
        payload["client_id"] = json!(CLIENT_ID);
        payload["token_use"] = json!("access");
        payload["exp"] = json!(4_102_444_800u64);
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-key-1".to_string());
        encode(
            &header,
            &payload,
            &EncodingKey::from_rsa_pem(TEST_PRIVATE_KEY.as_bytes()).unwrap(),
        )
        .unwrap()
    }

    fn verifier() -> CognitoJwtVerifier {
        CognitoJwtVerifier::new(
            ISSUER,
            CLIENT_ID,
            Arc::new(StaticJwksProvider {
                jwks: Arc::new(serde_json::from_str(TEST_JWKS).unwrap()),
            }),
        )
    }

    #[test]
    fn extracts_bearer_token() {
        assert_eq!(
            extract_bearer(Some("Bearer abc.def.ghi")).unwrap(),
            "abc.def.ghi"
        );
    }

    #[test]
    fn extracts_lowercase_bearer_token() {
        assert_eq!(
            extract_bearer(Some("bearer abc.def.ghi")).unwrap(),
            "abc.def.ghi"
        );
    }

    #[test]
    fn rejects_missing_bearer_token() {
        let err = extract_bearer(Some("Basic abc")).unwrap_err();

        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[test]
    fn rejects_malformed_token() {
        let err = decode_unverified_claims("not-a-jwt").unwrap_err();

        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[test]
    fn decodes_unverified_cognito_style_token_payload() {
        let context = decode_unverified_claims(&token(json!({
            "sub": "cognito-sub",
            "email": "chris@example.test",
            "cognito:username": "chris",
            "cognito:groups": ["admin", "mail"]
        })))
        .unwrap();

        assert_eq!(context.sub, "cognito-sub");
        assert_eq!(context.email.as_deref(), Some("chris@example.test"));
        assert_eq!(context.username.as_deref(), Some("chris"));
        assert_eq!(context.groups, ["admin", "mail"]);
    }

    #[tokio::test]
    async fn validates_signed_cognito_access_token() {
        let token = signed_token(json!({
            "sub": "cognito-sub",
            "email": "chris@example.test",
            "cognito:username": "chris",
            "cognito:groups": ["admin", "mail"]
        }));

        let context = verifier()
            .context_from_authorization(Some(&format!("Bearer {token}")))
            .await
            .unwrap();

        assert_eq!(context.sub, "cognito-sub");
        assert_eq!(context.email.as_deref(), Some("chris@example.test"));
        assert_eq!(context.username.as_deref(), Some("chris"));
        assert_eq!(context.groups, ["admin", "mail"]);
    }

    #[tokio::test]
    async fn rejects_access_token_for_wrong_client() {
        let mut payload = json!({
            "sub": "cognito-sub",
            "client_id": "other-client"
        });
        payload["iss"] = json!(ISSUER);
        payload["token_use"] = json!("access");
        payload["exp"] = json!(4_102_444_800u64);
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-key-1".to_string());
        let token = encode(
            &header,
            &payload,
            &EncodingKey::from_rsa_pem(TEST_PRIVATE_KEY.as_bytes()).unwrap(),
        )
        .unwrap();

        let err = verifier()
            .context_from_authorization(Some(&format!("Bearer {token}")))
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[tokio::test]
    async fn rejects_expired_access_token() {
        let payload = json!({
            "sub": "cognito-sub",
            "client_id": CLIENT_ID,
            "token_use": "access",
            "iss": ISSUER,
            "exp": 1
        });
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-key-1".to_string());
        let token = encode(
            &header,
            &payload,
            &EncodingKey::from_rsa_pem(TEST_PRIVATE_KEY.as_bytes()).unwrap(),
        )
        .unwrap();

        let err = verifier()
            .context_from_authorization(Some(&format!("Bearer {token}")))
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Unauthorized(_)));
    }
}
