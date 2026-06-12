#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINE_FILE="${ROOT_DIR}/scripts/file-length-baseline.txt"
LIMIT=400

declare -A baseline_limits=()

while read -r path max_lines; do
  if [[ -z "${path:-}" || "${path:0:1}" == "#" ]]; then
    continue
  fi

  baseline_limits["${path}"]="${max_lines}"
done < "${BASELINE_FILE}"

failed=0

while IFS= read -r -d '' file_path; do
  relative_path="${file_path#${ROOT_DIR}/}"
  line_count="$(wc -l < "${file_path}" | tr -d ' ')"
  allowed_lines="${baseline_limits[${relative_path}]:-${LIMIT}}"

  if (( line_count > allowed_lines )); then
    printf '%s has %s lines; limit is %s\n' "${relative_path}" "${line_count}" "${allowed_lines}" >&2
    failed=1
  fi
done < <(
  find "${ROOT_DIR}/backend" "${ROOT_DIR}/frontend/src" \
    -path '*/target*' -prune -o \
    \( -name '*.rs' -o -name '*.ts' -o -name '*.tsx' \) \
    -type f -print0
)

if (( failed != 0 )); then
  printf 'File length check failed. New Rust/TypeScript files must stay at or below %s lines; baseline files may not grow.\n' "${LIMIT}" >&2
  exit 1
fi
