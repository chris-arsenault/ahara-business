CREATE TABLE calendar_events (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title                TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'tentative',
    starts_at            TIMESTAMPTZ NOT NULL,
    ends_at              TIMESTAMPTZ NOT NULL,
    timezone             TEXT NOT NULL DEFAULT 'UTC',
    location             TEXT NOT NULL DEFAULT '',
    description          TEXT NOT NULL DEFAULT '',
    contact_id           UUID REFERENCES contacts(id) ON DELETE SET NULL,
    source_message_id    UUID REFERENCES messages(id) ON DELETE SET NULL,
    source_attachment_id UUID REFERENCES attachment_refs(id) ON DELETE SET NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT calendar_events_title_not_empty CHECK (title <> ''),
    CONSTRAINT calendar_events_timezone_not_empty CHECK (timezone <> ''),
    CONSTRAINT calendar_events_status_valid
        CHECK (status IN ('tentative', 'confirmed', 'canceled', 'completed', 'missed')),
    CONSTRAINT calendar_events_time_order CHECK (ends_at > starts_at)
);

CREATE INDEX idx_calendar_events_starts_at
    ON calendar_events (starts_at, ends_at);

CREATE INDEX idx_calendar_events_contact
    ON calendar_events (contact_id, starts_at)
    WHERE contact_id IS NOT NULL;

CREATE INDEX idx_calendar_events_source_message
    ON calendar_events (source_message_id)
    WHERE source_message_id IS NOT NULL;

CREATE TABLE bookings (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    calendar_event_id UUID REFERENCES calendar_events(id) ON DELETE SET NULL,
    contact_id        UUID REFERENCES contacts(id) ON DELETE SET NULL,
    title             TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'requested',
    starts_at         TIMESTAMPTZ NOT NULL,
    ends_at           TIMESTAMPTZ NOT NULL,
    location          TEXT NOT NULL DEFAULT '',
    notes             TEXT NOT NULL DEFAULT '',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT bookings_title_not_empty CHECK (title <> ''),
    CONSTRAINT bookings_status_valid
        CHECK (status IN ('requested', 'confirmed', 'canceled', 'completed', 'missed')),
    CONSTRAINT bookings_time_order CHECK (ends_at > starts_at)
);

CREATE INDEX idx_bookings_starts_at
    ON bookings (starts_at, ends_at);

CREATE INDEX idx_bookings_contact
    ON bookings (contact_id, starts_at)
    WHERE contact_id IS NOT NULL;

CREATE INDEX idx_bookings_calendar_event
    ON bookings (calendar_event_id)
    WHERE calendar_event_id IS NOT NULL;
