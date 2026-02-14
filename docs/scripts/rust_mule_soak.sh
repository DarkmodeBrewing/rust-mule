#!/usr/bin/env bash
set -euo pipefail

# Two-instance soak harness.
# Default expects freshly built artifacts at ../../mule-a and ../../mule-b.
#
# Examples:
#   docs/scripts/rust_mule_soak.sh start
#   docs/scripts/rust_mule_soak.sh wait_ready
#   docs/scripts/rust_mule_soak.sh soak 2000
#   docs/scripts/rust_mule_soak.sh stop
#   docs/scripts/rust_mule_soak.sh collect
#
# Run in background:
#   nohup bash -lc 'cd /path/to/rust-mule && docs/scripts/rust_mule_soak.sh start && docs/scripts/rust_mule_soak.sh wait_ready && docs/scripts/rust_mule_soak.sh soak 2000' >/tmp/rust-mule-soak/runner.out 2>&1 &

ROOT="${ROOT:-$PWD}"
A_SRC="${A_SRC:-$ROOT/../../mule-a}"
B_SRC="${B_SRC:-$ROOT/../../mule-b}"
RUN_ROOT="${RUN_ROOT:-/tmp/rust-mule-soak}"
A_DIR="$RUN_ROOT/mule-a"
B_DIR="$RUN_ROOT/mule-b"
LOG_DIR="$RUN_ROOT/logs"

A_URL="${A_URL:-http://127.0.0.1:17835}"
B_URL="${B_URL:-http://127.0.0.1:17836}"

mkdir -p "$RUN_ROOT" "$LOG_DIR"

ts() { date +"%Y-%m-%dT%H:%M:%S%z"; }
rand_hex16() { hexdump -n 16 -e '16/1 "%02x"' /dev/urandom; }

start_nodes() {
  rm -rf "$A_DIR" "$B_DIR"
  cp -a "$A_SRC" "$A_DIR"
  cp -a "$B_SRC" "$B_DIR"

  sed -i 's/session_name = "rust-mule-b"/session_name = "rust-mule-b-soak"/' "$B_DIR/config.toml" || true
  sed -i 's/forward_port = 40000/forward_port = 40001/' "$B_DIR/config.toml" || true
  sed -i 's/udp_port = 4665/udp_port = 4666/' "$B_DIR/config.toml" || true
  sed -i 's/port = 17835/port = 17836/' "$B_DIR/config.toml" || true

  rm -f "$A_DIR/data/rust-mule.lock" "$B_DIR/data/rust-mule.lock"
  mkdir -p "$A_DIR/data/logs" "$B_DIR/data/logs"

  (cd "$A_DIR" && nohup ./rust-mule >"$LOG_DIR/a.out" 2>&1 & echo $! >"$LOG_DIR/a.pid")
  (cd "$B_DIR" && nohup ./rust-mule >"$LOG_DIR/b.out" 2>&1 & echo $! >"$LOG_DIR/b.pid")

  echo "$(ts) started A pid=$(cat "$LOG_DIR/a.pid") B pid=$(cat "$LOG_DIR/b.pid")"
}

wait_ready() {
  for _ in $(seq 1 240); do
    local ta tb a_code b_code
    ta="$(cat "$A_DIR/data/api.token" 2>/dev/null || true)"
    tb="$(cat "$B_DIR/data/api.token" 2>/dev/null || true)"
    a_code="$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $ta" "$A_URL/api/v1/status" || true)"
    b_code="$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $tb" "$B_URL/api/v1/status" || true)"
    echo "$(ts) ready-check A=$a_code B=$b_code" | tee -a "$LOG_DIR/soak.log"
    if [[ "$a_code" == "200" && "$b_code" == "200" ]]; then
      return 0
    fi
    sleep 5
  done
  echo "$(ts) ERROR: status did not reach 200/200" | tee -a "$LOG_DIR/soak.log"
  return 1
}

soak() {
  local rounds wait_publish wait_search wait_between ta tb
  rounds="${1:-500}"
  wait_publish="${WAIT_PUBLISH:-20}"
  wait_search="${WAIT_SEARCH:-20}"
  wait_between="${WAIT_BETWEEN:-5}"

  ta="$(cat "$A_DIR/data/api.token")"
  tb="$(cat "$B_DIR/data/api.token")"

  echo "$(ts) soak-start rounds=$rounds" | tee -a "$LOG_DIR/soak.log"

  for i in $(seq 1 "$rounds"); do
    local file_id size a_pub b_srch b_get a_st b_st
    file_id="$(rand_hex16)"
    size="$(( (RANDOM % 5000000) + 1024 ))"

    echo "$(ts) round=$i file=$file_id size=$size" | tee -a "$LOG_DIR/soak.log"

    a_pub="$(curl -sS -H "Authorization: Bearer $ta" -H 'content-type: application/json' \
      -d "{\"file_id_hex\":\"$file_id\",\"file_size\":$size}" \
      "$A_URL/api/v1/kad/publish_source")"

    sleep "$wait_publish"

    b_srch="$(curl -sS -H "Authorization: Bearer $tb" -H 'content-type: application/json' \
      -d "{\"file_id_hex\":\"$file_id\",\"file_size\":$size}" \
      "$B_URL/api/v1/kad/search_sources")"

    sleep "$wait_search"

    b_get="$(curl -sS -H "Authorization: Bearer $tb" "$B_URL/api/v1/kad/sources/$file_id")"
    a_st="$(curl -sS -H "Authorization: Bearer $ta" "$A_URL/api/v1/status")"
    b_st="$(curl -sS -H "Authorization: Bearer $tb" "$B_URL/api/v1/status")"

    printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$(ts)" "$i" "$file_id" "$a_pub" "$b_srch" "$b_get" >> "$LOG_DIR/rounds.tsv"

    printf '%s\tA\t%s\n' "$(ts)" "$a_st" >> "$LOG_DIR/status.ndjson"
    printf '%s\tB\t%s\n' "$(ts)" "$b_st" >> "$LOG_DIR/status.ndjson"

    sleep "$wait_between"
  done

  echo "$(ts) soak-finished rounds=$rounds" | tee -a "$LOG_DIR/soak.log"
}

stop_nodes() {
  [[ -f "$LOG_DIR/a.pid" ]] && kill "$(cat "$LOG_DIR/a.pid")" 2>/dev/null || true
  [[ -f "$LOG_DIR/b.pid" ]] && kill "$(cat "$LOG_DIR/b.pid")" 2>/dev/null || true
  echo "$(ts) stop requested" | tee -a "$LOG_DIR/soak.log"
}

collect() {
  local bundle
  bundle="/tmp/rust-mule-soak-$(date +%Y%m%d_%H%M%S).tar.gz"
  tar -czf "$bundle" -C "$RUN_ROOT" .
  echo "$bundle"
}

case "${1:-}" in
  start) start_nodes ;;
  wait_ready) wait_ready ;;
  soak) soak "${2:-500}" ;;
  stop) stop_nodes ;;
  collect) collect ;;
  *) echo "usage: $0 {start|wait_ready|soak [rounds]|stop|collect}" ; exit 2 ;;
esac
