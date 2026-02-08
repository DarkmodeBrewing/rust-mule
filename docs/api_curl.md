# API Curl Cheat Sheet

This file collects `curl` commands for testing the local rust-mule HTTP API.

For “one script per endpoint” wrappers, see `docs/scripts/`.

Assumptions:

- `config.toml` has `[api].enabled=true`
- rust-mule is running
- API token exists at `data/api.token`

## Setup

```bash
BASE_URL="http://127.0.0.1:17835"
TOKEN="$(cat data/api.token)"
AUTH=(-H "Authorization: Bearer $TOKEN")
JSON=(-H "Content-Type: application/json")
```

## Health

```bash
curl -sS "$BASE_URL/health" | jq .
```

## Status Snapshot

```bash
curl -sS "${AUTH[@]}" "$BASE_URL/status" | jq .
```

## Live Events (SSE)

```bash
curl -N -sS "${AUTH[@]}" "$BASE_URL/events"
```

## KAD: Search Sources For FileID

`file_id_hex` is 16 bytes / 32 hex chars.

```bash
FILE_ID_HEX="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"file_id_hex\":\"$FILE_ID_HEX\",\"file_size\":0}" \
  "$BASE_URL/kad/search_sources" | jq .
```

## KAD: Publish This Node As A Source

```bash
FILE_ID_HEX="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"file_id_hex\":\"$FILE_ID_HEX\",\"file_size\":0}" \
  "$BASE_URL/kad/publish_source" | jq .
```

## KAD: Read Sources Learned So Far (In-Memory)

```bash
FILE_ID_HEX="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "$BASE_URL/kad/sources/$FILE_ID_HEX" | jq .
```

## KAD: Keyword Search (Discover File IDs)

This does a Kad2 keyword search using iMule-compatible keyword hashing (first extracted word).

```bash
QUERY="ubuntu iso"
curl -sS "${AUTH[@]}" "${JSON[@]}" \
  -d "{\"query\":\"$QUERY\"}" \
  "$BASE_URL/kad/search_keyword" | jq .
```

## KAD: Read Keyword Hits Learned So Far (In-Memory)

```bash
KEYWORD_ID_HEX="00112233445566778899aabbccddeeff"
curl -sS "${AUTH[@]}" "$BASE_URL/kad/keyword_results/$KEYWORD_ID_HEX" | jq .
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
  "$BASE_URL/kad/publish_keyword" | jq .
```
