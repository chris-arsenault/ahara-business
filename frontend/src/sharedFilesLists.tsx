import { Trash2 } from "lucide-react";
import type {
  AccessAsset,
  AccessAudience,
  AccessGrant,
  AccessPrincipal,
} from "./accessTypes";

export function AssetsList({ assets }: { assets: AccessAsset[] }) {
  return (
    <section className="shared-table-section" aria-label="Shared assets">
      <h2>Assets</h2>
      <ul className="shared-object-list">
        {assets.map((asset) => (
          <li key={asset.id}>
            <span>{asset.filename}</span>
            <em>{asset.owner_app}</em>
            <strong>{formatBytes(asset.size_bytes)}</strong>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function AudienceSummary({
  audiences,
  membersByAudience,
}: {
  audiences: AccessAudience[];
  membersByAudience: Record<string, { principal_id: string }[]>;
}) {
  return (
    <ul className="shared-compact-list" aria-label="Audience memberships">
      {audiences.map((audience) => (
        <li key={audience.id}>
          <span>{audience.display_name}</span>
          <strong>{membersByAudience[audience.id]?.length ?? 0} members</strong>
        </li>
      ))}
    </ul>
  );
}

export function GrantList({
  assets,
  audiences,
  grants,
  onRevoke,
  principals,
}: {
  assets: AccessAsset[];
  audiences: AccessAudience[];
  grants: AccessGrant[];
  onRevoke: (grantId: string) => void;
  principals: AccessPrincipal[];
}) {
  return (
    <section className="shared-table-section" aria-label="Shared grants">
      <h2>Grants</h2>
      <ul className="shared-object-list">
        {grants.map((grant) => (
          <li key={grant.id}>
            <span>{grantLabel(grant, assets, principals, audiences)}</span>
            <em>{grant.permission_level}</em>
            <strong>{grant.revoked_at ? "revoked" : "active"}</strong>
            <button
              className="icon-button"
              disabled={Boolean(grant.revoked_at)}
              type="button"
              title="Revoke grant"
              aria-label={`Revoke grant ${grant.id}`}
              onClick={() => onRevoke(grant.id)}
            >
              <Trash2 aria-hidden="true" size={15} />
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

function grantLabel(
  grant: AccessGrant,
  assets: AccessAsset[],
  principals: AccessPrincipal[],
  audiences: AccessAudience[],
) {
  const asset = assets.find((item) => item.id === grant.asset_id);
  const principal = principals.find((item) => item.id === grant.principal_id);
  const audience = audiences.find((item) => item.id === grant.audience_id);
  return `${asset?.filename ?? "asset"} -> ${
    principal?.display_name ?? audience?.display_name ?? "grantee"
  }`;
}

function formatBytes(value: number | null) {
  if (value === null) {
    return "unknown";
  }
  if (value < 1024) {
    return `${value} B`;
  }
  return `${Math.round(value / 1024)} KB`;
}
