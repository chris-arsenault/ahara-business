import { type FormEvent } from "react";
import { FileUp, Plus, ShieldCheck } from "lucide-react";
import type {
  AccessAsset,
  AccessAudience,
  AccessPrincipal,
} from "./accessTypes";
import type {
  AudienceDraft,
  GrantDraft,
  MemberDraft,
  PrincipalDraft,
  SharedUploadDraft,
} from "./sharedFilesTypes";
import { AudienceSummary } from "./sharedFilesLists";

export function UploadPanel({
  draft,
  onChange,
  onSubmit,
}: {
  draft: SharedUploadDraft;
  onChange: (draft: SharedUploadDraft) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <form className="shared-files-form" onSubmit={onSubmit}>
      <h2>Upload asset</h2>
      <label className="field-control">
        <span>Owner app</span>
        <input
          value={draft.ownerApp}
          onChange={(event) =>
            onChange({ ...draft, ownerApp: event.currentTarget.value })
          }
        />
      </label>
      <label className="field-control">
        <span>File</span>
        <input
          type="file"
          onChange={(event) =>
            onChange({ ...draft, file: event.currentTarget.files?.[0] ?? null })
          }
        />
      </label>
      <button className="secondary-button" disabled={!draft.file} type="submit">
        <FileUp aria-hidden="true" size={15} />
        Upload
      </button>
    </form>
  );
}

export function PrincipalPanel({
  draft,
  onChange,
  onSubmit,
}: {
  draft: PrincipalDraft;
  onChange: (draft: PrincipalDraft) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <form className="shared-files-form" onSubmit={onSubmit}>
      <h2>Principal</h2>
      <label className="field-control">
        <span>Display name</span>
        <input
          value={draft.displayName}
          onChange={(event) =>
            onChange({ ...draft, displayName: event.currentTarget.value })
          }
        />
      </label>
      <label className="field-control">
        <span>Kind</span>
        <select
          value={draft.kind}
          onChange={(event) =>
            onChange({
              ...draft,
              kind: event.currentTarget.value as PrincipalDraft["kind"],
            })
          }
        >
          <option value="external">external</option>
          <option value="operator">operator</option>
        </select>
      </label>
      <IdentityFields draft={draft} onChange={onChange} />
      <button className="secondary-button" type="submit">
        <Plus aria-hidden="true" size={15} />
        Add principal
      </button>
    </form>
  );
}

export function AudiencePanel({
  audiences,
  draft,
  memberDraft,
  membersByAudience,
  onAddMember,
  onAudienceChange,
  onCreateAudience,
  onMemberChange,
  principals,
}: {
  audiences: AccessAudience[];
  draft: AudienceDraft;
  memberDraft: MemberDraft;
  membersByAudience: Record<string, { principal_id: string }[]>;
  onAddMember: (event: FormEvent<HTMLFormElement>) => void;
  onAudienceChange: (draft: AudienceDraft) => void;
  onCreateAudience: (event: FormEvent<HTMLFormElement>) => void;
  onMemberChange: (draft: MemberDraft) => void;
  principals: AccessPrincipal[];
}) {
  function updateMemberAudience(audienceId: string) {
    onMemberChange({ ...memberDraft, audienceId });
  }

  function updateMemberPrincipal(principalId: string) {
    onMemberChange({ ...memberDraft, principalId });
  }

  return (
    <section className="shared-files-form">
      <h2>Audience</h2>
      <form className="shared-inline-form" onSubmit={onCreateAudience}>
        <AudienceDraftFields draft={draft} onChange={onAudienceChange} />
        <button className="secondary-button" type="submit">
          <Plus aria-hidden="true" size={15} />
          Add audience
        </button>
      </form>
      <form className="shared-inline-form" onSubmit={onAddMember}>
        <SelectAudience
          audiences={audiences}
          value={memberDraft.audienceId}
          onChange={updateMemberAudience}
        />
        <SelectPrincipal
          principals={principals}
          value={memberDraft.principalId}
          onChange={updateMemberPrincipal}
        />
        <button className="secondary-button" type="submit">
          Add member
        </button>
      </form>
      <AudienceSummary
        audiences={audiences}
        membersByAudience={membersByAudience}
      />
    </section>
  );
}

export function GrantPanel({
  assets,
  audiences,
  draft,
  onChange,
  onSubmit,
  principals,
}: {
  assets: AccessAsset[];
  audiences: AccessAudience[];
  draft: GrantDraft;
  onChange: (draft: GrantDraft) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  principals: AccessPrincipal[];
}) {
  function updateAsset(assetId: string) {
    onChange({ ...draft, assetId });
  }

  return (
    <form className="shared-files-form" onSubmit={onSubmit}>
      <h2>Grant</h2>
      <SelectAsset
        assets={assets}
        value={draft.assetId}
        onChange={updateAsset}
      />
      <GrantGranteeFields
        audiences={audiences}
        draft={draft}
        onChange={onChange}
        principals={principals}
      />
      <label className="field-control">
        <span>Expires</span>
        <input
          type="datetime-local"
          value={draft.expiresAt}
          onChange={(event) =>
            onChange({ ...draft, expiresAt: event.currentTarget.value })
          }
        />
      </label>
      <button className="secondary-button" type="submit">
        <ShieldCheck aria-hidden="true" size={15} />
        Grant download
      </button>
    </form>
  );
}

function IdentityFields({
  draft,
  onChange,
}: {
  draft: PrincipalDraft;
  onChange: (draft: PrincipalDraft) => void;
}) {
  return (
    <>
      {["username", "email", "cognitoSub"].map((key) => (
        <label className="field-control" key={key}>
          <span>{identityLabel(key)}</span>
          <input
            value={draft[key as keyof PrincipalDraft]}
            onChange={(event) =>
              onChange({ ...draft, [key]: event.currentTarget.value })
            }
          />
        </label>
      ))}
    </>
  );
}

function AudienceDraftFields({
  draft,
  onChange,
}: {
  draft: AudienceDraft;
  onChange: (draft: AudienceDraft) => void;
}) {
  return (
    <>
      <label className="field-control">
        <span>Key</span>
        <input
          value={draft.audienceKey}
          onChange={(event) =>
            onChange({ ...draft, audienceKey: event.currentTarget.value })
          }
        />
      </label>
      <label className="field-control">
        <span>Display name</span>
        <input
          value={draft.displayName}
          onChange={(event) =>
            onChange({ ...draft, displayName: event.currentTarget.value })
          }
        />
      </label>
    </>
  );
}

function GrantGranteeFields({
  audiences,
  draft,
  onChange,
  principals,
}: {
  audiences: AccessAudience[];
  draft: GrantDraft;
  onChange: (draft: GrantDraft) => void;
  principals: AccessPrincipal[];
}) {
  function updateAudience(audienceId: string) {
    onChange({ ...draft, audienceId });
  }

  function updatePrincipal(principalId: string) {
    onChange({ ...draft, principalId });
  }

  return (
    <>
      <label className="field-control">
        <span>Grantee type</span>
        <select
          value={draft.granteeKind}
          onChange={(event) =>
            onChange({
              ...draft,
              granteeKind: event.currentTarget
                .value as GrantDraft["granteeKind"],
            })
          }
        >
          <option value="principal">principal</option>
          <option value="audience">audience</option>
        </select>
      </label>
      {draft.granteeKind === "principal" ? (
        <SelectPrincipal
          principals={principals}
          value={draft.principalId}
          onChange={updatePrincipal}
        />
      ) : (
        <SelectAudience
          audiences={audiences}
          value={draft.audienceId}
          onChange={updateAudience}
        />
      )}
    </>
  );
}

function SelectAsset({
  assets,
  onChange,
  value,
}: {
  assets: AccessAsset[];
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="field-control">
      <span>Asset</span>
      <select value={value} onChange={(event) => onChange(event.target.value)}>
        <option value="">Select asset</option>
        {assets.map((asset) => (
          <option key={asset.id} value={asset.id}>
            {asset.filename}
          </option>
        ))}
      </select>
    </label>
  );
}

function SelectPrincipal({
  onChange,
  principals,
  value,
}: {
  onChange: (value: string) => void;
  principals: AccessPrincipal[];
  value: string;
}) {
  return (
    <label className="field-control">
      <span>Principal</span>
      <select value={value} onChange={(event) => onChange(event.target.value)}>
        <option value="">Select principal</option>
        {principals.map((principal) => (
          <option key={principal.id} value={principal.id}>
            {principal.display_name}
          </option>
        ))}
      </select>
    </label>
  );
}

function SelectAudience({
  audiences,
  onChange,
  value,
}: {
  audiences: AccessAudience[];
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="field-control">
      <span>Audience</span>
      <select value={value} onChange={(event) => onChange(event.target.value)}>
        <option value="">Select audience</option>
        {audiences.map((audience) => (
          <option key={audience.id} value={audience.id}>
            {audience.display_name}
          </option>
        ))}
      </select>
    </label>
  );
}

function identityLabel(key: string) {
  return key === "cognitoSub" ? "Cognito subject" : key;
}
