#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPARE_SCRIPT="$ROOT_DIR/scripts/test/kad_phase0_compare.sh"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

before_tsv="$tmp_dir/before.tsv"
after_tsv="$tmp_dir/after.tsv"
compare_tsv="$tmp_dir/compare.tsv"

cat >"$before_tsv" <<'EOF'
ts	uptime_secs	routing	live	pending	pending_overdue	pending_max_overdue_ms	tracked_out_requests	tracked_out_matched	tracked_out_unmatched	tracked_out_expired	outbound_shaper_delayed	outbound_shaper_drop_global_cap	outbound_shaper_drop_peer_cap	sent_reqs	recv_ress	timeouts	new_nodes	evicted	source_search_batch_sent	source_search_batch_send_fail	source_publish_batch_sent	source_publish_batch_send_fail
2026-02-20T00:00:00Z	100	163	2	20	2	2000	140	5	0	40	0	0	0	50	5	20	0	0	0	0	0	0
2026-02-20T00:00:05Z	105	163	2	19	1	1800	141	6	0	41	0	0	0	49	6	21	0	0	0	0	0	0
EOF

cat >"$after_tsv" <<'EOF'
ts	uptime_secs	routing	live	pending	pending_overdue	pending_max_overdue_ms	tracked_out_requests	tracked_out_matched	tracked_out_unmatched	tracked_out_expired	outbound_shaper_delayed	outbound_shaper_drop_global_cap	outbound_shaper_drop_peer_cap	sent_reqs	recv_ress	timeouts	new_nodes	evicted	source_search_batch_sent	source_search_batch_send_fail	source_publish_batch_sent	source_publish_batch_send_fail
2026-02-20T00:00:00Z	100	163	2	19	1	200	145	7	0	42	70	0	0	51	8	19	0	0	0	0	0	0
2026-02-20T00:00:05Z	105	163	2	19	1	180	146	8	0	43	74	0	0	51	9	19	0	0	0	0	0	0
EOF

bash "$COMPARE_SCRIPT" --before "$before_tsv" --after "$after_tsv" >"$compare_tsv"

awk -F '\t' '
  NR == 1 { next }
  $1 == "outbound_shaper_delayed" {
    seen = 1;
    if (($3 + 0) <= 0) {
      print "expected outbound_shaper_delayed after_avg > 0" > "/dev/stderr";
      exit 1;
    }
  }
  END {
    if (!seen) {
      print "missing outbound_shaper_delayed in compare output" > "/dev/stderr";
      exit 1;
    }
  }
' "$compare_tsv"

awk -F '\t' '
  NR == 1 { next }
  $1 == "outbound_shaper_drop_global_cap" || $1 == "outbound_shaper_drop_peer_cap" {
    seen++;
    if (($3 + 0) != 0) {
      print "expected shaper cap drops after_avg == 0 for " $1 > "/dev/stderr";
      exit 1;
    }
  }
  END {
    if (seen != 2) {
      print "missing shaper cap drop metrics in compare output" > "/dev/stderr";
      exit 1;
    }
  }
' "$compare_tsv"

awk -F '\t' '
  NR == 1 { next }
  $1 == "pending_max_overdue_ms" {
    seen = 1;
    if (($4 + 0) >= 0) {
      print "expected pending_max_overdue_ms delta < 0" > "/dev/stderr";
      exit 1;
    }
  }
  END {
    if (!seen) {
      print "missing pending_max_overdue_ms in compare output" > "/dev/stderr";
      exit 1;
    }
  }
' "$compare_tsv"

echo "kad_phase0_ci_smoke: ok"
