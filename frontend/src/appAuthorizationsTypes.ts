import type { ApiClient } from "./api";

export type AppAuthorizationsApi = Pick<
  ApiClient,
  | "listAppAuthorizationUsers"
  | "upsertAppAuthorizationUser"
  | "deleteAppAuthorizationUser"
>;

export function isAppAuthorizationsApi(
  apiClient: Partial<AppAuthorizationsApi>,
): apiClient is AppAuthorizationsApi {
  return Boolean(
    apiClient.listAppAuthorizationUsers &&
    apiClient.upsertAppAuthorizationUser &&
    apiClient.deleteAppAuthorizationUser,
  );
}
