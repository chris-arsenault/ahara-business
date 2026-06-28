type RuntimeGlobal<T> = T | undefined;

type RuntimeConfig = Partial<{
  apiBaseUrl: string;
  accessApiBaseUrl: string;
  opsApiBaseUrl: string;
  appBaseUrl: string;
  productName: string;
  cognitoUserPoolId: string;
  cognitoClientId: string;
}>;

declare global {
  interface Window {
    __APP_CONFIG__: RuntimeGlobal<RuntimeConfig>;
  }
}

const runtimeConfig =
  typeof window !== "undefined" ? window.__APP_CONFIG__ : undefined;

const readRuntime = (value?: string) =>
  value && value.trim().length > 0 ? value : undefined;

export const config = {
  apiBaseUrl:
    readRuntime(runtimeConfig?.apiBaseUrl) ??
    import.meta.env.VITE_API_BASE_URL ??
    "",
  accessApiBaseUrl:
    readRuntime(runtimeConfig?.accessApiBaseUrl) ??
    import.meta.env.VITE_ACCESS_API_BASE_URL ??
    "",
  opsApiBaseUrl:
    readRuntime(runtimeConfig?.opsApiBaseUrl) ??
    import.meta.env.VITE_OPS_API_BASE_URL ??
    "",
  appBaseUrl:
    readRuntime(runtimeConfig?.appBaseUrl) ??
    import.meta.env.VITE_APP_BASE_URL ??
    "",
  productName:
    readRuntime(runtimeConfig?.productName) ??
    import.meta.env.VITE_PRODUCT_NAME ??
    "Ahara Mail",
  cognitoUserPoolId:
    readRuntime(runtimeConfig?.cognitoUserPoolId) ??
    import.meta.env.VITE_COGNITO_USER_POOL_ID ??
    "",
  cognitoClientId:
    readRuntime(runtimeConfig?.cognitoClientId) ??
    import.meta.env.VITE_COGNITO_CLIENT_ID ??
    "",
};
