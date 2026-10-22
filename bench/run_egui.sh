#!/bin/sh
# PROTOTYPE (#131): run the egui prototype under protocol 1 on WSLg + D3D12.
# Usage: bench/run_egui.sh <scene.json> <out-dir> [vsync 0|1]
set -eu
here=$(dirname "$0")
GALLIUM_DRIVER=d3d12 WGPU_BACKEND=gl BENCH_VSYNC=${3:-1} \
  "$here/egui-proto/target/release/egui-bench-proto" "$1" "$2" >"$2.stdout" 2>"$2.stderr" || true
mkdir -p "$2"; mv "$2.stdout" "$2/stdout.txt"; mv "$2.stderr" "$2/stderr.txt"
cat "$2/summary.txt"
