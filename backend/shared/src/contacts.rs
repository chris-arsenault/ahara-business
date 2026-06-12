use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::routing::parse_route;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub display_name: String,
    pub primary_address: Option<String>,
    pub primary_address_normalized: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateContactRequest {
    pub display_name: String,
    pub primary_address: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateContactRequest {
    pub display_name: Option<String>,
    pub primary_address: Option<String>,
    pub notes: Option<String>,
}

#[async_trait]
pub trait ContactsService: Send + Sync {
    async fn list_contacts(&self) -> AppResult<Vec<Contact>>;
    async fn create_contact(&self, request: CreateContactRequest) -> AppResult<Contact>;
    async fn get_contact(&self, contact_id: &str) -> AppResult<Contact>;
    async fn update_contact(
        &self,
        contact_id: &str,
        request: UpdateContactRequest,
    ) -> AppResult<Contact>;
}

#[derive(Debug, Clone)]
pub struct PgContactsService {
    pool: DbPool,
}

impl PgContactsService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContactsService for PgContactsService {
    async fn list_contacts(&self) -> AppResult<Vec<Contact>> {
        let rows: Vec<ContactRow> = sqlx::query_as(
            "SELECT id, display_name, primary_address, primary_address_normalized, notes
             FROM contacts
             ORDER BY lower(display_name), created_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn create_contact(&self, request: CreateContactRequest) -> AppResult<Contact> {
        let (primary_address, primary_address_normalized) =
            normalize_primary_address(request.primary_address.as_deref())?;
        let row: ContactRow = sqlx::query_as(
            "INSERT INTO contacts (
                 display_name,
                 primary_address,
                 primary_address_normalized,
                 notes
             )
             VALUES ($1, $2, $3, $4)
             RETURNING id, display_name, primary_address, primary_address_normalized, notes",
        )
        .bind(request.display_name)
        .bind(primary_address)
        .bind(primary_address_normalized)
        .bind(request.notes.unwrap_or_default())
        .fetch_one(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        Ok(row.into())
    }

    async fn get_contact(&self, contact_id: &str) -> AppResult<Contact> {
        let contact_id = parse_contact_id(contact_id)?;
        let row: ContactRow = sqlx::query_as(
            "SELECT id, display_name, primary_address, primary_address_normalized, notes
             FROM contacts
             WHERE id = $1",
        )
        .bind(contact_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("contact {contact_id}")))?;

        Ok(row.into())
    }

    async fn update_contact(
        &self,
        contact_id: &str,
        request: UpdateContactRequest,
    ) -> AppResult<Contact> {
        let existing = self.get_contact(contact_id).await?;
        let contact_id = parse_contact_id(contact_id)?;
        let (primary_address, primary_address_normalized) = match request.primary_address {
            Some(primary_address) => normalize_primary_address(Some(&primary_address))?,
            None => (
                existing.primary_address.clone(),
                existing.primary_address_normalized.clone(),
            ),
        };

        let row: ContactRow = sqlx::query_as(
            "UPDATE contacts
             SET display_name = $2,
                 primary_address = $3,
                 primary_address_normalized = $4,
                 notes = $5,
                 updated_at = now()
             WHERE id = $1
             RETURNING id, display_name, primary_address, primary_address_normalized, notes",
        )
        .bind(contact_id)
        .bind(request.display_name.unwrap_or(existing.display_name))
        .bind(primary_address)
        .bind(primary_address_normalized)
        .bind(request.notes.unwrap_or(existing.notes))
        .fetch_one(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        Ok(row.into())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ContactRow {
    id: Uuid,
    display_name: String,
    primary_address: Option<String>,
    primary_address_normalized: Option<String>,
    notes: String,
}

impl From<ContactRow> for Contact {
    fn from(value: ContactRow) -> Self {
        Self {
            id: value.id.to_string(),
            display_name: value.display_name,
            primary_address: value.primary_address,
            primary_address_normalized: value.primary_address_normalized,
            notes: value.notes,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryContactsService {
    contacts: Arc<Mutex<BTreeMap<String, Contact>>>,
}

impl InMemoryContactsService {
    pub fn with_contacts(contacts: impl IntoIterator<Item = Contact>) -> Self {
        Self {
            contacts: Arc::new(Mutex::new(
                contacts
                    .into_iter()
                    .map(|contact| (contact.id.clone(), contact))
                    .collect(),
            )),
        }
    }
}

#[async_trait]
impl ContactsService for InMemoryContactsService {
    async fn list_contacts(&self) -> AppResult<Vec<Contact>> {
        Ok(self.contacts.lock().unwrap().values().cloned().collect())
    }

    async fn create_contact(&self, request: CreateContactRequest) -> AppResult<Contact> {
        let (primary_address, primary_address_normalized) =
            normalize_primary_address(request.primary_address.as_deref())?;
        let contact = Contact {
            id: Uuid::new_v4().to_string(),
            display_name: request.display_name,
            primary_address,
            primary_address_normalized,
            notes: request.notes.unwrap_or_default(),
        };

        self.contacts
            .lock()
            .unwrap()
            .insert(contact.id.clone(), contact.clone());
        Ok(contact)
    }

    async fn get_contact(&self, contact_id: &str) -> AppResult<Contact> {
        self.contacts
            .lock()
            .unwrap()
            .get(contact_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("contact {contact_id}")))
    }

    async fn update_contact(
        &self,
        contact_id: &str,
        request: UpdateContactRequest,
    ) -> AppResult<Contact> {
        let mut contacts = self.contacts.lock().unwrap();
        let contact = contacts
            .get_mut(contact_id)
            .ok_or_else(|| AppError::NotFound(format!("contact {contact_id}")))?;

        if let Some(display_name) = request.display_name {
            contact.display_name = display_name;
        }
        if let Some(primary_address) = request.primary_address {
            let (address, normalized) = normalize_primary_address(Some(&primary_address))?;
            contact.primary_address = address;
            contact.primary_address_normalized = normalized;
        }
        if let Some(notes) = request.notes {
            contact.notes = notes;
        }

        Ok(contact.clone())
    }
}

fn parse_contact_id(contact_id: &str) -> AppResult<Uuid> {
    Uuid::parse_str(contact_id)
        .map_err(|_| AppError::Validation("contact id must be a UUID".to_string()))
}

fn normalize_primary_address(address: Option<&str>) -> AppResult<(Option<String>, Option<String>)> {
    let Some(address) = address.map(str::trim).filter(|address| !address.is_empty()) else {
        return Ok((None, None));
    };
    parse_route(address).map_err(|err| AppError::Validation(err.to_string()))?;
    Ok((
        Some(address.to_string()),
        Some(address.to_ascii_lowercase()),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        Contact, ContactsService, CreateContactRequest, InMemoryContactsService,
        UpdateContactRequest,
    };
    use crate::error::AppError;

    fn existing_contact() -> Contact {
        Contact {
            id: "contact-1".to_string(),
            display_name: "Chris".to_string(),
            primary_address: Some("Chris@Example.Test".to_string()),
            primary_address_normalized: Some("chris@example.test".to_string()),
            notes: "existing".to_string(),
        }
    }

    fn service() -> InMemoryContactsService {
        InMemoryContactsService::with_contacts([existing_contact()])
    }

    #[tokio::test]
    async fn contacts_lists_existing_contacts() {
        let contacts = service().list_contacts().await.unwrap();

        assert_eq!(contacts, vec![existing_contact()]);
    }

    #[tokio::test]
    async fn contacts_creates_contacts_with_normalized_primary_address() {
        let contact = service()
            .create_contact(CreateContactRequest {
                display_name: "Support".to_string(),
                primary_address: Some("Support@Ahara.IO".to_string()),
                notes: None,
            })
            .await
            .unwrap();

        assert_eq!(contact.display_name, "Support");
        assert_eq!(contact.primary_address.as_deref(), Some("Support@Ahara.IO"));
        assert_eq!(
            contact.primary_address_normalized.as_deref(),
            Some("support@ahara.io")
        );
        assert_eq!(contact.notes, "");
    }

    #[tokio::test]
    async fn contacts_gets_and_updates_contacts() {
        let service = service();
        let fetched = service.get_contact("contact-1").await.unwrap();
        let updated = service
            .update_contact(
                "contact-1",
                UpdateContactRequest {
                    display_name: Some("Chris A".to_string()),
                    primary_address: Some("Chris+A@Example.Test".to_string()),
                    notes: Some("updated".to_string()),
                },
            )
            .await
            .unwrap();

        assert_eq!(fetched.display_name, "Chris");
        assert_eq!(updated.display_name, "Chris A");
        assert_eq!(
            updated.primary_address_normalized.as_deref(),
            Some("chris+a@example.test")
        );
        assert_eq!(updated.notes, "updated");
    }

    #[tokio::test]
    async fn contacts_rejects_invalid_primary_address() {
        let err = service()
            .create_contact(CreateContactRequest {
                display_name: "Broken".to_string(),
                primary_address: Some("not-an-address".to_string()),
                notes: None,
            })
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn contacts_reports_not_found() {
        let err = service().get_contact("missing").await.unwrap_err();

        assert!(matches!(err, AppError::NotFound(_)));
    }
}
