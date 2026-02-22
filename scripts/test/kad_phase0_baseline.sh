#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:17835}"
TOKEN_FILE="${TOKEN_FILE:-data/api.token}"
DURATION_SECS="${DURATION_SECS:-1800}"
INTERVAL_SECS="${INTERVAL_SECS:-5}"
OUT_FILE="${OUT_FILE:-/tmp/rust-mule-kad-phase0-$(date +%Y%m%d_%H%M%S).tsv}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base-url)
      BASE_URL="$2"
      shift 2
      ;;
    --token-file)
      TOKEN_FILE="$2"
      shift 2
      ;;
    --duration-secs)
      DURATION_SECS="$2"
      shift 2
      ;;
    --interval-secs)
      INTERVAL_SECS="$2"
      shift 2
      ;;
    --out-file)
      OUT_FILE="$2"
      shift 2
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 2
      ;;
  esac
done

if [[ ! -f "$TOKEN_FILE" ]]; then
  echo "missing token file: $TOKEN_FILE" >&2
  exit 1
fi

TOKEN="$(cat "$TOKEN_FILE")"
mkdir -p "$(dirname "$OUT_FILE")"

echo -e "ts\tuptime_secs\trouting\tlive\tpending\tpending_overdue\tpending_max_overdue_ms\ttracked_out_requests\ttracked_out_matched\ttracked_out_unmatched\ttracked_out_expired\toutbound_shaper_delayed\toutbound_shaper_drop_global_cap\toutbound_shaper_drop_peer_cap\tdropped_legacy_kad1\tdropped_unhandled_opcode\tsent_reqs\trecv_ress\ttimeouts\tsent_reqs_total\trecv_ress_total\ttimeouts_total\ttracked_out_matched_total\ttracked_out_unmatched_total\ttracked_out_expired_total\toutbound_shaper_delayed_total\tdropped_legacy_kad1_total\tdropped_unhandled_opcode_total\tsam_framing_desync_total\trestart_marker\tnew_nodes\tevicted\tsource_search_batch_sent\tsource_search_batch_send_fail\tsource_publish_batch_sent\tsource_publish_batch_send_fail" >"$OUT_FILE"

start_ts="$(date +%s)"
deadline="$((start_ts + DURATION_SECS))"
samples=0
skip_503=0
skip_other=0
prev_uptime=-1
restart_events=0

while [[ "$(date +%s)" -lt "$deadline" ]]; do
  now_iso="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  response="$(
    curl -sS -H "Authorization: Bearer $TOKEN" \
      -w $'\n%{http_code}' \
      "$BASE_URL/api/v1/status" 2>/dev/null || true
  )"
  http_code="${response##*$'\n'}"
  payload="${response%$'\n'*}"
  if [[ "$http_code" == "200" && -n "$payload" ]]; then
    uptime_secs="$(jq -r '(.uptime_secs // 0)' <<<"$payload")"
    restart_marker=0
    if [[ "$prev_uptime" -ge 0 && "$uptime_secs" -lt "$prev_uptime" ]]; then
      restart_marker=1
      restart_events=$((restart_events + 1))
    fi
    prev_uptime="$uptime_secs"
    row="$(
      jq -r --arg ts "$now_iso" --argjson restart_marker "$restart_marker" '[
        $ts,
        (.uptime_secs // 0),
        (.routing // 0),
        (.live // 0),
        (.pending // 0),
        (.pending_overdue // 0),
        (.pending_max_overdue_ms // 0),
        (.tracked_out_requests // 0),
        (.tracked_out_matched // 0),
        (.tracked_out_unmatched // 0),
        (.tracked_out_expired // 0),
        (.outbound_shaper_delayed // 0),
        (.outbound_shaper_drop_global_cap // 0),
        (.outbound_shaper_drop_peer_cap // 0),
        (.dropped_legacy_kad1 // 0),
        (.dropped_unhandled_opcode // 0),
        (.sent_reqs // 0),
        (.recv_ress // 0),
        (.timeouts // 0),
        (.sent_reqs_total // 0),
        (.recv_ress_total // 0),
        (.timeouts_total // 0),
        (.tracked_out_matched_total // 0),
        (.tracked_out_unmatched_total // 0),
        (.tracked_out_expired_total // 0),
        (.outbound_shaper_delayed_total // 0),
        (.dropped_legacy_kad1_total // 0),
        (.dropped_unhandled_opcode_total // 0),
        (.sam_framing_desync_total // 0),
        $restart_marker,
        (.new_nodes // 0),
        (.evicted // 0),
        (.source_search_batch_sent // 0),
        (.source_search_batch_send_fail // 0),
        (.source_publish_batch_sent // 0),
        (.source_publish_batch_send_fail // 0)
      ] | @tsv' <<<"$payload"
    )"
    echo "$row" >>"$OUT_FILE"
    samples=$((samples + 1))
  elif [[ "$http_code" == "503" ]]; then
    skip_503=$((skip_503 + 1))
  else
    skip_other=$((skip_other + 1))
  fi
  sleep "$INTERVAL_SECS"
done

echo "wrote baseline samples: $OUT_FILE (samples=$samples, skipped_503=$skip_503, skipped_other=$skip_other, restarts=$restart_events)"
