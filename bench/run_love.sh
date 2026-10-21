#!/bin/sh
# LOVE reference run of the benchmark protocol (bench/README.md).
# Usage: bench/run_love.sh <synthetic-N.edn> <llvmpipe|d3d12> <out-dir>
# Optional: UML_CJK_FONT (Sarasa Mono SC), BENCH_TIMEOUT seconds (default 1800;
# the 2000-class scene spends minutes in the Lua scene core before frame 1).
set -eu
REPO=$(cd "$(dirname "$0")/.." && pwd)
IR=$(realpath "$1"); DRIVER=$2; OUT=$3
mkdir -p "$OUT"; OUT=$(realpath "$OUT")
# An isolated project root: no .metrics overlay, no mailbox, no companion.
PROJECT=$(mktemp -d); cp "$IR" "$PROJECT/diagram.edn"
# The viewer loads cimgui from <project>/.uml-viewer; reuse the provisioned copy.
mkdir -p "$PROJECT/.uml-viewer"
ln -s "$REPO/.uml-viewer/cimgui-love-1.90-1" "$PROJECT/.uml-viewer/cimgui-love-1.90-1"
case $DRIVER in
  llvmpipe) GALLIUM=llvmpipe ;;
  d3d12) GALLIUM=d3d12 ;;
  *) echo "driver must be llvmpipe or d3d12" >&2; exit 2 ;;
esac
status=0
HIDE=""; [ "${UML_VIEWER_HIDE_EDGES:-}" = 1 ] && HIDE=1
LEGACY=""; [ "${UML_VIEWER_LEGACY:-}" = 1 ] && LEGACY=1
GALLIUM_DRIVER=$GALLIUM UML_VIEWER_HIDE_EDGES=$HIDE UML_VIEWER_LEGACY=$LEGACY UML_VIEWER_TEST_DRIVER="$REPO/bench/love_protocol_driver.lua" \
  UML_VIEWER_EVIDENCE="$OUT" UML_VIEWER_TEST_SECONDS=100000 UML_VIEWER_PROJECT_ROOT="$PROJECT" \
  UML_COMPANION_NO_ATTACH=1 timeout "${BENCH_TIMEOUT:-1800}" love "$REPO/love" viewer "$PROJECT/diagram.edn" \
  >"$OUT/stdout.txt" 2>"$OUT/stderr.txt" || status=$?
rm -rf "$PROJECT"
echo "exit=$status"; cat "$OUT/summary.txt" 2>/dev/null || tail -5 "$OUT/stderr.txt"
exit $status
