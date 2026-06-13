import type { AppApiClient } from "./api";
import type {
  AccessAsset,
  AccessAudience,
  AccessAudienceMember,
  AccessGrant,
  AccessPermissionLevel,
  AccessPrincipal,
} from "./accessTypes";

export type SharedFilesApi = Pick<
  AppApiClient,
  | "listAccessPrincipals"
  | "createAccessPrincipal"
  | "listAccessAudiences"
  | "createAccessAudience"
  | "listAccessAudienceMembers"
  | "addAccessAudienceMember"
  | "listAccessAssets"
  | "uploadAccessAsset"
  | "listAccessGrants"
  | "createAccessGrant"
  | "revokeAccessGrant"
>;

export type SharedFilesState =
  | { status: "loading" }
  | {
      status: "ready";
      assets: AccessAsset[];
      audiences: AccessAudience[];
      grants: AccessGrant[];
      membersByAudience: Record<string, AccessAudienceMember[]>;
      principals: AccessPrincipal[];
    }
  | { status: "error"; message: string };

export type SharedUploadDraft = {
  file: File | null;
  ownerApp: string;
};

export type PrincipalDraft = {
  cognitoSub: string;
  displayName: string;
  email: string;
  kind: AccessPrincipal["principal_kind"];
  username: string;
};

export type AudienceDraft = {
  audienceKey: string;
  description: string;
  displayName: string;
};

export type MemberDraft = {
  audienceId: string;
  principalId: string;
};

export type GrantDraft = {
  assetId: string;
  audienceId: string;
  expiresAt: string;
  granteeKind: "principal" | "audience";
  permissionLevel: AccessPermissionLevel;
  principalId: string;
};

export function isSharedFilesApi(
  apiClient: Partial<SharedFilesApi>,
): apiClient is SharedFilesApi {
  return sharedFilesApiKeys.every((key) => Boolean(apiClient[key]));
}

const sharedFilesApiKeys = [
  "listAccessAssets",
  "uploadAccessAsset",
  "listAccessPrincipals",
  "createAccessPrincipal",
  "listAccessAudiences",
  "createAccessAudience",
  "listAccessAudienceMembers",
  "addAccessAudienceMember",
  "listAccessGrants",
  "createAccessGrant",
  "revokeAccessGrant",
] as const;
