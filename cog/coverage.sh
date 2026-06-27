#!/usr/bin/env bash
# Code coverage via rustc's native -C instrument-coverage + Homebrew llvm tools
# (must match rustc's LLVM major version). Output: terminal summary + tmp/coverage/html.
set -euo pipefail
cd "$(dirname "$0")"

LLVM_BIN=/opt/homebrew/opt/llvm/bin
PROFDATA="$LLVM_BIN/llvm-profdata"
COV="$LLVM_BIN/llvm-cov"
OUT=tmp/coverage
rm -rf "$OUT"; mkdir -p "$OUT/raw"

export RUSTFLAGS="-C instrument-coverage"
export LLVM_PROFILE_FILE="$PWD/$OUT/raw/cog-%p-%m.profraw"

# Build instrumented binaries. Capture BOTH the test executables (to run) and the
# `cog` bin (spawned as a subprocess by the e2e tests — needed as an --object below).
JSON=$(cargo test --tests --no-run --message-format=json 2>/dev/null)
TEST_BINS=$(echo "$JSON" | python3 -c '
import sys, json
for line in sys.stdin:
    try: m = json.loads(line)
    except: continue
    if m.get("profile",{}).get("test") and m.get("executable"):
        print(m["executable"])')
BIN_COG=$(echo "$JSON" | python3 -c '
import sys, json
for line in sys.stdin:
    try: m = json.loads(line)
    except: continue
    t = m.get("target",{})
    if "bin" in t.get("kind",[]) and m.get("executable"):
        print(m["executable"])')

# Run each test binary (generates .profraw, incl. from the spawned cog process).
for b in $TEST_BINS; do "$b" >/dev/null 2>&1; done

# Merge raw profiles.
"$PROFDATA" merge -sparse "$OUT"/raw/*.profraw -o "$OUT/cog.profdata"

# Report coverage of the `cog` bin (where the real code runs under e2e tests).
OBJ_ARGS=()
for b in $BIN_COG; do OBJ_ARGS+=(--object "$b"); done

# Ignore test harness + deps from the report; keep src/.
IGNORE='(/\.cargo/|/rustc/|tests/|/registry/)'

echo "=== Coverage summary ==="
"$COV" report "${OBJ_ARGS[@]}" \
  --instr-profile="$OUT/cog.profdata" \
  --ignore-filename-regex="$IGNORE" \
  --sources src

"$COV" show "${OBJ_ARGS[@]}" \
  --instr-profile="$OUT/cog.profdata" \
  --ignore-filename-regex="$IGNORE" \
  --format=html --output-dir="$OUT/html" \
  --sources src
echo "HTML report: $OUT/html/index.html"
