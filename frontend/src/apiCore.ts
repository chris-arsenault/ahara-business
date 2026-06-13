export type AccessTokenRequest = Partial<{
  forceRefresh: boolean;
}>;

export type FetchLike = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

export type ApiClientOptions = {
  getAccessToken: (
    request?: AccessTokenRequest,
  ) => Promise<string | undefined> | string | undefined;
} & Partial<{
  baseUrl: string;
  accessBaseUrl: string;
  fetchImpl: FetchLike;
}>;

export type ApiRequestOptions = Partial<{
  method: string;
  body: unknown;
}>;

type ApiErrorPayload = Partial<{
  code: string;
  error: ApiErrorPayload;
  message: string;
}>;

export class ApiClientError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
    this.name = "ApiClientError";
  }
}

export async function authenticatedRequest<T>({
  baseUrl,
  clientOptions,
  path,
  requestOptions = {},
}: {
  baseUrl: string;
  clientOptions: ApiClientOptions;
  path: string;
  requestOptions: ApiRequestOptions;
}): Promise<T> {
  const token = await clientOptions.getAccessToken();
  if (!token) {
    throw new ApiClientError(401, "unauthorized", "missing access token");
  }

  const fetchImpl = clientOptions.fetchImpl ?? defaultFetch;
  let response = await fetchImpl(
    `${baseUrl}${path}`,
    requestParts(token, requestOptions),
  );
  if (response.status === 401) {
    const refreshedToken = await clientOptions.getAccessToken({
      forceRefresh: true,
    });
    if (refreshedToken) {
      response = await fetchImpl(
        `${baseUrl}${path}`,
        requestParts(refreshedToken, requestOptions),
      );
    }
  }
  if (!response.ok) {
    throw await apiError(response);
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

export function defaultFetch(input: RequestInfo | URL, init?: RequestInit) {
  return globalThis.fetch(input, init);
}

export function uploadHeaders(headers: Record<string, string>) {
  const result = new Headers();
  Object.entries(headers).forEach(([name, value]) => result.set(name, value));
  return result;
}

function requestParts(token: string, options: ApiRequestOptions) {
  const headers = new Headers({
    authorization: `Bearer ${token}`,
  });
  let body: string | undefined;
  if (options.body !== undefined) {
    headers.set("content-type", "application/json");
    body = JSON.stringify(options.body);
  }

  return {
    method: options.method ?? "GET",
    headers,
    body,
  };
}

async function apiError(response: Response) {
  let payload: ApiErrorPayload = {};
  try {
    payload = (await response.json()) as ApiErrorPayload;
  } catch {
    payload = {};
  }
  return new ApiClientError(
    response.status,
    payload.error?.code ?? payload.code ?? "api_error",
    payload.error?.message ?? payload.message ?? response.statusText,
  );
}
