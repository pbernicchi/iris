#!/bin/bash
# JIT diagnostic launcher — runs emulator and captures output for analysis
# Usage: ./jit-diag.sh [mode]
#   mode: "jit"      — JIT enabled (default)
#         "verify"   — JIT with verification
#         "nojit"    — interpreter only through JIT dispatch
#         "interp"   — pure interpreter (no JIT feature, baseline)
#         "perf"     — perf profile, interpreter only (text report for analysis)
#         "perf-jit" — perf profile with JIT enabled

MODE="${1:-jit}"
# IRIS_JIT_MAX_TIER from environment (0=Alu, 1=Loads, 2=Full, unset=Full)
TIER_ENV=""
if [ -n "$IRIS_JIT_MAX_TIER" ]; then
  TIER_ENV="IRIS_JIT_MAX_TIER=$IRIS_JIT_MAX_TIER"
fi
OUTFILE="jit-diag-$(date +%Y%m%d-%H%M%S)-${MODE}.log"

echo "=== IRIS JIT Diagnostic ===" | tee "$OUTFILE"
echo "Mode: $MODE" | tee -a "$OUTFILE"
echo "Date: $(date)" | tee -a "$OUTFILE"
echo "Host: $(uname -m) $(uname -s) $(uname -r)" | tee -a "$OUTFILE"
echo "Rust: $(rustc --version)" | tee -a "$OUTFILE"
echo "" | tee -a "$OUTFILE"

case "$MODE" in
  jit)
    echo "Running: IRIS_JIT=1 $TIER_ENV cargo run --release --features jit,lightning" | tee -a "$OUTFILE"
    IRIS_JIT=1 $TIER_ENV cargo run --release --features jit,lightning 2>&1 | tee -a "$OUTFILE"
    ;;
  verify)
    echo "Running: IRIS_JIT=1 IRIS_JIT_VERIFY=1 $TIER_ENV cargo run --release --features jit,lightning" | tee -a "$OUTFILE"
    IRIS_JIT=1 IRIS_JIT_VERIFY=1 $TIER_ENV cargo run --release --features jit,lightning 2>&1 | tee -a "$OUTFILE"
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
