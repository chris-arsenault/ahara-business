import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  CalendarBookingView,
  type CalendarBookingApi,
} from "./calendarBooking";
import type { Booking, CalendarEvent, Contact, IcsCandidate } from "./types";

const contact: Contact = {
  id: "contact-1",
  display_name: "Client",
  primary_address: "client@example.com",
  primary_address_normalized: "client@example.com",
  notes: "",
};

const eventRow: CalendarEvent = {
  id: "event-1",
  title: "Intro",
  status: "tentative",
  starts_at: "2026-06-13 14:00:00+00",
  ends_at: "2026-06-13 14:30:00+00",
  timezone: "UTC",
  location: "Zoom",
  description: "",
  contact_id: "contact-1",
  source_message_id: null,
  source_attachment_id: null,
  created_at: "now",
  updated_at: "now",
};

const booking: Booking = {
  id: "booking-1",
  calendar_event_id: "event-1",
  contact_id: "contact-1",
  title: "Session",
  status: "requested",
  starts_at: "2026-06-13 14:00:00+00",
  ends_at: "2026-06-13 14:30:00+00",
  location: "Zoom",
  notes: "",
  created_at: "now",
  updated_at: "now",
};

const candidate: IcsCandidate = {
  message_id: "message-1",
  attachment_id: "attachment-1",
  filename: "invite.ics",
  content_type: "text/calendar",
  size_bytes: 512,
  subject: "Schedule",
  from_address: "sender@example.com",
  received_at: "2026-06-13 12:00:00+00",
  contact_id: "contact-1",
  suggested_title: "Schedule call",
  suggested_starts_at: "2026-06-15T14:00:00Z",
  suggested_ends_at: "2026-06-15T14:30:00Z",
  suggested_timezone: "America/New_York",
  suggested_location: "Meet",
  suggested_description: "Discuss next steps",
  suggested_status: "confirmed",
  parse_error: null,
};

afterEach(() => cleanup());

describe("CalendarBookingView", () => {
  it("renders events bookings and ICS candidates", async () => {
    render(<CalendarBookingView apiClient={api()} />);

    expect(
      within(await section("Events")).getByText("Intro"),
    ).toBeInTheDocument();
    expect(
      within(await section("Bookings")).getByText("Session"),
    ).toBeInTheDocument();
    expect(
      within(await section("ICS candidates")).getByText("Schedule call"),
    ).toBeInTheDocument();
  });

  it("creates events and updates status", async () => {
    const user = userEvent.setup();
    const calls: string[] = [];
    render(<CalendarBookingView apiClient={api(calls)} />);

    await user.type((await screen.findAllByLabelText("Title"))[0], "Follow up");
    await user.click(screen.getByRole("button", { name: "Add event" }));
    const eventArticle =
      within(await section("Events"))
        .getByText("Intro")
        .closest("article") ?? document.body;
    await user.selectOptions(
      within(eventArticle).getByRole("combobox"),
      "confirmed",
    );

    expect(calls).toContain("create-event:Follow up");
    expect(calls).toContain("event-status:event-1:confirmed");
  });

  it("prefills event draft from parsed ICS candidates", async () => {
    const user = userEvent.setup();
    render(<CalendarBookingView apiClient={api()} />);

    await user.click(await screen.findByRole("button", { name: "Use" }));

    expect((await screen.findAllByLabelText("Title"))[0]).toHaveValue(
      "Schedule call",
    );
    expect(screen.getAllByLabelText("Starts")[0]).toHaveValue(
      "2026-06-15T14:00",
    );
    expect(screen.getAllByLabelText("Ends")[0]).toHaveValue("2026-06-15T14:30");
    expect(screen.getByLabelText("Timezone")).toHaveValue("America/New_York");
    expect(screen.getAllByLabelText("Location")[0]).toHaveValue("Meet");
    expect(screen.getByLabelText("Description")).toHaveValue(
      "Discuss next steps",
    );
  });
});

async function section(name: string) {
  const heading = await screen.findByRole("heading", { name });
  return heading.closest("section") ?? document.body;
}

function api(calls: string[] = []): CalendarBookingApi {
  return {
    listCalendarEvents: async () => [eventRow],
    createCalendarEvent: async (request) => {
      calls.push(`create-event:${request.title}`);
      return {
        ...eventRow,
        ...request,
        id: "event-2",
        status: request.status ?? "tentative",
      };
    },
    updateCalendarEvent: async (id, request) => {
      calls.push(`event-status:${id}:${request.status}`);
      return { ...eventRow, ...request };
    },
    listCalendarIcsCandidates: async () => [candidate],
    listBookings: async () => [booking],
    createBooking: async (request) => ({
      ...booking,
      ...request,
      id: "booking-2",
      status: request.status ?? "requested",
    }),
    updateBooking: async (_id, request) => ({ ...booking, ...request }),
    listContacts: async () => [contact],
  };
}
