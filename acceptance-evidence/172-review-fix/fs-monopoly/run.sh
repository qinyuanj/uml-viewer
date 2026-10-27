#!/bin/bash
# usage: run.sh <binary> <evidence-dir>; launches the real viewer on the copied fs-monopoly IR.
set -u
here=$(cd "$(dirname "$0")" && pwd)
bin=$1; ev=$2
rm -rf "$here/xdg" "$ev"; mkdir -p "$here/xdg" "$ev"
cd "$here/proj"
env XDG_CONFIG_HOME=$here/xdg/config XDG_DATA_HOME=$here/xdg/data XDG_CACHE_HOME=$here/xdg/cache XDG_STATE_HOME=$here/xdg/state \
  UML_VIEWER_PROJECT_ROOT=$here/proj UML_VIEWER_EVIDENCE=$ev UML_VIEWER_TEST_SECONDS=60 UML_VIEWER_TEST_DRIVER=$here/capture.edn \
  UML_MAIL_TEST_ROOT=$here/proj UML_COMPANION_NO_ATTACH=1 \
  GALLIUM_DRIVER=d3d12 WGPU_BACKEND=gl timeout 150 "$bin" viewer diagram.edn > "$ev/stdout.txt" 2> "$ev/stderr.txt"
echo "exit $?"
