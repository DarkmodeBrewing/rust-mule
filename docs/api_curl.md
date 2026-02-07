# API Curl Cheat Sheet

This file collects `curl` commands for testing the local rust-mule HTTP API.

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

