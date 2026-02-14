#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/docs/debug_probe_peer.sh [options]

Calls:
  POST /api/v1/debug/probe_peer

Required:
  --udp-dest-b64 STR     Peer UDP destination (base64, from /api/v1/kad/peers)
  --keyword-id-hex HEX   16-byte hex KadID (32 hex chars)
  --file-id-hex HEX      16-byte hex KadID (32 hex chars)
  --filename NAME        File name for publish
  --file-size BYTES      File size (integer)

Optional:
  --file-type STR        File type tag (e.g. "Pro")
  --base-url URL         Default: http://127.0.0.1:17835
  --token TOKEN          Bearer token (overrides --token-file)
  --token-file PATH      Default: data/api.token
USAGE
}

BASE_URL="http://127.0.0.1:17835"
TOKEN_FILE="data/api.token"
TOKEN=""

UDP_DEST_B64=""
KEYWORD_ID_HEX=""
FILE_ID_HEX=""
FILENAME=""
FILE_SIZE=""
FILE_TYPE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --udp-dest-b64) UDP_DEST_B64="$2"; shift 2 ;;
    --keyword-id-hex) KEYWORD_ID_HEX="$2"; shift 2 ;;
    --file-id-hex) FILE_ID_HEX="$2"; shift 2 ;;
    --filename) FILENAME="$2"; shift 2 ;;
    --file-size) FILE_SIZE="$2"; shift 2 ;;
    --file-type) FILE_TYPE="$2"; shift 2 ;;
    --base-url) BASE_URL="$2"; shift 2 ;;
    --token) TOKEN="$2"; shift 2 ;;
    --token-file) TOKEN_FILE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$UDP_DEST_B64" || -z "$KEYWORD_ID_HEX" || -z "$FILE_ID_HEX" || -z "$FILENAME" || -z "$FILE_SIZE" ]]; then
  echo "Missing required args." >&2
  usage
  exit 2
fi

if [[ -z "$TOKEN" ]]; then
  TOKEN="$(cat "$TOKEN_FILE")"
fi

if [[ -n "$FILE_TYPE" ]]; then
  BODY="{\"udp_dest_b64\":\"$UDP_DEST_B64\",\"keyword_id_hex\":\"$KEYWORD_ID_HEX\",\"file_id_hex\":\"$FILE_ID_HEX\",\"filename\":\"$FILENAME\",\"file_size\":$FILE_SIZE,\"file_type\":\"$FILE_TYPE\"}"
else
  BODY="{\"udp_dest_b64\":\"$UDP_DEST_B64\",\"keyword_id_hex\":\"$KEYWORD_ID_HEX\",\"file_id_hex\":\"$FILE_ID_HEX\",\"filename\":\"$FILENAME\",\"file_size\":$FILE_SIZE}"
fi

curl -sS \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "$BODY" \
  "$BASE_URL/api/v1/debug/probe_peer"
