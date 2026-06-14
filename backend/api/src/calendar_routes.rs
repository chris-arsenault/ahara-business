use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, patch};
use axum::{Json, Router};
use shared::calendar::{
    Booking, BookingQuery, CalendarEvent, CalendarEventQuery, CreateBookingRequest,
    CreateCalendarEventRequest, IcsCandidate, PgCalendarService, UpdateBookingRequest,
    UpdateCalendarEventRequest,
};

use crate::{ApiError, ApiState, require_user};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/calendar/events", get(list_events).post(create_event))
        .route("/calendar/events/{event_id}", patch(update_event))
        .route("/calendar/ics-candidates", get(list_ics_candidates))
        .route("/bookings", get(list_bookings).post(create_booking))
        .route("/bookings/{booking_id}", patch(update_booking))
}

async fn list_events(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<CalendarEventQuery>,
) -> Result<Json<Vec<CalendarEvent>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(service(&state).list_events(query).await?))
}

async fn create_event(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateCalendarEventRequest>,
) -> Result<Json<CalendarEvent>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(service(&state).create_event(request).await?))
}

async fn update_event(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(event_id): Path<String>,
    Json(request): Json<UpdateCalendarEventRequest>,
) -> Result<Json<CalendarEvent>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        service(&state).update_event(&event_id, request).await?,
    ))
}

async fn list_ics_candidates(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<IcsCandidate>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(service(&state).list_ics_candidates().await?))
}

async fn list_bookings(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<BookingQuery>,
) -> Result<Json<Vec<Booking>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(service(&state).list_bookings(query).await?))
}

async fn create_booking(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateBookingRequest>,
) -> Result<Json<Booking>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(service(&state).create_booking(request).await?))
}

async fn update_booking(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(booking_id): Path<String>,
    Json(request): Json<UpdateBookingRequest>,
) -> Result<Json<Booking>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        service(&state).update_booking(&booking_id, request).await?,
    ))
}

fn service(state: &ApiState) -> PgCalendarService {
    PgCalendarService::with_raw_mail_store(
        state.db.clone(),
        state.raw_mail_store.clone(),
        shared::inbound::limits::IngestLimits::default(),
    )
}
