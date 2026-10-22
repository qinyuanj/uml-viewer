#!/bin/sh
# Rust viewer run of the benchmark protocol 1 (bench/README.md, #143).
# Usage: bench/run_rust.sh <synthetic-N.edn> <out-dir>
# Needs a WSLg desktop session and `cargo build --release --locked` first.
# Optional: UML_VIEWER_BENCH_ONLY=<phase> (one phase), BENCH_TIMEOUT seconds
# (default 600). Exit 0 = every protocol-1 criterion passed on a D3D12
# renderer; 1 = a criterion failed or the run is void (see summary.txt).
set -eu
REPO=$(cd "$(dirname "$0")/.." && pwd)
IR=$(realpath "$1"); OUT=$2
mkdir -p "$OUT"; OUT=$(realpath "$OUT")
BINARY=${UML_VIEWER_RUST_EXECUTABLE:-$REPO/target/release/uml-viewer}
if [ ! -x "$BINARY" ]; then
  echo "missing $BINARY; run cargo build --release --locked (or get-uml-viewer)" >&2
  exit 2
fi
# An isolated project root: no .metrics overlay, no mailbox, no companion.
PROJECT=$(mktemp -d); cp "$IR" "$PROJECT/diagram.edn"
status=0
GALLIUM_DRIVER=d3d12 WGPU_BACKEND=gl \
  UML_VIEWER_BENCH="$OUT" UML_VIEWER_EVIDENCE="$OUT" UML_VIEWER_PROJECT_ROOT="$PROJECT" \
  timeout "${BENCH_TIMEOUT:-600}" "$BINARY" viewer "$PROJECT/diagram.edn" \
  >"$OUT/stdout.txt" 2>"$OUT/stderr.txt" || status=$?
rm -rf "$PROJECT"
echo "exit=$status"; cat "$OUT/summary.txt" 2>/dev/null || tail -5 "$OUT/stderr.txt"
exit $status
