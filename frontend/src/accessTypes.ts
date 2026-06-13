export type AccessPrincipalKind = "operator" | "external";

export type AccessPrincipal = {
  id: string;
  principal_kind: AccessPrincipalKind;
  cognito_sub: string | null;
  username: string | null;
  email: string | null;
  display_name: string;
  active: boolean;
};

export type CreateAccessPrincipalRequest = {
  principal_kind: AccessPrincipalKind;
  display_name: string;
} & Partial<{
  cognito_sub: string | null;
  username: string | null;
  email: string | null;
}>;

export type AccessAudience = {
  id: string;
  audience_key: string;
  display_name: string;
  description: string | null;
  active: boolean;
};

export type CreateAccessAudienceRequest = {
  audience_key: string;
  display_name: string;
} & Partial<{
  description: string | null;
}>;

export type AccessAudienceMember = {
  audience_id: string;
  principal_id: string;
};

export type AccessAsset = {
  id: string;
  owner_app: string;
  resource_id: string | null;
  storage_kind: "managed_s3" | "external_s3";
  filename: string;
  content_type: string;
  size_bytes: number | null;
  sha256: string | null;
  active: boolean;
};

export type SharedFileUploadRequest = {
  owner_app: string;
  filename: string;
  content_type: string;
} & Partial<{
  resource_id: string | null;
  size_bytes: number | null;
  sha256: string | null;
  expires_in_seconds: number | null;
}>;

export type AccessSignedUrl = {
  url: string;
  method: string;
  headers: Record<string, string>;
  expires_in_seconds: number;
};

export type AccessAssetUpload = {
  asset: AccessAsset;
  upload: AccessSignedUrl;
};

export type AccessPermissionLevel = "view" | "download" | "manage";

export type AccessGrant = {
  id: string;
  principal_id: string | null;
  audience_id: string | null;
  resource_id: string | null;
  asset_id: string | null;
  permission_level: AccessPermissionLevel;
  expires_at: string | null;
  revoked_at: string | null;
};

export type CreateAccessGrantRequest = {
  permission_level: AccessPermissionLevel;
} & Partial<{
  principal_id: string | null;
  audience_id: string | null;
  resource_id: string | null;
  asset_id: string | null;
  expires_at: string | null;
}>;
