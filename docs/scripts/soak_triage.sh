#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  docs/scripts/soak_triage.sh /path/to/rust-mule-soak-YYYYmmdd_HHMMSS.tar.gz

Outputs:
  - run completion signal (stop requested vs abrupt ending)
  - round success metrics and failure streaks
  - source concentration in successful rounds
  - key per-node (A/B) status counters from status.ndjson
  - panic/fatal signature scan in captured stdout logs
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -ne 1 ]]; then
  usage
  exit 1
fi

archive="$1"
if [[ ! -f "$archive" ]]; then
  echo "error: archive not found: $archive" >&2
  exit 1
fi

tmpdir="$(mktemp -d /tmp/rust-mule-soak-triage-XXXXXX)"
trap 'rm -rf "$tmpdir"' EXIT

tar -xzf "$archive" -C "$tmpdir"

log_dir="$tmpdir/logs"
rounds_tsv="$log_dir/rounds.tsv"
status_ndjson="$log_dir/status.ndjson"
runner_out="$tmpdir/runner.out"
soak_log="$log_dir/soak.log"
a_out="$log_dir/a.out"
b_out="$log_dir/b.out"

if [[ ! -f "$rounds_tsv" ]]; then
  echo "error: rounds file missing: $rounds_tsv" >&2
  exit 1
fi

echo "== Soak Triage =="
echo "archive: $archive"
echo

echo "-- Completion Signal --"
if [[ -f "$soak_log" ]] && grep -q "stop requested" "$soak_log"; then
  grep "stop requested" "$soak_log" | tail -n 1
else
  echo "stop marker: not found in soak.log"
fi
if [[ -f "$runner_out" ]]; then
  grep -E "started|stop requested|failed|error|panic|killed|terminated" "$runner_out" | tail -n 12 || true
fi
echo

echo "-- Round Outcomes --"
awk -F '\t' '
BEGIN{
  total=0; ok=0; first_ok_round=0; first_ok_ts="";
  last_ok_round=0; last_ok_ts=""; cur_fail=0; max_fail=0
}
{
  total++
  success=($6 ~ /"sources":\[\{/)
  if (success) {
    ok++
    if (first_ok_round==0) {
      first_ok_round=$2
      first_ok_ts=$1
    }
    last_ok_round=$2
    last_ok_ts=$1
    if (cur_fail>max_fail) max_fail=cur_fail
    cur_fail=0
  } else {
    cur_fail++
  }
}
END{
  if (cur_fail>max_fail) max_fail=cur_fail
  pct=(total>0)?(ok*100.0/total):0
  printf("total_rounds=%d\nsuccess_rounds=%d\nsuccess_pct=%.2f\n", total, ok, pct)
  if (first_ok_round>0) {
    printf("first_success_round=%d first_success_ts=%s\n", first_ok_round, first_ok_ts)
    printf("last_success_round=%d last_success_ts=%s\n", last_ok_round, last_ok_ts)
  } else {
    printf("first_success_round=none\nlast_success_round=none\n")
  }
  printf("max_consecutive_failures=%d\n", max_fail)
}
' "$rounds_tsv"
tail -n 300 "$rounds_tsv" | awk -F '\t' '
BEGIN{total=0; ok=0}
{total++; if($6 ~ /"sources":\[\{/) ok++}
END{
  pct=(total>0)?(ok*100.0/total):0
  printf("last300_success=%d/%d (%.2f%%)\n", ok, total, pct)
}'
echo

echo "-- Success Source Concentration --"
awk -F '\t' '$6 ~ /"sources":\[\{/ {print $3}' "$rounds_tsv" | sort | uniq | wc -l | awk '{print "unique_success_file_ids="$1}'
awk -F '\t' '
$6 ~ /"sources":\[\{/ {
  if (match($6, /"source_id_hex":"[0-9a-f]+"/)) {
    s=substr($6, RSTART, RLENGTH)
    gsub(/"source_id_hex":"|"/, "", s)
    print s
  }
}
' "$rounds_tsv" | sort | uniq -c | sort -nr | head -n 10 | sed 's/^ *//'
echo

if [[ -f "$status_ndjson" ]]; then
  echo "-- Status Counter Summary (max,last) --"
  awk -F '\t' '
function extract(line,key,    pat,v) {
  pat="\"" key "\":[0-9]+"
  if (match(line, pat)) {
    v=substr(line, RSTART, RLENGTH)
    sub("\"" key "\":", "", v)
    return v+0
  }
  return -1
}
{
  node=$2
  j=$3
  keys[1]="live"
  keys[2]="live_10m"
  keys[3]="routing"
  keys[4]="pending"
  keys[5]="timeouts"
  keys[6]="dropped_undecipherable"
  keys[7]="dropped_unparsable"
  keys[8]="source_store_files"
  keys[9]="source_store_entries_total"
  keys[10]="recv_search_source_reqs"
  keys[11]="recv_publish_source_reqs"
  keys[12]="source_search_hits"
  keys[13]="source_search_misses"
  keys[14]="source_search_results_served"
  keys[15]="source_probe_first_publish_responses"
  keys[16]="source_probe_first_search_responses"
  keys[17]="source_probe_search_results_total"
  keys[18]="source_probe_publish_latency_ms_total"
  keys[19]="source_probe_search_latency_ms_total"
  for (i=1; i<=19; i++) {
    k=keys[i]
    v=extract(j, k)
    if (v >= 0) {
      if (!((node SUBSEP k) in maxv) || v > maxv[node,k]) maxv[node,k]=v
      lastv[node,k]=v
    }
  }
}
END{
  nodes[1]="A"
  nodes[2]="B"
  for (ni=1; ni<=2; ni++) {
    n=nodes[ni]
    print "node=" n
    for (i=1; i<=19; i++) {
      k=keys[i]
      printf("  %s max=%d last=%d\n", k, maxv[n,k]+0, lastv[n,k]+0)
    }
  }
}
' "$status_ndjson"
  echo
fi

echo "-- Panic/Fatal Scan --"
for f in "$a_out" "$b_out"; do
  if [[ -f "$f" ]]; then
    label="$(basename "$f")"
    count="$(grep -Eic "panicked|panic!|thread .* panicked|fatal error|segmentation" "$f" || true)"
    echo "$label panic_fatal_matches=$count"
  fi
done
