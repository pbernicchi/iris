#!/bin/bash
# JIT diagnostic launcher — runs emulator and captures output for analysis
# Usage: ./jit-diag.sh [mode]
#   mode: "jit"      — JIT enabled (default)
#         "verify"   — JIT with verification
#         "nojit"    — interpreter only through JIT dispatch
#         "interp"   — pure interpreter (no JIT feature, baseline)
#         "perf"     — perf profile, interpreter only (text report for analysis)
#         "perf-jit" — perf profile with JIT enabled
#
# All IRIS_JIT_* env vars are passed through automatically:
#   IRIS_JIT_MAX_TIER=0 ./jit-diag.sh jit
#   IRIS_JIT_PROBE=500 IRIS_JIT_PROBE_MIN=100 ./jit-diag.sh jit

MODE="${1:-jit}"
OUTFILE="jit-diag-$(date +%Y%m%d-%H%M%S)-${MODE}.log"

# Collect all IRIS_JIT_* env vars for display and passthrough
JIT_VARS=$(env | grep '^IRIS_JIT_' | tr '\n' ' ')

echo "=== IRIS JIT Diagnostic ===" | tee "$OUTFILE"
echo "Mode: $MODE" | tee -a "$OUTFILE"
echo "Date: $(date)" | tee -a "$OUTFILE"
echo "Host: $(uname -m) $(uname -s) $(uname -r)" | tee -a "$OUTFILE"
echo "Rust: $(rustc --version)" | tee -a "$OUTFILE"
[ -n "$JIT_VARS" ] && echo "Env: $JIT_VARS" | tee -a "$OUTFILE"
echo "" | tee -a "$OUTFILE"

case "$MODE" in
  jit)
    echo "Running: IRIS_JIT=1 ${JIT_VARS}cargo run --release --features jit,lightning" | tee -a "$OUTFILE"
    IRIS_JIT=1 cargo run --release --features jit,lightning 2>&1 | tee -a "$OUTFILE"
    ;;
  verify)
    echo "Running: IRIS_JIT=1 IRIS_JIT_VERIFY=1 ${JIT_VARS}cargo run --release --features jit,lightning" | tee -a "$OUTFILE"
    IRIS_JIT=1 IRIS_JIT_VERIFY=1 cargo run --release --features jit,lightning 2>&1 | tee -a "$OUTFILE"
    ;;
  nojit)
    echo "Running: cargo run --release --features jit,lightning (no IRIS_JIT)" | tee -a "$OUTFILE"
    cargo run --release --features jit,lightning 2>&1 | tee -a "$OUTFILE"
    ;;
  interp)
    echo "Running: cargo run --release --features lightning (no jit feature)" | tee -a "$OUTFILE"
    cargo run --release --features lightning 2>&1 | tee -a "$OUTFILE"
    ;;
  perf)
    PERFREPORT="perf-report-$(date +%Y%m%d-%H%M%S).txt"
    echo "Building (profiling profile, no jit feature)..." | tee -a "$OUTFILE"
    cargo build --profile profiling --features lightning 2>&1 | tee -a "$OUTFILE"
    echo "--- Press Ctrl-C when you have enough samples ---"
    perf record -F 99 --call-graph dwarf -o perf.data -- ./target/profiling/iris
    echo "Processing perf data..." | tee -a "$OUTFILE"
    perf report --stdio --no-children -i perf.data > "$PERFREPORT" 2>&1
    echo "Perf report saved to: $PERFREPORT"
    ;;
  perf-jit)
    PERFREPORT="perf-report-jit-$(date +%Y%m%d-%H%M%S).txt"
    echo "Building (profiling profile, jit feature)..." | tee -a "$OUTFILE"
    cargo build --profile profiling --features jit,lightning 2>&1 | tee -a "$OUTFILE"
    echo "--- Press Ctrl-C when you have enough samples ---"
    IRIS_JIT=1 perf record -F 99 --call-graph dwarf -o perf.data -- ./target/profiling/iris
    echo "Processing perf data..." | tee -a "$OUTFILE"
    perf report --stdio --no-children -i perf.data > "$PERFREPORT" 2>&1
    echo "Perf report saved to: $PERFREPORT"
    ;;
  *)
    echo "Unknown mode: $MODE"
    echo "Usage: $0 [jit|verify|nojit|interp|perf|perf-jit]"
    exit 1
    ;;
esac

echo "" >> "$OUTFILE"
echo "=== Exit code: $? ===" >> "$OUTFILE"
echo "Output saved to: $OUTFILE"
