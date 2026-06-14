use sqlx::FromRow;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::calendar_types::{
    BookingStatus, CalendarEventStatus, CreateBookingRequest, CreateCalendarEventRequest,
    UpdateBookingRequest, UpdateCalendarEventRequest,
};
use crate::error::{AppError, AppResult};

#[derive(Debug, FromRow)]
pub(crate) struct CalendarEventRow {
    pub(crate) id: Uuid,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) starts_at: String,
    pub(crate) ends_at: String,
    pub(crate) timezone: String,
    pub(crate) location: String,
    pub(crate) description: String,
    pub(crate) contact_id: Option<Uuid>,
    pub(crate) source_message_id: Option<Uuid>,
    pub(crate) source_attachment_id: Option<Uuid>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct BookingRow {
    pub(crate) id: Uuid,
    pub(crate) calendar_event_id: Option<Uuid>,
    pub(crate) contact_id: Option<Uuid>,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) starts_at: String,
    pub(crate) ends_at: String,
    pub(crate) location: String,
    pub(crate) notes: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct IcsCandidateRow {
    pub(crate) message_id: Uuid,
    pub(crate) s3_raw_key: Option<String>,
    pub(crate) raw_deleted_at: Option<String>,
    pub(crate) attachment_id: Uuid,
    pub(crate) position: i32,
    pub(crate) filename: String,
    pub(crate) content_type: String,
    pub(crate) size_bytes: Option<i64>,
    pub(crate) subject: String,
    pub(crate) from_address: String,
    pub(crate) received_at: Option<String>,
    pub(crate) contact_id: Option<Uuid>,
}

pub(crate) struct NormalizedEventInput {
    pub(crate) title: String,
    pub(crate) status: CalendarEventStatus,
    pub(crate) starts_at: String,
    pub(crate) ends_at: String,
    pub(crate) timezone: String,
    pub(crate) location: String,
    pub(crate) description: String,
    pub(crate) contact_id: Option<Uuid>,
    pub(crate) source_message_id: Option<Uuid>,
    pub(crate) source_attachment_id: Option<Uuid>,
}

impl NormalizedEventInput {
    pub(crate) fn create(request: CreateCalendarEventRequest) -> AppResult<Self> {
        let (starts_at, ends_at) = validate_time_range(&request.starts_at, &request.ends_at)?;
        Ok(Self {
            title: required_text("title", request.title)?,
            status: request.status.unwrap_or(CalendarEventStatus::Tentative),
            starts_at,
            ends_at,
            timezone: request
                .timezone
                .and_then(optional_text)
                .unwrap_or_else(|| "UTC".to_string()),
            location: request.location.and_then(optional_text).unwrap_or_default(),
            description: request
                .description
                .and_then(optional_text)
                .unwrap_or_default(),
            contact_id: parse_optional_uuid(request.contact_id.as_deref(), "contact id")?,
            source_message_id: parse_optional_uuid(
                request.source_message_id.as_deref(),
                "source message id",
            )?,
            source_attachment_id: parse_optional_uuid(
                request.source_attachment_id.as_deref(),
                "source attachment id",
            )?,
        })
    }

    pub(crate) fn update(
        current: CalendarEventRow,
        request: UpdateCalendarEventRequest,
    ) -> AppResult<Self> {
        let starts_at = request.starts_at.unwrap_or(current.starts_at);
        let ends_at = request.ends_at.unwrap_or(current.ends_at);
        let (starts_at, ends_at) = validate_time_range(&starts_at, &ends_at)?;
        Ok(Self {
            title: request
                .title
                .map_or(Ok(current.title), |v| required_text("title", v))?,
            status: request
                .status
                .unwrap_or(CalendarEventStatus::parse(&current.status)?),
            starts_at,
            ends_at,
            timezone: request
                .timezone
                .and_then(optional_text)
                .unwrap_or(current.timezone),
            location: request
                .location
                .and_then(optional_text)
                .unwrap_or(current.location),
            description: request
                .description
                .and_then(optional_text)
                .unwrap_or(current.description),
            contact_id: request
                .contact_id
                .map(|id| parse_optional_uuid(Some(&id), "contact id"))
                .unwrap_or(Ok(current.contact_id))?,
            source_message_id: request
                .source_message_id
                .map(|id| parse_optional_uuid(Some(&id), "source message id"))
                .unwrap_or(Ok(current.source_message_id))?,
            source_attachment_id: request
                .source_attachment_id
                .map(|id| parse_optional_uuid(Some(&id), "source attachment id"))
                .unwrap_or(Ok(current.source_attachment_id))?,
        })
    }
}

pub(crate) struct NormalizedBookingInput {
    pub(crate) calendar_event_id: Option<Uuid>,
    pub(crate) contact_id: Option<Uuid>,
    pub(crate) title: String,
    pub(crate) status: BookingStatus,
    pub(crate) starts_at: String,
    pub(crate) ends_at: String,
    pub(crate) location: String,
    pub(crate) notes: String,
}

impl NormalizedBookingInput {
    pub(crate) fn create(request: CreateBookingRequest) -> AppResult<Self> {
        let (starts_at, ends_at) = validate_time_range(&request.starts_at, &request.ends_at)?;
        Ok(Self {
            calendar_event_id: parse_optional_uuid(
                request.calendar_event_id.as_deref(),
                "calendar event id",
            )?,
            contact_id: parse_optional_uuid(request.contact_id.as_deref(), "contact id")?,
            title: required_text("title", request.title)?,
            status: request.status.unwrap_or(BookingStatus::Requested),
            starts_at,
            ends_at,
            location: request.location.and_then(optional_text).unwrap_or_default(),
            notes: request.notes.and_then(optional_text).unwrap_or_default(),
        })
    }

    pub(crate) fn update(current: BookingRow, request: UpdateBookingRequest) -> AppResult<Self> {
        let starts_at = request.starts_at.unwrap_or(current.starts_at);
        let ends_at = request.ends_at.unwrap_or(current.ends_at);
        let (starts_at, ends_at) = validate_time_range(&starts_at, &ends_at)?;
        Ok(Self {
            calendar_event_id: request
                .calendar_event_id
                .map(|id| parse_optional_uuid(Some(&id), "calendar event id"))
                .unwrap_or(Ok(current.calendar_event_id))?,
            contact_id: request
                .contact_id
                .map(|id| parse_optional_uuid(Some(&id), "contact id"))
                .unwrap_or(Ok(current.contact_id))?,
            title: request
                .title
                .map_or(Ok(current.title), |v| required_text("title", v))?,
            status: request
                .status
                .unwrap_or(BookingStatus::parse(&current.status)?),
            starts_at,
            ends_at,
            location: request
                .location
                .and_then(optional_text)
                .unwrap_or(current.location),
            notes: request
                .notes
                .and_then(optional_text)
                .unwrap_or(current.notes),
        })
    }
}

pub(crate) fn parse_uuid(value: &str, label: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| AppError::Validation(format!("{label} must be a UUID")))
}

pub(crate) fn parse_optional_uuid(value: Option<&str>, label: &str) -> AppResult<Option<Uuid>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| parse_uuid(value, label))
        .transpose()
}

fn required_text(label: &str, value: String) -> AppResult<String> {
    optional_text(value).ok_or_else(|| AppError::Validation(format!("{label} is required")))
}

fn optional_text(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn validate_time_range(starts_at: &str, ends_at: &str) -> AppResult<(String, String)> {
    let starts_at = starts_at.trim();
    let ends_at = ends_at.trim();
    let start = parse_rfc3339(starts_at, "starts_at")?;
    let end = parse_rfc3339(ends_at, "ends_at")?;
    if end <= start {
        return Err(AppError::Validation(
            "ends_at must be after starts_at".to_string(),
        ));
    }
    Ok((starts_at.to_string(), ends_at.to_string()))
}

fn parse_rfc3339(value: &str, label: &str) -> AppResult<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| AppError::Validation(format!("{label} must be RFC3339")))
}

pub(crate) const EVENT_SELECT_BY_ID: &str =
    "SELECT id, title, status, starts_at::text AS starts_at,
        ends_at::text AS ends_at, timezone, location, description, contact_id,
        source_message_id, source_attachment_id, created_at::text AS created_at,
        updated_at::text AS updated_at
     FROM calendar_events WHERE id = $1";

pub(crate) const EVENT_INSERT: &str = "INSERT INTO calendar_events (
        title, status, starts_at, ends_at, timezone, location, description,
        contact_id, source_message_id, source_attachment_id
    )
    VALUES ($1, $2, $3::timestamptz, $4::timestamptz, $5, $6, $7, $8, $9, $10)
    RETURNING id, title, status, starts_at::text AS starts_at,
        ends_at::text AS ends_at, timezone, location, description, contact_id,
        source_message_id, source_attachment_id, created_at::text AS created_at,
        updated_at::text AS updated_at";

pub(crate) const EVENT_UPDATE: &str = "UPDATE calendar_events
    SET title = $2, status = $3, starts_at = $4::timestamptz,
        ends_at = $5::timestamptz, timezone = $6, location = $7,
        description = $8, contact_id = $9, source_message_id = $10,
        source_attachment_id = $11, updated_at = now()
    WHERE id = $1
    RETURNING id, title, status, starts_at::text AS starts_at,
        ends_at::text AS ends_at, timezone, location, description, contact_id,
        source_message_id, source_attachment_id, created_at::text AS created_at,
        updated_at::text AS updated_at";

pub(crate) const BOOKING_SELECT_BY_ID: &str =
    "SELECT id, calendar_event_id, contact_id, title, status,
        starts_at::text AS starts_at, ends_at::text AS ends_at, location, notes,
        created_at::text AS created_at, updated_at::text AS updated_at
     FROM bookings WHERE id = $1";

pub(crate) const BOOKING_INSERT: &str = "INSERT INTO bookings (
        calendar_event_id, contact_id, title, status, starts_at, ends_at,
        location, notes
    )
    VALUES ($1, $2, $3, $4, $5::timestamptz, $6::timestamptz, $7, $8)
    RETURNING id, calendar_event_id, contact_id, title, status,
        starts_at::text AS starts_at, ends_at::text AS ends_at, location, notes,
        created_at::text AS created_at, updated_at::text AS updated_at";

pub(crate) const BOOKING_UPDATE: &str = "UPDATE bookings
    SET calendar_event_id = $2, contact_id = $3, title = $4, status = $5,
        starts_at = $6::timestamptz, ends_at = $7::timestamptz,
        location = $8, notes = $9, updated_at = now()
    WHERE id = $1
    RETURNING id, calendar_event_id, contact_id, title, status,
        starts_at::text AS starts_at, ends_at::text AS ends_at, location, notes,
        created_at::text AS created_at, updated_at::text AS updated_at";
