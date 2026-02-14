# API Curl Cheat Sheet

This file collects `curl` commands for testing the local rust-mule HTTP API.

For “one script per endpoint” wrappers, see `scripts/docs/`.

Assumptions:

- rust-mule is running
- API token exists at `data/api.token`

## Setup

```bash
BASE_URL="http://127.0.0.1:17835"
TOKEN="$(cat data/api.token)"
AUTH=(-H "Authorization: Bearer $TOKEN")
JSON=(-H "Content-Type: application/json")
```

## Auth Bootstrap Token (Loopback Only)

Requires `[api].auth_mode = "local_ui"`.

```bash
curl -sS "$BASE_URL/api/v1/auth/bootstrap" | jq .
```

## Create Frontend Session Cookie

```bash
curl -i -sS -X POST "${AUTH[@]}" "$BASE_URL/api/v1/session"
```

For subsequent commands that require session auth (SSE/UI), use:

```bash
COOKIE=(--cookie "rm_session=<session-id>")
```

## Rotate API Bearer Token

```bash
curl -sS -X POST "${AUTH[@]}" "$BASE_URL/api/v1/token/rotate" | jq .
```

## Session Check (Cookie Auth)

```bash
curl -sS "${COOKIE[@]}" "$BASE_URL/api/v1/session/check" | jq .
```

## Session Logout (Cookie Auth)

```bash
curl -sS -X POST "${COOKIE[@]}" "$BASE_URL/api/v1/session/logout" | jq .
```

## Health

```bash
curl -sS "$BASE_URL/api/v1/health" | jq .
```

## Status Snapshot

```bash
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/status" | jq .
```

## Settings Snapshot

```bash
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/settings" | jq .
```

## Update Settings

```bash
curl -sS -X PATCH "${AUTH[@]}" "${JSON[@]}" \
  -d '{"general":{"log_level":"info","log_to_file":true,"log_file_level":"debug","auto_open_ui":true},"sam":{"host":"127.0.0.1","port":7656,"session_name":"rust-mule"},"api":{"port":17835}}' \
  "$BASE_URL/api/v1/settings" | jq .
```

## Active Keyword Searches

```bash
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/searches" | jq .
```

## Downloads: List Queue

```bash
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/downloads" | jq .
```

## Downloads: Create Queue Entry

```bash
curl -sS -X POST "${AUTH[@]}" "${JSON[@]}" \
  -d '{"file_name":"movie.iso","file_size":1234,"file_hash_md4_hex":"0123456789abcdef0123456789abcdef"}' \
  "$BASE_URL/api/v1/downloads" | jq .
```

## Downloads: Pause/Resume/Cancel

```bash
PART=1
curl -sS -X POST "${AUTH[@]}" "${JSON[@]}" -d '{}' "$BASE_URL/api/v1/downloads/$PART/pause" | jq .
curl -sS -X POST "${AUTH[@]}" "${JSON[@]}" -d '{}' "$BASE_URL/api/v1/downloads/$PART/resume" | jq .
curl -sS -X POST "${AUTH[@]}" "${JSON[@]}" -d '{}' "$BASE_URL/api/v1/downloads/$PART/cancel" | jq .
```

## Downloads: Delete

```bash
PART=1
curl -sS -X DELETE "${AUTH[@]}" "$BASE_URL/api/v1/downloads/$PART" | jq .
```

## Active Keyword Search Details

`search_id` is currently the keyword ID hex (16 bytes / 32 hex chars).

```bash
SEARCH_ID="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/searches/$SEARCH_ID" | jq .
```

## Stop Active Keyword Search

```bash
SEARCH_ID="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d '{}' \
  "$BASE_URL/api/v1/searches/$SEARCH_ID/stop" | jq .
```

## Delete Active Keyword Search

```bash
SEARCH_ID="00112233445566778899aabbccddeeff"
curl -sS -X DELETE "${AUTH[@]}" \
  "$BASE_URL/api/v1/searches/$SEARCH_ID?purge_results=true" | jq .
```

## Live Events (SSE)

```bash
curl -N -sS "${COOKIE[@]}" "$BASE_URL/api/v1/events"
```

## KAD: Search Sources For FileID

`file_id_hex` is 16 bytes / 32 hex chars.

```bash
FILE_ID_HEX="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"file_id_hex\":\"$FILE_ID_HEX\",\"file_size\":0}" \
  "$BASE_URL/api/v1/kad/search_sources" | jq .
```

## KAD: Publish This Node As A Source

```bash
FILE_ID_HEX="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"file_id_hex\":\"$FILE_ID_HEX\",\"file_size\":0}" \
  "$BASE_URL/api/v1/kad/publish_source" | jq .
```

## KAD: Read Sources Learned So Far (In-Memory)

```bash
FILE_ID_HEX="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/kad/sources/$FILE_ID_HEX" | jq .
```

## KAD: Keyword Search (Discover File IDs)

This does a Kad2 keyword search using iMule-compatible keyword hashing (first extracted word).

```bash
QUERY="ubuntu iso"
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"query\":\"$QUERY\"}" \
  "$BASE_URL/api/v1/kad/search_keyword" | jq .
```

Or specify a keyword hash directly:

```bash
KEYWORD_ID_HEX="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"keyword_id_hex\":\"$KEYWORD_ID_HEX\"}" \
  "$BASE_URL/api/v1/kad/search_keyword" | jq .
```

## KAD: Read Keyword Hits Learned So Far (In-Memory)

```bash
KEYWORD_ID_HEX="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/kad/keyword_results/$KEYWORD_ID_HEX" | jq .
```

## KAD: List Known Peers (Routing Snapshot)

```bash
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/kad/peers" | jq .
```

## Debug: Routing Summary

```bash
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/debug/routing/summary" | jq .
```

## Debug: Routing Buckets

```bash
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/debug/routing/buckets" | jq .
```

## Debug: Routing Nodes (Per Bucket)

```bash
BUCKET=0
curl -sS "${AUTH[@]}" "$BASE_URL/api/v1/debug/routing/nodes?bucket=$BUCKET" | jq .
```

## Debug: Trigger One Lookup

```bash
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{}" \
  "$BASE_URL/api/v1/debug/lookup_once" | jq .
```

Or provide a target KadID:

```bash
TARGET_ID_HEX="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"target_id_hex\":\"$TARGET_ID_HEX\"}" \
  "$BASE_URL/api/v1/debug/lookup_once" | jq .
```

## Debug: Probe A Specific Peer (HELLO + SEARCH + PUBLISH)

Use `/api/v1/kad/peers` to find a `udp_dest_b64` for a known peer.

```bash
UDP_DEST_B64="AAA...AAAA"
KEYWORD_ID_HEX="00112233445566778899aabbccddeeff"
FILE_ID_HEX="ffeeddccbbaa99887766554433221100"
FILENAME="probe.bin"
FILE_SIZE=123

curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"udp_dest_b64\":\"$UDP_DEST_B64\",\"keyword_id_hex\":\"$KEYWORD_ID_HEX\",\"file_id_hex\":\"$FILE_ID_HEX\",\"filename\":\"$FILENAME\",\"file_size\":$FILE_SIZE}" \
  "$BASE_URL/api/v1/debug/probe_peer" | jq .
```

Optional file type:

```bash
FILE_TYPE="Pro"
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"udp_dest_b64\":\"$UDP_DEST_B64\",\"keyword_id_hex\":\"$KEYWORD_ID_HEX\",\"file_id_hex\":\"$FILE_ID_HEX\",\"filename\":\"$FILENAME\",\"file_size\":$FILE_SIZE,\"file_type\":\"$FILE_TYPE\"}" \
  "$BASE_URL/api/v1/debug/probe_peer" | jq .
```

## KAD: Publish A Keyword->File Entry (DHT)

This enqueues a Kad2 `PUBLISH_KEY_REQ` to a couple closest peers. It uses iMule-style keyword
hashing (first extracted word from `query`).

```bash
QUERY="ubuntu iso"
FILE_ID_HEX="00112233445566778899aabbccddeeff"
FILENAME="ubuntu-24.04.iso"
FILE_SIZE=123
FILE_TYPE="Pro"

curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"query\":\"$QUERY\",\"file_id_hex\":\"$FILE_ID_HEX\",\"filename\":\"$FILENAME\",\"file_size\":$FILE_SIZE,\"file_type\":\"$FILE_TYPE\"}" \
  "$BASE_URL/api/v1/kad/publish_keyword" | jq .
```

Or specify a keyword hash directly:

```bash
KEYWORD_ID_HEX="00112233445566778899aabbccddeeff"
FILE_ID_HEX="00112233445566778899aabbccddeeff"
FILENAME="ubuntu-24.04.iso"
FILE_SIZE=123

curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"keyword_id_hex\":\"$KEYWORD_ID_HEX\",\"file_id_hex\":\"$FILE_ID_HEX\",\"filename\":\"$FILENAME\",\"file_size\":$FILE_SIZE}" \
  "$BASE_URL/api/v1/kad/publish_keyword" | jq .
```
