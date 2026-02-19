#!/usr/bin/env bash
set -euo pipefail

# -------------------------------
# Export GitHub Code Scanning (CodeQL) alerts to CSV
#
# Requirements:
#   - curl
#   - jq
#
# Usage:
#   export GITHUB_TOKEN="ghp_...."   # or fine-grained token
#   ./export-codeql-alerts.sh OWNER REPO
#
# Optional:
#   STATE=open|fixed|dismissed|all   (default: all)
#   PER_PAGE=100                     (default: 100; max 100)
#
# Output:
#   /tmp/codeql-alerts-OWNER-REPO_YYYYmmdd-HHMMSS.csv
# -------------------------------

OWNER="${1:-}"
REPO="${2:-}"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="${SCRIPT_DIR}/.env"

load_env_file() {
  local file="$1"
  [[ -f "$file" ]] || return 0
  while IFS= read -r line || [[ -n "$line" ]]; do
    # Ignore empty lines and full-line comments.
    [[ -z "$line" || "${line:0:1}" == "#" ]] && continue
    # Accept only KEY=VALUE assignments.
    if [[ "$line" =~ ^([A-Za-z_][A-Za-z0-9_]*)=(.*)$ ]]; then
      local key="${BASH_REMATCH[1]}"
      local val="${BASH_REMATCH[2]}"
      # Trim surrounding single/double quotes when present.
      if [[ "$val" =~ ^\"(.*)\"$ ]]; then
        val="${BASH_REMATCH[1]}"
      elif [[ "$val" =~ ^\'(.*)\'$ ]]; then
        val="${BASH_REMATCH[1]}"
      fi
      # Keep caller environment precedence.
      if [[ -z "${!key-}" ]]; then
        export "${key}=${val}"
      fi
    fi
  done <"$file"
}

# Load .env from script directory if present.
load_env_file "${ENV_FILE}"

if [[ -z "${OWNER}" || -z "${REPO}" ]]; then
  echo "Usage: $0 OWNER REPO"
  exit 1
fi

: "${GITHUB_TOKEN:?You must set GITHUB_TOKEN in your environment}"

STATE="${STATE:-all}"
PER_PAGE="${PER_PAGE:-100}"
TOOL_NAME="${TOOL_NAME:-CodeQL}"

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required. Install it (Ubuntu: sudo apt-get install -y jq)."
  exit 1
fi

if [[ "${PER_PAGE}" -lt 1 || "${PER_PAGE}" -gt 100 ]]; then
  echo "PER_PAGE must be between 1 and 100"
  exit 1
fi

case "${STATE}" in
  open|fixed|dismissed|all) ;;
  *)
    echo "STATE must be one of: open, fixed, dismissed, all"
    exit 1
    ;;
esac

API="https://api.github.com"
BASE_URL="${API}/repos/${OWNER}/${REPO}/code-scanning/alerts"

TMP_JSON="$(mktemp)"
TMP_ALL="$(mktemp)"
TMP_HEADERS="$(mktemp)"

cleanup() {
  rm -f "${TMP_JSON}" "${TMP_ALL}" "${TMP_HEADERS}"
}
trap cleanup EXIT

timestamp="$(date +"%Y%m%d-%H%M%S")"
OUT_CSV="/tmp/codeql-alerts-${OWNER}-${REPO}_${timestamp}.csv"

# CSV header
echo '"number","state","created_at","updated_at","dismissed_at","dismissed_reason","dismissed_comment","rule_id","rule_severity","rule_description","tool_name","tool_guid","ref","commit_sha","path","start_line","end_line","html_url"' > "${OUT_CSV}"

page=1
total=0

echo "Fetching alerts from ${OWNER}/${REPO} (state=${STATE}, per_page=${PER_PAGE})..."

while :; do
  url="${BASE_URL}?per_page=${PER_PAGE}&page=${page}"
  if [[ "${STATE}" != "all" ]]; then
    url="${url}&state=${STATE}"
  fi
  url="${url}&tool_name=${TOOL_NAME}"

  # Fetch one page (headers + body)
  curl -sS \
    -D "${TMP_HEADERS}" \
    -H "Accept: application/vnd.github+json" \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    "${url}" > "${TMP_JSON}"

  # Basic error handling: if API returns an error object, show it and stop.
  if jq -e 'type=="object" and has("message")' "${TMP_JSON}" >/dev/null 2>&1; then
    echo "GitHub API error:"
    jq -r '.message' "${TMP_JSON}"
    echo "Details:"
    jq '.' "${TMP_JSON}"
    exit 1
  fi

  count_on_page="$(jq 'length' "${TMP_JSON}")"
  if [[ "${count_on_page}" -eq 0 ]]; then
    break
  fi

  total=$((total + count_on_page))
  echo "  Page ${page}: ${count_on_page} alerts (total: ${total})"

  # Append page to combined file
  cat "${TMP_JSON}" >> "${TMP_ALL}"
  echo >> "${TMP_ALL}"

  # If fewer than PER_PAGE, we're done
  if [[ "${count_on_page}" -lt "${PER_PAGE}" ]]; then
    break
  fi

  page=$((page + 1))
done

if [[ "${total}" -eq 0 ]]; then
  echo "No alerts found for tool='${TOOL_NAME}' (state='${STATE}')."
  echo "CSV written to: ${OUT_CSV}"
  exit 0
fi

# Convert ALL fetched pages to one big JSON array
# TMP_ALL currently holds multiple JSON arrays separated by newlines.
# This merges them into a single array.
MERGED_JSON="$(mktemp)"
trap 'rm -f "${MERGED_JSON}"; cleanup' EXIT

jq -s 'add' "${TMP_ALL}" > "${MERGED_JSON}"

echo "Converting ${total} alerts to CSV..."

# Build CSV lines
# Notes:
# - most_recent_instance can be null for some alerts; handle defensively.
# - tool can be missing; handle defensively.
# - line fields can be null.
jq -r --arg tool_name "${TOOL_NAME}" '
  def esc: gsub("\""; "\"\"");
  def s(x): (x // "") | tostring;

  .[] | select((.tool.name // "") == $tool_name) |
  [
    s(.number),
    s(.state),
    s(.created_at),
    s(.updated_at),
    s(.dismissed_at),
    s(.dismissed_reason),
    s(.dismissed_comment),
    s(.rule.id),
    s(.rule.severity),
    s(.rule.description),
    s(.tool.name),
    s(.tool.guid),
    s(.most_recent_instance.ref),
    s(.most_recent_instance.commit_sha),
    s(.most_recent_instance.location.path),
    s(.most_recent_instance.location.start_line),
    s(.most_recent_instance.location.end_line),
    s(.html_url)
  ]
  | map(esc)
  | "\"" + join("\",\"") + "\""
' "${MERGED_JSON}" >> "${OUT_CSV}"

echo "Done."
echo "CSV written to: ${OUT_CSV}"
