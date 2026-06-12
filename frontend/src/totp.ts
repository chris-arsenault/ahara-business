export type TotpSetupUriRequest = {
  issuer: string;
  accountName: string;
  secretCode: string;
};

export function buildTotpSetupUri({
  issuer,
  accountName,
  secretCode,
}: TotpSetupUriRequest) {
  const normalizedIssuer = issuer.trim() || "Ahara Mail";
  const normalizedAccountName = accountName.trim() || normalizedIssuer;
  const normalizedSecretCode = secretCode.replace(/\s+/g, "");
  const label = `${encodeURIComponent(normalizedIssuer)}:${encodeURIComponent(normalizedAccountName)}`;
  const query = [
    ["secret", normalizedSecretCode],
    ["issuer", normalizedIssuer],
    ["algorithm", "SHA1"],
    ["digits", "6"],
    ["period", "30"],
  ]
    .map(([key, value]) => `${key}=${encodeURIComponent(value)}`)
    .join("&");

  return `otpauth://totp/${label}?${query}`;
}
