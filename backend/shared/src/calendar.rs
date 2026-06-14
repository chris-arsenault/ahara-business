use std::sync::Arc;

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::calendar_ics::{ParsedIcsEvent, parse_ics_event};
use crate::calendar_model::{
    BOOKING_INSERT, BOOKING_SELECT_BY_ID, BOOKING_UPDATE, BookingRow, CalendarEventRow,
    EVENT_INSERT, EVENT_SELECT_BY_ID, EVENT_UPDATE, IcsCandidateRow, NormalizedBookingInput,
    NormalizedEventInput, parse_optional_uuid, parse_uuid,
};
use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::inbound::limits::IngestLimits;
use crate::inbound::mime::extract_attachment_body;
use crate::ports::RawMailStore;

pub use crate::calendar_types::{
    Booking, BookingQuery, BookingStatus, CalendarEvent, CalendarEventQuery, CalendarEventStatus,
    CreateBookingRequest, CreateCalendarEventRequest, IcsCandidate, UpdateBookingRequest,
    UpdateCalendarEventRequest,
};

#[derive(Clone)]
pub struct PgCalendarService {
    pool: DbPool,
    raw_mail_store: Option<Arc<dyn RawMailStore>>,
    limits: IngestLimits,
}

impl PgCalendarService {
    pub fn new(pool: DbPool) -> Self {
        Self {
            pool,
            raw_mail_store: None,
            limits: IngestLimits::default(),
        }
    }

    pub fn with_raw_mail_store(
        pool: DbPool,
        raw_mail_store: Arc<dyn RawMailStore>,
        limits: IngestLimits,
    ) -> Self {
        Self {
            pool,
            raw_mail_store: Some(raw_mail_store),
            limits,
        }
    }

    pub async fn list_events(&self, query: CalendarEventQuery) -> AppResult<Vec<CalendarEvent>> {
        let contact_id = parse_optional_uuid(query.contact_id.as_deref(), "contact id")?;
        let status = query.status.map(CalendarEventStatus::as_db_value);
        let starts_from = optional_time_bound(query.starts_from, "starts_from")?;
        let starts_to = optional_time_bound(query.starts_to, "starts_to")?;
        validate_time_window(starts_from.as_deref(), starts_to.as_deref())?;
        let limit = query.limit.unwrap_or(100).clamp(1, 250);
        let rows: Vec<CalendarEventRow> = sqlx::query_as(
            "SELECT id, title, status, starts_at::text AS starts_at,
                    ends_at::text AS ends_at, timezone, location, description,
                    contact_id, source_message_id, source_attachment_id,
                    created_at::text AS created_at, updated_at::text AS updated_at
             FROM calendar_events
             WHERE ($1::uuid IS NULL OR contact_id = $1)
               AND ($2::text IS NULL OR status = $2)
               AND ($3::timestamptz IS NULL OR starts_at >= $3::timestamptz)
               AND ($4::timestamptz IS NULL OR starts_at < $4::timestamptz)
             ORDER BY starts_at ASC, created_at ASC
             LIMIT $5",
        )
        .bind(contact_id)
        .bind(status)
        .bind(starts_from)
        .bind(starts_to)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    pub async fn create_event(
        &self,
        request: CreateCalendarEventRequest,
    ) -> AppResult<CalendarEvent> {
        let input = NormalizedEventInput::create(request)?;
        let row = sqlx::query_as(EVENT_INSERT)
            .bind(&input.title)
            .bind(input.status.as_db_value())
            .bind(&input.starts_at)
            .bind(&input.ends_at)
            .bind(&input.timezone)
            .bind(&input.location)
            .bind(&input.description)
            .bind(input.contact_id)
            .bind(input.source_message_id)
            .bind(input.source_attachment_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        CalendarEventRow::try_into(row)
    }

    pub async fn update_event(
        &self,
        event_id: &str,
        request: UpdateCalendarEventRequest,
    ) -> AppResult<CalendarEvent> {
        let event_id = parse_uuid(event_id, "event id")?;
        let current = self.event_row(event_id).await?;
        let input = NormalizedEventInput::update(current, request)?;
        let row = sqlx::query_as(EVENT_UPDATE)
            .bind(event_id)
            .bind(&input.title)
            .bind(input.status.as_db_value())
            .bind(&input.starts_at)
            .bind(&input.ends_at)
            .bind(&input.timezone)
            .bind(&input.location)
            .bind(&input.description)
            .bind(input.contact_id)
            .bind(input.source_message_id)
            .bind(input.source_attachment_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        CalendarEventRow::try_into(row)
    }

    pub async fn list_ics_candidates(&self) -> AppResult<Vec<IcsCandidate>> {
        let rows: Vec<IcsCandidateRow> = sqlx::query_as(
            "SELECT messages.id AS message_id, messages.s3_raw_key,
                    messages.raw_deleted_at::text AS raw_deleted_at,
                    attachment_refs.id AS attachment_id, attachment_refs.position,
                    attachment_refs.filename, attachment_refs.content_type,
                    attachment_refs.size_bytes, messages.subject,
                    messages.from_address, messages.received_at::text AS received_at,
                    messages.contact_id
             FROM attachment_refs
             JOIN messages ON messages.id = attachment_refs.message_id
             WHERE messages.direction = 'inbound'
               AND messages.status = 'received'
               AND messages.security_disposition = 'accepted'
               AND (
                   lower(attachment_refs.filename) LIKE '%.ics'
                   OR lower(attachment_refs.content_type) LIKE '%calendar%'
               )
             ORDER BY messages.received_at DESC NULLS LAST, messages.created_at DESC
             LIMIT 100",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
        let mut candidates = Vec::with_capacity(rows.len());
        for row in rows {
            candidates.push(self.candidate_from_row(row).await);
        }
        Ok(candidates)
    }

    pub async fn list_bookings(&self, query: BookingQuery) -> AppResult<Vec<Booking>> {
        let contact_id = parse_optional_uuid(query.contact_id.as_deref(), "contact id")?;
        let status = query.status.map(BookingStatus::as_db_value);
        let limit = query.limit.unwrap_or(100).clamp(1, 250);
        let rows: Vec<BookingRow> = sqlx::query_as(
            "SELECT id, calendar_event_id, contact_id, title, status,
                    starts_at::text AS starts_at, ends_at::text AS ends_at,
                    location, notes, created_at::text AS created_at,
                    updated_at::text AS updated_at
             FROM bookings
             WHERE ($1::uuid IS NULL OR contact_id = $1)
               AND ($2::text IS NULL OR status = $2)
             ORDER BY starts_at ASC, created_at ASC
             LIMIT $3",
        )
        .bind(contact_id)
        .bind(status)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
        rows.into_iter().map(TryInto::try_into).collect()
    }
}

impl PgCalendarService {
    pub async fn create_booking(&self, request: CreateBookingRequest) -> AppResult<Booking> {
        let input = NormalizedBookingInput::create(request)?;
        let row = sqlx::query_as(BOOKING_INSERT)
            .bind(input.calendar_event_id)
            .bind(input.contact_id)
            .bind(&input.title)
            .bind(input.status.as_db_value())
            .bind(&input.starts_at)
            .bind(&input.ends_at)
            .bind(&input.location)
            .bind(&input.notes)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        BookingRow::try_into(row)
    }

    pub async fn update_booking(
        &self,
        booking_id: &str,
        request: UpdateBookingRequest,
    ) -> AppResult<Booking> {
        let booking_id = parse_uuid(booking_id, "booking id")?;
        let current = self.booking_row(booking_id).await?;
        let input = NormalizedBookingInput::update(current, request)?;
        let row = sqlx::query_as(BOOKING_UPDATE)
            .bind(booking_id)
            .bind(input.calendar_event_id)
            .bind(input.contact_id)
            .bind(&input.title)
            .bind(input.status.as_db_value())
            .bind(&input.starts_at)
            .bind(&input.ends_at)
            .bind(&input.location)
            .bind(&input.notes)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        BookingRow::try_into(row)
    }

    async fn event_row(&self, event_id: Uuid) -> AppResult<CalendarEventRow> {
        sqlx::query_as(EVENT_SELECT_BY_ID)
            .bind(event_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?
            .ok_or_else(|| AppError::NotFound(format!("calendar event {event_id}")))
    }

    async fn booking_row(&self, booking_id: Uuid) -> AppResult<BookingRow> {
        sqlx::query_as(BOOKING_SELECT_BY_ID)
            .bind(booking_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?
            .ok_or_else(|| AppError::NotFound(format!("booking {booking_id}")))
    }

    async fn candidate_from_row(&self, row: IcsCandidateRow) -> IcsCandidate {
        let mut candidate = candidate_metadata(&row);
        match self.parse_candidate_event(&row).await {
            Ok(Some(event)) => apply_parsed_event(&mut candidate, event),
            Ok(None) => {}
            Err(err) => candidate.parse_error = Some(err.public_message()),
        }
        candidate
    }

    async fn parse_candidate_event(
        &self,
        row: &IcsCandidateRow,
    ) -> AppResult<Option<ParsedIcsEvent>> {
        let Some(raw_mail_store) = &self.raw_mail_store else {
            return Ok(None);
        };
        if row.raw_deleted_at.is_some() {
            return Err(AppError::NotFound(
                "raw mail for calendar invite".to_string(),
            ));
        }
        let raw_key = row
            .s3_raw_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::NotFound("raw mail for calendar invite".to_string()))?;
        let raw = raw_mail_store.get_raw_mail(raw_key).await?;
        let body = extract_attachment_body(&raw.bytes, row.position, self.limits)
            .map_err(|err| AppError::Validation(err.to_string()))?
            .ok_or_else(|| AppError::NotFound(format!("attachment {}", row.attachment_id)))?;
        parse_ics_event(&body.bytes)
    }
}

impl TryFrom<CalendarEventRow> for CalendarEvent {
    type Error = AppError;

    fn try_from(value: CalendarEventRow) -> AppResult<Self> {
        Ok(Self {
            id: value.id.to_string(),
            title: value.title,
            status: CalendarEventStatus::parse(&value.status)?,
            starts_at: value.starts_at,
            ends_at: value.ends_at,
            timezone: value.timezone,
            location: value.location,
            description: value.description,
            contact_id: value.contact_id.map(|id| id.to_string()),
            source_message_id: value.source_message_id.map(|id| id.to_string()),
            source_attachment_id: value.source_attachment_id.map(|id| id.to_string()),
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl TryFrom<BookingRow> for Booking {
    type Error = AppError;

    fn try_from(value: BookingRow) -> AppResult<Self> {
        Ok(Self {
            id: value.id.to_string(),
            calendar_event_id: value.calendar_event_id.map(|id| id.to_string()),
            contact_id: value.contact_id.map(|id| id.to_string()),
            title: value.title,
            status: BookingStatus::parse(&value.status)?,
            starts_at: value.starts_at,
            ends_at: value.ends_at,
            location: value.location,
            notes: value.notes,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

fn candidate_metadata(value: &IcsCandidateRow) -> IcsCandidate {
    IcsCandidate {
        message_id: value.message_id.to_string(),
        attachment_id: value.attachment_id.to_string(),
        filename: value.filename.clone(),
        content_type: value.content_type.clone(),
        size_bytes: value.size_bytes,
        subject: value.subject.clone(),
        from_address: value.from_address.clone(),
        received_at: value.received_at.clone(),
        contact_id: value.contact_id.map(|id| id.to_string()),
        suggested_title: None,
        suggested_starts_at: None,
        suggested_ends_at: None,
        suggested_timezone: None,
        suggested_location: None,
        suggested_description: None,
        suggested_status: None,
        parse_error: None,
    }
}

fn apply_parsed_event(candidate: &mut IcsCandidate, event: ParsedIcsEvent) {
    candidate.suggested_title = event.title;
    candidate.suggested_starts_at = Some(event.starts_at);
    candidate.suggested_ends_at = Some(event.ends_at);
    candidate.suggested_timezone = Some(event.timezone);
    candidate.suggested_location = event.location;
    candidate.suggested_description = event.description;
    candidate.suggested_status = event.status;
}

fn optional_time_bound(value: Option<String>, label: &str) -> AppResult<Option<String>> {
    let Some(value) = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    parse_query_time(&value, label)?;
    Ok(Some(value))
}

fn validate_time_window(starts_from: Option<&str>, starts_to: Option<&str>) -> AppResult<()> {
    let (Some(starts_from), Some(starts_to)) = (starts_from, starts_to) else {
        return Ok(());
    };
    if parse_query_time(starts_to, "starts_to")? <= parse_query_time(starts_from, "starts_from")? {
        return Err(AppError::Validation(
            "starts_to must be after starts_from".to_string(),
        ));
    }
    Ok(())
}

fn parse_query_time(value: &str, label: &str) -> AppResult<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| AppError::Validation(format!("{label} must be RFC3339")))
}
