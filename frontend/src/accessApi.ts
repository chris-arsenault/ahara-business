import { config } from "./config";
import {
  ApiClientError,
  authenticatedRequest,
  defaultFetch,
  uploadHeaders,
  type ApiClientOptions,
  type ApiRequestOptions,
} from "./apiCore";
import type {
  AccessAsset,
  AccessAssetUpload,
  AccessAudience,
  AccessAudienceMember,
  AccessGrant,
  AccessPrincipal,
  CreateAccessAudienceRequest,
  CreateAccessGrantRequest,
  CreateAccessPrincipalRequest,
  SharedFileUploadRequest,
} from "./accessTypes";

export type AccessApiSurface = {
  listAccessPrincipals: () => Promise<AccessPrincipal[]>;
  createAccessPrincipal: (
    request: CreateAccessPrincipalRequest,
  ) => Promise<AccessPrincipal>;
  listAccessAudiences: () => Promise<AccessAudience[]>;
  createAccessAudience: (
    request: CreateAccessAudienceRequest,
  ) => Promise<AccessAudience>;
  listAccessAudienceMembers: (
    audienceId: string,
  ) => Promise<AccessAudienceMember[]>;
  addAccessAudienceMember: (
    audienceId: string,
    principalId: string,
  ) => Promise<AccessAudienceMember>;
  listAccessAssets: () => Promise<AccessAsset[]>;
  uploadAccessAsset: (
    file: File,
    request: SharedFileUploadRequest,
  ) => Promise<AccessAsset>;
  listAccessGrants: () => Promise<AccessGrant[]>;
  createAccessGrant: (
    request: CreateAccessGrantRequest,
  ) => Promise<AccessGrant>;
  revokeAccessGrant: (grantId: string) => Promise<AccessGrant>;
};

export class AccessApiClient implements AccessApiSurface {
  private readonly baseUrl: string;
  private readonly options: ApiClientOptions;

  constructor(options: ApiClientOptions) {
    this.options = options;
    this.baseUrl = (options.accessBaseUrl ?? config.accessApiBaseUrl).replace(
      /\/$/,
      "",
    );
  }

  listAccessPrincipals() {
    return this.request<AccessPrincipal[]>("/principals");
  }

  createAccessPrincipal(request: CreateAccessPrincipalRequest) {
    return this.request<AccessPrincipal>("/principals", post(request));
  }

  listAccessAudiences() {
    return this.request<AccessAudience[]>("/audiences");
  }

  createAccessAudience(request: CreateAccessAudienceRequest) {
    return this.request<AccessAudience>("/audiences", post(request));
  }

  listAccessAudienceMembers(audienceId: string) {
    return this.request<AccessAudienceMember[]>(
      `/audiences/${encodeURIComponent(audienceId)}/members`,
    );
  }

  addAccessAudienceMember(audienceId: string, principalId: string) {
    return this.request<AccessAudienceMember>(
      `/audiences/${encodeURIComponent(audienceId)}/members`,
      post({ principal_id: principalId }),
    );
  }

  listAccessAssets() {
    return this.request<AccessAsset[]>("/assets");
  }

  async uploadAccessAsset(file: File, request: SharedFileUploadRequest) {
    const upload = await this.request<AccessAssetUpload>(
      "/assets/upload-url",
      post(request),
    );
    const response = await (this.options.fetchImpl ?? defaultFetch)(
      upload.upload.url,
      {
        method: upload.upload.method,
        headers: uploadHeaders(upload.upload.headers),
        body: file,
      },
    );
    if (!response.ok) {
      throw new ApiClientError(
        response.status,
        "upload_failed",
        response.statusText || "upload failed",
      );
    }
    return upload.asset;
  }

  listAccessGrants() {
    return this.request<AccessGrant[]>("/grants");
  }

  createAccessGrant(request: CreateAccessGrantRequest) {
    return this.request<AccessGrant>("/grants", post(request));
  }

  revokeAccessGrant(grantId: string) {
    return this.request<AccessGrant>(
      `/grants/${encodeURIComponent(grantId)}/revoke`,
      { method: "POST" },
    );
  }

  private request<T>(path: string, requestOptions: ApiRequestOptions = {}) {
    return authenticatedRequest<T>({
      baseUrl: this.baseUrl,
      clientOptions: this.options,
      path,
      requestOptions,
    });
  }
}

export function attachAccessApi<T extends object>(
  baseClient: T,
  options: ApiClientOptions,
): T & AccessApiSurface {
  const access = new AccessApiClient(options);
  return Object.assign(baseClient, bindAccessApi(access));
}

function bindAccessApi(access: AccessApiClient): AccessApiSurface {
  return {
    listAccessPrincipals: () => access.listAccessPrincipals(),
    createAccessPrincipal: (request) => access.createAccessPrincipal(request),
    listAccessAudiences: () => access.listAccessAudiences(),
    createAccessAudience: (request) => access.createAccessAudience(request),
    listAccessAudienceMembers: (audienceId) =>
      access.listAccessAudienceMembers(audienceId),
    addAccessAudienceMember: (audienceId, principalId) =>
      access.addAccessAudienceMember(audienceId, principalId),
    listAccessAssets: () => access.listAccessAssets(),
    uploadAccessAsset: (file, request) =>
      access.uploadAccessAsset(file, request),
    listAccessGrants: () => access.listAccessGrants(),
    createAccessGrant: (request) => access.createAccessGrant(request),
    revokeAccessGrant: (grantId) => access.revokeAccessGrant(grantId),
  };
}

function post(body: unknown) {
  return { method: "POST", body };
}
