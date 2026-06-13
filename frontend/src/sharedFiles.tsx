/* eslint-disable max-lines-per-function */
import { useEffect, useState, type FormEvent } from "react";
import { FolderLock } from "lucide-react";
import type { SharedFileUploadRequest } from "./accessTypes";
import {
  AudiencePanel,
  GrantPanel,
  PrincipalPanel,
  UploadPanel,
} from "./sharedFilesForms";
import { AssetsList, GrantList } from "./sharedFilesLists";
import type {
  AudienceDraft,
  GrantDraft,
  MemberDraft,
  PrincipalDraft,
  SharedFilesApi,
  SharedFilesState,
  SharedUploadDraft,
} from "./sharedFilesTypes";

const defaultUploadDraft = { file: null, ownerApp: "ahara_business" };
const defaultPrincipalDraft: PrincipalDraft = {
  cognitoSub: "",
  displayName: "",
  email: "",
  kind: "external",
  username: "",
};
const defaultAudienceDraft = {
  audienceKey: "",
  description: "",
  displayName: "",
};
const defaultMemberDraft = { audienceId: "", principalId: "" };
const defaultGrantDraft: GrantDraft = {
  assetId: "",
  audienceId: "",
  expiresAt: "",
  granteeKind: "principal",
  permissionLevel: "download",
  principalId: "",
};

export type { SharedFilesApi } from "./sharedFilesTypes";

export function SharedFilesView({ apiClient }: { apiClient: SharedFilesApi }) {
  const [state, setState] = useState<SharedFilesState>({ status: "loading" });
  const [uploadDraft, setUploadDraft] =
    useState<SharedUploadDraft>(defaultUploadDraft);
  const [principalDraft, setPrincipalDraft] = useState<PrincipalDraft>(
    defaultPrincipalDraft,
  );
  const [audienceDraft, setAudienceDraft] =
    useState<AudienceDraft>(defaultAudienceDraft);
  const [memberDraft, setMemberDraft] =
    useState<MemberDraft>(defaultMemberDraft);
  const [grantDraft, setGrantDraft] = useState<GrantDraft>(defaultGrantDraft);
  const [actionError, setActionError] = useState<string>();

  async function load() {
    setState({ status: "loading" });
    try {
      setState(await loadSharedFiles(apiClient));
    } catch (error) {
      setState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to load files",
      });
    }
  }

  useEffect(() => {
    let active = true;
    loadSharedFiles(apiClient)
      .then((nextState) => {
        if (active) {
          setState(nextState);
        }
      })
      .catch((error: unknown) => {
        if (active) {
          setState({
            status: "error",
            message:
              error instanceof Error ? error.message : "Unable to load files",
          });
        }
      });
    return () => {
      active = false;
    };
  }, [apiClient]);

  if (state.status !== "ready") {
    return <SharedFilesShell state={state} />;
  }

  async function runAction(action: () => Promise<void>) {
    setActionError(undefined);
    try {
      await action();
      await load();
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Action failed");
    }
  }

  function revokeGrant(grantId: string) {
    void runAction(async () => {
      await apiClient.revokeAccessGrant(grantId);
    });
  }

  return (
    <section className="admin-panel" aria-labelledby="files-title">
      <Header />
      {actionError ? (
        <div className="error-state compact-error" role="alert">
          {actionError}
        </div>
      ) : null}
      <div className="shared-files-grid">
        <UploadPanel
          draft={uploadDraft}
          onChange={setUploadDraft}
          onSubmit={submit(() =>
            runAction(() => uploadAsset(apiClient, uploadDraft)),
          )}
        />
        <PrincipalPanel
          draft={principalDraft}
          onChange={setPrincipalDraft}
          onSubmit={submit(() =>
            runAction(async () => {
              await createPrincipal(apiClient, principalDraft);
              setPrincipalDraft(defaultPrincipalDraft);
            }),
          )}
        />
        <AudiencePanel
          audiences={state.audiences}
          draft={audienceDraft}
          memberDraft={memberDraft}
          membersByAudience={state.membersByAudience}
          principals={state.principals}
          onAudienceChange={setAudienceDraft}
          onCreateAudience={submit(() =>
            runAction(async () => {
              await createAudience(apiClient, audienceDraft);
              setAudienceDraft(defaultAudienceDraft);
            }),
          )}
          onMemberChange={setMemberDraft}
          onAddMember={submit(() =>
            runAction(() => addMember(apiClient, memberDraft)),
          )}
        />
        <GrantPanel
          assets={state.assets}
          audiences={state.audiences}
          draft={grantDraft}
          principals={state.principals}
          onChange={setGrantDraft}
          onSubmit={submit(() =>
            runAction(() => createGrant(apiClient, grantDraft)),
          )}
        />
      </div>
      <AssetsList assets={state.assets} />
      <GrantList
        assets={state.assets}
        audiences={state.audiences}
        grants={state.grants}
        principals={state.principals}
        onRevoke={revokeGrant}
      />
    </section>
  );
}

function Header() {
  return (
    <header className="admin-toolbar">
      <div className="toolbar-title">
        <FolderLock aria-hidden="true" size={19} />
        <h1 id="files-title">Shared files</h1>
      </div>
    </header>
  );
}

function SharedFilesShell({
  state,
}: {
  state: Extract<SharedFilesState, { status: "loading" | "error" }>;
}) {
  return (
    <section className="admin-panel" aria-labelledby="files-title">
      <Header />
      {state.status === "loading" ? (
        <div className="empty-state" role="status">
          Loading files
        </div>
      ) : (
        <div className="error-state" role="alert">
          {state.message}
        </div>
      )}
    </section>
  );
}

async function loadSharedFiles(
  apiClient: SharedFilesApi,
): Promise<SharedFilesState> {
  const [assets, audiences, grants, principals] = await Promise.all([
    apiClient.listAccessAssets(),
    apiClient.listAccessAudiences(),
    apiClient.listAccessGrants(),
    apiClient.listAccessPrincipals(),
  ]);
  const memberEntries = await Promise.all(
    audiences.map(async (audience) => [
      audience.id,
      await apiClient.listAccessAudienceMembers(audience.id),
    ]),
  );
  return {
    status: "ready",
    assets,
    audiences,
    grants,
    membersByAudience: Object.fromEntries(memberEntries),
    principals,
  };
}

function submit(action: () => Promise<void>) {
  return (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void action();
  };
}

async function uploadAsset(
  apiClient: SharedFilesApi,
  draft: SharedUploadDraft,
) {
  if (!draft.file) {
    throw new Error("Choose a file first");
  }
  await apiClient.uploadAccessAsset(draft.file, uploadRequest(draft));
}

function uploadRequest(draft: SharedUploadDraft): SharedFileUploadRequest {
  const file = draft.file;
  if (!file) {
    throw new Error("Choose a file first");
  }
  return {
    owner_app: draft.ownerApp.trim(),
    filename: file.name,
    content_type: file.type || "application/octet-stream",
    size_bytes: file.size,
  };
}

async function createPrincipal(
  apiClient: SharedFilesApi,
  draft: PrincipalDraft,
) {
  await apiClient.createAccessPrincipal({
    principal_kind: draft.kind,
    display_name: draft.displayName.trim(),
    cognito_sub: optionalText(draft.cognitoSub),
    username: optionalText(draft.username),
    email: optionalText(draft.email),
  });
}

async function createAudience(apiClient: SharedFilesApi, draft: AudienceDraft) {
  await apiClient.createAccessAudience({
    audience_key: draft.audienceKey.trim(),
    display_name: draft.displayName.trim(),
    description: optionalText(draft.description),
  });
}

async function addMember(apiClient: SharedFilesApi, draft: MemberDraft) {
  await apiClient.addAccessAudienceMember(draft.audienceId, draft.principalId);
}

async function createGrant(apiClient: SharedFilesApi, draft: GrantDraft) {
  await apiClient.createAccessGrant({
    asset_id: draft.assetId,
    audience_id: draft.granteeKind === "audience" ? draft.audienceId : null,
    expires_at: expiry(draft.expiresAt),
    permission_level: draft.permissionLevel,
    principal_id: draft.granteeKind === "principal" ? draft.principalId : null,
  });
}

function optionalText(value: string) {
  return value.trim() || null;
}

function expiry(value: string) {
  return value ? new Date(value).toISOString() : null;
}
