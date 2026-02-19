#!/usr/bin/env bash
set -euo pipefail

BEFORE_FILE=""
AFTER_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --before)
      BEFORE_FILE="$2"
      shift 2
      ;;
    --after)
      AFTER_FILE="$2"
      shift 2
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$BEFORE_FILE" || -z "$AFTER_FILE" ]]; then
  echo "usage: $0 --before <baseline_before.tsv> --after <baseline_after.tsv>" >&2
  exit 2
fi

if [[ ! -f "$BEFORE_FILE" ]]; then
  echo "missing before file: $BEFORE_FILE" >&2
  exit 1
fi
if [[ ! -f "$AFTER_FILE" ]]; then
  echo "missing after file: $AFTER_FILE" >&2
  exit 1
fi

tmp_before="$(mktemp)"
tmp_after="$(mktemp)"
trap 'rm -f "$tmp_before" "$tmp_after"' EXIT

summarize() {
  local in_file="$1"
  local out_file="$2"
  awk -F '\t' '
    NR == 1 {
      for (i = 1; i <= NF; i++) {
        h[i] = $i;
      }
      next;
    }
    {
      for (i = 2; i <= NF; i++) {
        key = h[i];
        val = $i + 0;
        n[key] += 1;
        s[key] += val;
        if (!(key in min) || val < min[key]) {
          min[key] = val;
        }
        if (!(key in max) || val > max[key]) {
          max[key] = val;
        }
      }
    }
    END {
      for (k in n) {
        printf "%s\t%.6f\t%.6f\t%.6f\t%d\n", k, s[k] / n[k], min[k], max[k], n[k];
      }
    }
  ' "$in_file" | sort >"$out_file"
}

summarize "$BEFORE_FILE" "$tmp_before"
summarize "$AFTER_FILE" "$tmp_after"

awk -F '\t' '
  BEGIN {
    OFS = "\t";
    print "metric", "before_avg", "after_avg", "delta", "pct_change", "before_min", "before_max", "after_min", "after_max", "samples_before", "samples_after";
  }
  FNR == NR {
    before_avg[$1] = $2;
    before_min[$1] = $3;
    before_max[$1] = $4;
    before_n[$1] = $5;
    seen[$1] = 1;
    next;
  }
  {
    m = $1;
    after_avg[m] = $2;
    after_min[m] = $3;
    after_max[m] = $4;
    after_n[m] = $5;
    seen[m] = 1;
  }
  END {
    for (m in seen) {
      ba = (m in before_avg) ? before_avg[m] : 0;
      aa = (m in after_avg) ? after_avg[m] : 0;
      d = aa - ba;
      pct = "n/a";
      if (ba != 0) {
        pct = sprintf("%.2f%%", (d * 100.0) / ba);
      }
      print m, sprintf("%.6f", ba), sprintf("%.6f", aa), sprintf("%.6f", d), pct,
        sprintf("%.6f", (m in before_min) ? before_min[m] : 0),
        sprintf("%.6f", (m in before_max) ? before_max[m] : 0),
        sprintf("%.6f", (m in after_min) ? after_min[m] : 0),
        sprintf("%.6f", (m in after_max) ? after_max[m] : 0),
        ((m in before_n) ? before_n[m] : 0),
        ((m in after_n) ? after_n[m] : 0);
    }
  }
' "$tmp_before" "$tmp_after" | sort
