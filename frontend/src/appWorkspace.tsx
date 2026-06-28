import type { ReactNode } from "react";
import { AppAuthorizationsView } from "./appAuthorizations";
import {
  isAppAuthorizationsApi,
  type AppAuthorizationsApi,
} from "./appAuthorizationsTypes";
import {
  CalendarBookingView,
  type CalendarBookingApi,
} from "./calendarBooking";
import { ContactsView, type ContactsApi } from "./contacts";
import { FinanceView, type FinanceApi } from "./finance";
import { ForwardingView } from "./forwardingAudit";
import { isForwardingApi, type ForwardingApi } from "./forwardingAuditTypes";
import { MailboxView, type MailboxApi } from "./mailbox";
import { OpsDashboardView, type OpsDashboardApi } from "./opsDashboard";
import { RoutingAdmin, type RoutingAdminApi } from "./routingAdmin";
import { SharedFilesView, type SharedFilesApi } from "./sharedFiles";
import { isSharedFilesApi } from "./sharedFilesTypes";

export type ActiveView =
  | "mailbox"
  | "contacts"
  | "authorizations"
  | "calendar"
  | "finance"
  | "operations"
  | "routing"
  | "forwarding"
  | "files";
export type AppApi = MailboxApi &
  Partial<
    RoutingAdminApi &
      SharedFilesApi &
      CalendarBookingApi &
      ContactsApi &
      FinanceApi &
      OpsDashboardApi &
      ForwardingApi &
      AppAuthorizationsApi
  >;

export function WorkspaceView({
  activeView,
  apiClient,
}: {
  activeView: ActiveView;
  apiClient: AppApi;
}) {
  const view = viewFor(activeView, apiClient);
  if (view) {
    return view;
  }
  return (
    <div className="empty-state">
      {activeView === "contacts" ? "Contacts" : "Unavailable"}
    </div>
  );
}

function viewFor(activeView: ActiveView, apiClient: AppApi): ReactNode | null {
  const views: Record<ActiveView, () => ReactNode | null> = {
    authorizations: () =>
      isAppAuthorizationsApi(apiClient) ? (
        <AppAuthorizationsView apiClient={apiClient} />
      ) : null,
    calendar: () =>
      isCalendarBookingApi(apiClient) ? (
        <CalendarBookingView apiClient={apiClient} />
      ) : null,
    contacts: () =>
      isContactsApi(apiClient) ? <ContactsView apiClient={apiClient} /> : null,
    files: () =>
      isSharedFilesApi(apiClient) ? (
        <SharedFilesView apiClient={apiClient} />
      ) : null,
    finance: () =>
      isFinanceApi(apiClient) ? <FinanceView apiClient={apiClient} /> : null,
    forwarding: () =>
      isForwardingApi(apiClient) ? (
        <ForwardingView apiClient={apiClient} />
      ) : null,
    mailbox: () => <MailboxView apiClient={apiClient} />,
    operations: () =>
      isOpsDashboardApi(apiClient) ? (
        <OpsDashboardView apiClient={apiClient} />
      ) : null,
    routing: () =>
      isRoutingAdminApi(apiClient) ? (
        <RoutingAdmin apiClient={apiClient} />
      ) : null,
  };
  return views[activeView]();
}

function isOpsDashboardApi(
  apiClient: Partial<OpsDashboardApi>,
): apiClient is OpsDashboardApi {
  return Boolean(
    apiClient.listOperationSummaries && apiClient.listOperationEvents,
  );
}

function isFinanceApi(apiClient: Partial<FinanceApi>): apiClient is FinanceApi {
  return Boolean(
    apiClient.listFinanceExpenses &&
    apiClient.createFinanceExpense &&
    apiClient.createFinanceExpenseOccurrence &&
    apiClient.updateFinanceExpense &&
    apiClient.listFinanceReceivables &&
    apiClient.createFinanceReceivable &&
    apiClient.updateFinanceReceivable &&
    apiClient.getFinanceSummary &&
    apiClient.listContacts,
  );
}

function isContactsApi(
  apiClient: Partial<ContactsApi>,
): apiClient is ContactsApi {
  return Boolean(
    apiClient.listContacts &&
    apiClient.createContact &&
    apiClient.updateContact,
  );
}

function isCalendarBookingApi(
  apiClient: Partial<CalendarBookingApi>,
): apiClient is CalendarBookingApi {
  return Boolean(
    apiClient.listCalendarEvents &&
    apiClient.createCalendarEvent &&
    apiClient.updateCalendarEvent &&
    apiClient.listCalendarIcsCandidates &&
    apiClient.listBookings &&
    apiClient.createBooking &&
    apiClient.updateBooking &&
    apiClient.listContacts,
  );
}

function isRoutingAdminApi(
  apiClient: Partial<RoutingAdminApi>,
): apiClient is RoutingAdminApi {
  return Boolean(
    apiClient.listDomains &&
    apiClient.createDomain &&
    apiClient.updateDomain &&
    apiClient.addAddress &&
    apiClient.updateAddress &&
    apiClient.deactivateAddress,
  );
}
