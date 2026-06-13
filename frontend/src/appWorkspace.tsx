import { MailboxView, type MailboxApi } from "./mailbox";
import { RoutingAdmin, type RoutingAdminApi } from "./routingAdmin";
import { SharedFilesView, type SharedFilesApi } from "./sharedFiles";
import { isSharedFilesApi } from "./sharedFilesTypes";

export type ActiveView = "mailbox" | "contacts" | "routing" | "files";
export type AppApi = MailboxApi & Partial<RoutingAdminApi & SharedFilesApi>;

export function WorkspaceView({
  activeView,
  apiClient,
}: {
  activeView: ActiveView;
  apiClient: AppApi;
}) {
  if (activeView === "mailbox") {
    return <MailboxView apiClient={apiClient} />;
  }
  if (activeView === "files" && isSharedFilesApi(apiClient)) {
    return <SharedFilesView apiClient={apiClient} />;
  }
  if (activeView === "routing" && isRoutingAdminApi(apiClient)) {
    return <RoutingAdmin apiClient={apiClient} />;
  }
  return (
    <div className="empty-state">
      {activeView === "contacts" ? "Contacts" : "Unavailable"}
    </div>
  );
}

function isRoutingAdminApi(
  apiClient: Partial<RoutingAdminApi>,
): apiClient is RoutingAdminApi {
  return Boolean(
    apiClient.listDomains &&
    apiClient.updateDomain &&
    apiClient.addAddress &&
    apiClient.deactivateAddress &&
    apiClient.listForwardingRules &&
    apiClient.upsertForwardingRule &&
    apiClient.deactivateForwardingRule,
  );
}
