#!/bin/bash
# usage: run.sh <binary> <ir> — captures before (base rank order) and after
# (default span order) with the same binary, IR and capture steps.
set -u
here=$(cd "$(dirname "$0")" && pwd)
bin=$1; ir=$2
for variant in before after; do
  script=$here/capture.edn
  [ "$variant" = before ] && script=$here/capture-base.edn
  out=$here/$variant
  rm -rf "$here/xdg" "$here/proj" "$out"; mkdir -p "$here/xdg" "$here/proj" "$out"
  cp "$ir" "$here/proj/diagram.edn"
  (cd "$here/proj" && env XDG_CONFIG_HOME=$here/xdg/config XDG_DATA_HOME=$here/xdg/data \
    XDG_CACHE_HOME=$here/xdg/cache XDG_STATE_HOME=$here/xdg/state \
    UML_VIEWER_PROJECT_ROOT=$here/proj UML_VIEWER_EVIDENCE=$out UML_VIEWER_TEST_SECONDS=60 \
    UML_VIEWER_TEST_DRIVER=$script UML_MAIL_TEST_ROOT=$here/proj UML_COMPANION_NO_ATTACH=1 \
    GALLIUM_DRIVER=d3d12 WGPU_BACKEND=gl timeout 150 "$bin" viewer diagram.edn \
    > "$out/stdout.txt" 2> "$out/stderr.txt")
  echo "$variant exit $?"
done
rm -rf "$here/xdg" "$here/proj"
