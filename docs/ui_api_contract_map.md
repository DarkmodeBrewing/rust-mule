# UI API Contract Map

This map ties each UI page/controller to the API endpoints and required response fields.
If any required field changes, update UI controllers and tests in the same change.

## Auth + Session

| UI usage | Endpoint | Required fields / behavior |
| --- | --- | --- |
| Auth bootstrap page (`/auth`) | `GET /api/v1/auth/bootstrap` | JSON: `token` (string) |
| Auth bootstrap page (`/auth`) | `POST /api/v1/session` | `200 OK`, sets `rm_session` cookie |
| Sidebar session status | `GET /api/v1/session/check` | `200` when active, `401/403` when expired |
| Sidebar logout | `POST /api/v1/session/logout` | `200`, clears `rm_session` cookie |
| SSE reconnect guard | `GET /api/v1/session/check` | `401/403` triggers redirect to `/auth` |

## Overview (`indexApp`)

| UI section | Endpoint | Required fields |
| --- | --- | --- |
| Runtime status panel | `GET /api/v1/status` | `routing`, `live`, `live_10m`, `pending`, `timeouts` |
| Source diagnostics | `GET /api/v1/status` | `sent_search_source_reqs`, `recv_search_source_reqs`, `sent_publish_source_reqs`, `recv_publish_source_reqs`, `source_store_files`, `source_store_entries_total` |
| Active search threads list | `GET /api/v1/searches` | `searches[]` with `search_id_hex`, `state`, `hits` |
| Search details export | `GET /api/v1/searches/:search_id` | `search`, `hits[]` |
| Stop selected search | `POST /api/v1/searches/:search_id/stop` | `stopped` (bool) |
| Delete selected search | `DELETE /api/v1/searches/:search_id` | `deleted` (bool), `purged_results` (bool) |
| Live status stream | `GET /api/v1/events` (SSE) | `event: status` with full status payload JSON |

## Searches (`appSearch`, `appSearchDetails`)

| UI section | Endpoint | Required fields |
| --- | --- | --- |
| Start keyword search | `POST /api/v1/kad/search_keyword` | `queued`, `keyword_id_hex` |
| Keyword results pane | `GET /api/v1/kad/keyword_results/:keyword_id_hex` | `keyword_id_hex`, `hits[]` with `file_id_hex`, `filename`, `file_size`, `origin` |
| Search thread list | `GET /api/v1/searches` | `searches[]` |
| Search details page | `GET /api/v1/searches/:search_id` | `search`, `hits[]` |

## Nodes / Routing (`appNodeStats`)

| UI section | Endpoint | Required fields |
| --- | --- | --- |
| Status cards + charts | `GET /api/v1/status` | `routing`, `live`, `live_10m`; also time-series counters used by charts |
| Node table | `GET /api/v1/kad/peers` | `peers[]` with `kad_id_hex`, `kad_version`, `verified`, `last_seen_secs_ago`, `last_inbound_secs_ago` |
| Search-derived hit chart | `GET /api/v1/searches` | `searches[]` with `hits` |
| Live status stream | `GET /api/v1/events` | `event: status` payload |

## Logs (`appLogs`)

| UI section | Endpoint | Required fields |
| --- | --- | --- |
| Snapshot status log | `GET /api/v1/status` | Any valid status payload |
| Search thread panel | `GET /api/v1/searches` | `searches[]` |
| Live status log | `GET /api/v1/events` | `event: status` payload |

## Settings (`appSettings`)

| UI section | Endpoint | Required fields |
| --- | --- | --- |
| Settings load | `GET /api/v1/settings` | `settings`, `restart_required`; nested keys under `settings.general`, `settings.sam`, `settings.api` |
| Settings save | `PATCH /api/v1/settings` | Updated `settings`, `restart_required` |
| Rotate token action | `POST /api/v1/token/rotate` | `token` (string), `sessions_cleared` (bool) |
| Session refresh after rotate | `POST /api/v1/session` | `200 OK`, sets `rm_session` cookie |

## Contract Test Coverage

`src/api/mod.rs` includes router-level contract tests for UI-critical endpoints:

- `ui_api_contract_endpoints_return_expected_shapes`

This test asserts response shape invariants for:

- `GET /api/v1/status`
- `GET /api/v1/searches`
- `GET /api/v1/searches/:search_id`
- `GET /api/v1/kad/keyword_results/:keyword_id_hex`
- `GET /api/v1/kad/peers`
- `GET /api/v1/settings`
