use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use aws_sdk_cognitoidentityprovider::Client as CognitoClient;
use aws_sdk_cognitoidentityprovider::types::{AttributeType, MessageActionType};
use aws_sdk_dynamodb::Client as DdbClient;
use aws_sdk_dynamodb::types::AttributeValue;
use serde::{Deserialize, Serialize};

use crate::config::{AppAuthorizationConfig, CognitoConfig};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppAuthorizationUser {
    pub username: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub apps: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertAppAuthorizationUserRequest {
    pub password: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    #[serde(default)]
    pub apps: HashMap<String, String>,
}

#[async_trait]
pub trait AppAuthorizationService: Send + Sync {
    async fn list_users(&self) -> AppResult<Vec<AppAuthorizationUser>>;
    async fn upsert_user(
        &self,
        username: &str,
        request: UpsertAppAuthorizationUserRequest,
    ) -> AppResult<AppAuthorizationUser>;
    async fn delete_user(&self, username: &str) -> AppResult<()>;
}

#[derive(Clone)]
pub struct AwsAppAuthorizationService {
    ddb: DdbClient,
    cognito: CognitoClient,
    table_name: String,
    user_pool_id: String,
}

impl AwsAppAuthorizationService {
    pub async fn from_config(auth: &AppAuthorizationConfig, cognito: &CognitoConfig) -> Self {
        let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        Self {
            ddb: DdbClient::new(&aws_config),
            cognito: CognitoClient::new(&aws_config),
            table_name: auth.table_name.clone(),
            user_pool_id: cognito.user_pool_id.clone(),
        }
    }

    pub fn new(
        ddb: DdbClient,
        cognito: CognitoClient,
        table_name: impl Into<String>,
        user_pool_id: impl Into<String>,
    ) -> Self {
        Self {
            ddb,
            cognito,
            table_name: table_name.into(),
            user_pool_id: user_pool_id.into(),
        }
    }
}

#[async_trait]
impl AppAuthorizationService for AwsAppAuthorizationService {
    async fn list_users(&self) -> AppResult<Vec<AppAuthorizationUser>> {
        let result = self
            .ddb
            .scan()
            .table_name(&self.table_name)
            .send()
            .await
            .map_err(dynamo_error)?;
        let mut users = result
            .items()
            .iter()
            .filter_map(record_from_item)
            .collect::<Vec<_>>();
        users.sort_by(|left, right| left.username.cmp(&right.username));
        Ok(users)
    }

    async fn upsert_user(
        &self,
        username: &str,
        request: UpsertAppAuthorizationUserRequest,
    ) -> AppResult<AppAuthorizationUser> {
        let password = request.password.clone().and_then(optional_text);
        let record = record_from_request(username, request)?;
        self.ensure_cognito_user(&record, password.as_deref())
            .await?;
        self.ddb
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item_from_record(&record)))
            .send()
            .await
            .map_err(dynamo_error)?;
        Ok(record)
    }

    async fn delete_user(&self, username: &str) -> AppResult<()> {
        let username = normalize_username(username)?;
        self.ddb
            .delete_item()
            .table_name(&self.table_name)
            .key("username", AttributeValue::S(username.clone()))
            .send()
            .await
            .map_err(dynamo_error)?;
        self.disable_cognito_user(&username).await?;
        Ok(())
    }
}

impl AwsAppAuthorizationService {
    async fn ensure_cognito_user(
        &self,
        record: &AppAuthorizationUser,
        password: Option<&str>,
    ) -> AppResult<()> {
        match self
            .cognito
            .admin_get_user()
            .user_pool_id(&self.user_pool_id)
            .username(&record.username)
            .send()
            .await
        {
            Ok(_) => {
                self.cognito
                    .admin_enable_user()
                    .user_pool_id(&self.user_pool_id)
                    .username(&record.username)
                    .send()
                    .await
                    .map_err(cognito_error)?;
                Ok(())
            }
            Err(err) if err.to_string().contains("UserNotFoundException") => {
                self.create_cognito_user(record, password).await
            }
            Err(err) => Err(cognito_error(err)),
        }
    }

    async fn create_cognito_user(
        &self,
        record: &AppAuthorizationUser,
        password: Option<&str>,
    ) -> AppResult<()> {
        let password = password
            .ok_or_else(|| AppError::Validation("password is required for new users".into()))?;
        let mut create = self
            .cognito
            .admin_create_user()
            .user_pool_id(&self.user_pool_id)
            .username(&record.username)
            .message_action(MessageActionType::Suppress);
        if let Some(email) = &record.email {
            create = create
                .user_attributes(attribute("email", email))
                .user_attributes(attribute("email_verified", "true"));
        }
        if let Some(display_name) = &record.display_name {
            create = create.user_attributes(attribute("name", display_name));
        }
        create.send().await.map_err(cognito_error)?;
        self.cognito
            .admin_set_user_password()
            .user_pool_id(&self.user_pool_id)
            .username(&record.username)
            .password(password)
            .permanent(true)
            .send()
            .await
            .map_err(cognito_error)?;
        Ok(())
    }

    async fn disable_cognito_user(&self, username: &str) -> AppResult<()> {
        match self
            .cognito
            .admin_disable_user()
            .user_pool_id(&self.user_pool_id)
            .username(username)
            .send()
            .await
        {
            Ok(_) => Ok(()),
            Err(err) if err.to_string().contains("UserNotFoundException") => Ok(()),
            Err(err) => Err(cognito_error(err)),
        }
    }
}

#[derive(Clone, Default)]
pub struct InMemoryAppAuthorizationService {
    users: Arc<Mutex<HashMap<String, AppAuthorizationUser>>>,
}

impl InMemoryAppAuthorizationService {
    pub fn with_users(users: impl IntoIterator<Item = AppAuthorizationUser>) -> Self {
        Self {
            users: Arc::new(Mutex::new(
                users
                    .into_iter()
                    .map(|user| (user.username.clone(), user))
                    .collect(),
            )),
        }
    }
}

#[async_trait]
impl AppAuthorizationService for InMemoryAppAuthorizationService {
    async fn list_users(&self) -> AppResult<Vec<AppAuthorizationUser>> {
        let mut users = self
            .users
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        users.sort_by(|left, right| left.username.cmp(&right.username));
        Ok(users)
    }

    async fn upsert_user(
        &self,
        username: &str,
        request: UpsertAppAuthorizationUserRequest,
    ) -> AppResult<AppAuthorizationUser> {
        let record = record_from_request(username, request)?;
        self.users
            .lock()
            .unwrap()
            .insert(record.username.clone(), record.clone());
        Ok(record)
    }

    async fn delete_user(&self, username: &str) -> AppResult<()> {
        let username = normalize_username(username)?;
        self.users.lock().unwrap().remove(&username);
        Ok(())
    }
}

fn record_from_request(
    username: &str,
    request: UpsertAppAuthorizationUserRequest,
) -> AppResult<AppAuthorizationUser> {
    let username = normalize_username(username)?;
    let apps = sanitize_apps(request.apps)?;
    Ok(AppAuthorizationUser {
        username: username.clone(),
        email: request.email.and_then(optional_text),
        display_name: request
            .display_name
            .and_then(optional_text)
            .or(Some(username)),
        apps,
    })
}

fn sanitize_apps(raw: HashMap<String, String>) -> AppResult<HashMap<String, String>> {
    raw.into_iter()
        .filter_map(|(key, role)| {
            let key = key.trim().to_string();
            let role = role.trim().to_string();
            if key.is_empty() && role.is_empty() {
                None
            } else {
                Some((key, role))
            }
        })
        .map(|(key, role)| {
            if key.is_empty() || role.is_empty() {
                Err(AppError::Validation(
                    "app authorization keys and roles must be non-empty".into(),
                ))
            } else {
                Ok((key, role))
            }
        })
        .collect()
}

fn record_from_item(item: &HashMap<String, AttributeValue>) -> Option<AppAuthorizationUser> {
    let username = item.get("username")?.as_s().ok()?.clone();
    let apps = item
        .get("apps")
        .and_then(|value| value.as_m().ok())
        .map(apps_from_attribute)
        .unwrap_or_default();
    Some(AppAuthorizationUser {
        username,
        email: item
            .get("email")
            .and_then(|value| value.as_s().ok())
            .cloned(),
        display_name: item
            .get("displayName")
            .and_then(|value| value.as_s().ok())
            .cloned(),
        apps,
    })
}

fn apps_from_attribute(apps: &HashMap<String, AttributeValue>) -> HashMap<String, String> {
    apps.iter()
        .filter_map(|(key, value)| value.as_s().ok().map(|role| (key.clone(), role.clone())))
        .collect()
}

fn item_from_record(record: &AppAuthorizationUser) -> HashMap<String, AttributeValue> {
    let mut item = HashMap::from([(
        "username".to_string(),
        AttributeValue::S(record.username.clone()),
    )]);
    if let Some(email) = &record.email {
        item.insert("email".to_string(), AttributeValue::S(email.clone()));
    }
    if let Some(display_name) = &record.display_name {
        item.insert(
            "displayName".to_string(),
            AttributeValue::S(display_name.clone()),
        );
    }
    let apps = record
        .apps
        .iter()
        .filter(|(key, _)| key.as_str() != "__password")
        .map(|(key, role)| (key.clone(), AttributeValue::S(role.clone())))
        .collect();
    item.insert("apps".to_string(), AttributeValue::M(apps));
    item
}

fn normalize_username(username: &str) -> AppResult<String> {
    let trimmed = username.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("username is required".into()));
    }
    if trimmed.contains('"') {
        return Err(AppError::Validation(
            "username contains invalid characters".into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn attribute(name: &str, value: &str) -> AttributeType {
    AttributeType::builder()
        .name(name)
        .value(value)
        .build()
        .expect("static Cognito attribute names are valid")
}

fn dynamo_error(err: impl std::fmt::Display) -> AppError {
    AppError::ExternalService {
        service: "dynamodb",
        message: err.to_string(),
    }
}

fn cognito_error(err: impl std::fmt::Display) -> AppError {
    AppError::ExternalService {
        service: "cognito",
        message: err.to_string(),
    }
}
