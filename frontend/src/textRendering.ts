export function sanitizeAttachmentName(filename: string) {
  const withoutControls = [...filename]
    .filter((char) => !isControl(char))
    .join("");
  const normalized = withoutControls.replaceAll("\\", "/");
  const basename =
    normalized
      .split("/")
      .reverse()
      .find((part) => part.trim().length > 0)
      ?.trim() ?? "";
  const safe = [...basename]
    .filter((char) => char !== "/" && char !== "\\")
    .slice(0, 240)
    .join("");

  return safe && safe !== "." && safe !== ".." ? safe : "attachment";
}

export function formatBytes(sizeBytes: number | null) {
  if (sizeBytes === null) {
    return "unknown size";
  }
  if (sizeBytes < 1024) {
    return `${sizeBytes} B`;
  }
  if (sizeBytes < 1024 * 1024) {
    return `${(sizeBytes / 1024).toFixed(1)} KB`;
  }
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

function isControl(char: string) {
  const code = char.charCodeAt(0);
  return code <= 0x1f || code === 0x7f;
}
