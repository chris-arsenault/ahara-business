use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalendarEventStatus {
    Tentative,
    Confirmed,
    Canceled,
    Completed,
    Missed,
}

impl CalendarEventStatus {
    pub(crate) fn as_db_value(self) -> &'static str {
        match self {
            Self::Tentative => "tentative",
            Self::Confirmed => "confirmed",
            Self::Canceled => "canceled",
            Self::Completed => "completed",
            Self::Missed => "missed",
        }
    }

    pub(crate) fn parse(value: &str) -> AppResult<Self> {
        match value {
            "tentative" => Ok(Self::Tentative),
            "confirmed" => Ok(Self::Confirmed),
            "canceled" => Ok(Self::Canceled),
            "completed" => Ok(Self::Completed),
            "missed" => Ok(Self::Missed),
            _ => Err(AppError::Internal(format!("unknown event status {value}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BookingStatus {
    Requested,
    Confirmed,
    Canceled,
    Completed,
    Missed,
}

impl BookingStatus {
    pub(crate) fn as_db_value(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Confirmed => "confirmed",
            Self::Canceled => "canceled",
            Self::Completed => "completed",
            Self::Missed => "missed",
        }
    }

    pub(crate) fn parse(value: &str) -> AppResult<Self> {
        match value {
            "requested" => Ok(Self::Requested),
            "confirmed" => Ok(Self::Confirmed),
            "canceled" => Ok(Self::Canceled),
            "completed" => Ok(Self::Completed),
            "missed" => Ok(Self::Missed),
            _ => Err(AppError::Internal(format!(
                "unknown booking status {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub status: CalendarEventStatus,
    pub starts_at: String,
    pub ends_at: String,
    pub timezone: String,
    pub location: String,
    pub description: String,
    pub contact_id: Option<String>,
    pub source_message_id: Option<String>,
    pub source_attachment_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Booking {
    pub id: String,
    pub calendar_event_id: Option<String>,
    pub contact_id: Option<String>,
    pub title: String,
    pub status: BookingStatus,
    pub starts_at: String,
    pub ends_at: String,
    pub location: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IcsCandidate {
    pub message_id: String,
    pub attachment_id: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: Option<i64>,
    pub subject: String,
    pub from_address: String,
    pub received_at: Option<String>,
    pub contact_id: Option<String>,
    pub suggested_title: Option<String>,
    pub suggested_starts_at: Option<String>,
    pub suggested_ends_at: Option<String>,
    pub suggested_timezone: Option<String>,
    pub suggested_location: Option<String>,
    pub suggested_description: Option<String>,
    pub suggested_status: Option<CalendarEventStatus>,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CalendarEventQuery {
    pub contact_id: Option<String>,
    pub status: Option<CalendarEventStatus>,
    pub starts_from: Option<String>,
    pub starts_to: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookingQuery {
    pub contact_id: Option<String>,
    pub status: Option<BookingStatus>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateCalendarEventRequest {
    pub title: String,
    pub starts_at: String,
    pub ends_at: String,
    pub timezone: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub contact_id: Option<String>,
    pub source_message_id: Option<String>,
    pub source_attachment_id: Option<String>,
    pub status: Option<CalendarEventStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateCalendarEventRequest {
    pub title: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub timezone: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub contact_id: Option<String>,
    pub source_message_id: Option<String>,
    pub source_attachment_id: Option<String>,
    pub status: Option<CalendarEventStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateBookingRequest {
    pub title: String,
    pub starts_at: String,
    pub ends_at: String,
    pub calendar_event_id: Option<String>,
    pub contact_id: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub status: Option<BookingStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateBookingRequest {
    pub title: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub calendar_event_id: Option<String>,
    pub contact_id: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub status: Option<BookingStatus>,
}
