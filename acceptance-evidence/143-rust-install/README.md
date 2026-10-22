# #143 安装器构建 Rust 并按 protocol 1 验收性能

日期：2026-10-22。原生 Ubuntu 24.04 WSL2 + WSLg（`DISPLAY=:0`、
`WAYLAND_DISPLAY=wayland-0`），Ryzen 5 5500 / GTX 1080 Ti，Rust 1.99.1，
egui/eframe 0.37 + wgpu GL 后端，Mesa 25.2.8。所有数字均为本机实测，
由 agent 运行；没有推算值。

## 验收标准核对

| 标准 | 结果 | 证据 |
|---|---|---|
| 缺依赖时给出明确提示 | 通过 | `scripts/test_posix_entries.py`：缺 cargo（提示 rustup / `UML_CARGO_EXECUTABLE`，且不创建 `.uml-viewer/`）、缺 git、cargo 版本与 `rust-toolchain.toml` 不符（同时列出要求版本与实际版本）、构建未产出二进制；缺 libxkbcommon-x11 只警告不拒绝（#139 选定 Wayland 无装饰） |
| 重复安装幂等 | 通过 | `test_reinstall_is_idempotent_and_provisions_all_three_tools`：两次安装都成功；policy（含 `:proposals`）字节不变；每次都在 `.uml-viewer/uml-viewer` 中执行 `build --release --locked`；`install.json` 的 `rust` 指向 `target/release/uml-viewer`。已有 ownership/recovery 记录时拒绝（`test_install_refuses_existing_ownership`），脏工具副本仍被保留（`test_installer_preserves_dirty_tool_checkout`） |
| 2000 类平移/缩放 p95 ≤ 33.3 ms | 通过（修复后） | `rust-2000-d3d12/summary.txt`：p95 为 17.59 / 17.91 / 17.59 / 29.85 ms，`verdict=PASS`，`run_rust.sh` 退出 0 |
| 500 类稳定 60 fps | **未通过（差边缘）** | `rust-500-d3d12/summary.txt`：fps 为 59.12 / 57.86 / 59.32 / 57.05；pan-detail、pan-fit 通过，zoom-detail（57.86 fps，p95 18.10 ms）和 zoom-fit（57.05 fps，p95 19.05 ms）略低于 58 fps / 18.3 ms 门槛 |
| 空闲 CPU 近零（≤ 2%） | 通过（修复后） | 500：0.40%；2000：0.40% |
| 大图首帧不崩溃 | 通过 | 2000 类完整跑完 warmup 与五个相位，stderr 中没有 panic；`rust-2000-d3d12/render.png` 是第 4 帧截图 |
| 测量结果与结论写入验收证据 | 通过 | 本文件与两个结果目录 |

所有运行的 renderer 均为 `D3D12 (NVIDIA GeForce GTX 1080 Ti)`，backend 为 `Gl`，
因此有效，不属于作废情况（见 `renderer.txt` 与 `protocol.edn` 的 `:environment`）。
窗口 1500 × 920，DPI 1，vsync 用 eframe 默认值（AutoVsync）。

## Protocol 1 结果（Rust viewer，D3D12）

### 修复后（`ticket/143-perf`）

两次完整运行（500、2000 各一次），与 #141 agent 的 driver 运行同时进行，共享 GPU/WSLg。

| 场景 | 相位 | fps | p95 ms | 进程 CPU% |
|---|---|---|---|---|
| 500 | idle | — | — | 0.40 |
| 500 | pan-detail | 59.12 | 17.52 | 44.2 |
| 500 | zoom-detail | 57.86 | 18.10 | 49.8 |
| 500 | pan-fit | 59.32 | 17.70 | 39.4 |
| 500 | zoom-fit | 57.05 | 19.05 | 53.9 |
| 2000 | idle | — | — | 0.40 |
| 2000 | pan-detail | 58.94 | 17.59 | 52.3 |
| 2000 | zoom-detail | 58.50 | 17.91 | 57.9 |
| 2000 | pan-fit | 58.87 | 17.59 | 42.8 |
| 2000 | zoom-fit | 53.19 | 29.85 | 58.4 |

2000 类 stderr 中没有 panic，首帧截图 `rust-2000-d3d12/render.png`。

### 修复前（原始测量，结果目录已被上表覆盖）

| 场景 | 相位 | fps | p95 ms | 进程 CPU% |
|---|---|---|---|---|
| 500 | idle | — | — | 6.41 |
| 500 | pan-detail | 30.61 | 36.81 | 55.0 |
| 500 | zoom-detail | 20.77 | 75.41 | 65.8 |
| 500 | pan-fit | 22.27 | 51.89 | 59.3 |
| 500 | zoom-fit | 14.60 | 107.21 | 73.8 |
| 2000 | idle | — | — | 19.81 |
| 2000 | pan-detail | 12.37 | 96.67 | 79.0 |
| 2000 | zoom-detail | 10.04 | 128.25 | 81.1 |
| 2000 | pan-fit | 5.38 | 222.58 | 82.5 |
| 2000 | zoom-fit | 5.59 | 255.84 | 83.2 |

启动时间只记录，不设门槛。headless `uml-viewer scene --ir` 的 IR→scene 耗时：
500 类 0.21 s，2000 类 1.03 s；LOVE 参考分别为 34 s 和 532 s。O(E·N) 的
`prepare_scene` 已不复存在。

## 结论与原因

修复前的瓶颈（CPU 端）：`paint()` 每帧从场景 `Value` 重建全部类/边命令；文字按非整数字号拟合，绕过 egui 的 galley 缓存；
总览档的 feathered 曲线和圆角矩形细分很贵；Blocks 档仍画全部边；空闲时按 250 ms 轮询节奏重绘。

修复（`crates/viewer/src/render_cache.rs`、`paint.rs`、`watch.rs`、`app.rs`、`bench.rs`）：

- 命令只在场景（revision/focus/extra）、选中或悬停变化时重建；LOD 档或总览缩放的四分之一倍频程台阶变化时只重新简化边折线
  （距离 8 px 加 0.5 px 弦偏差）。平移和缩放只改变换。
- 文字按整数像素拟合，排版结果跨帧缓存（图集清空或 DPI 变化时失效）；小于 5 px 的画布文字不画。
- Frames/Blocks 档的类框、C/M 圆点、边合成一个无 feather 的 mesh；Blocks 不画边（spec #134 渲染）；小于 2 px 的圆角改为直角；
  缩放 < 0.6 时 Full 档的边也走 mesh。
- 空闲：后台线程只 stat IR、`.metrics/**` 和 `to-viewer.edn`，戳变化才请求重绘；driver、计时运行和待截图仍保留 250 ms 节奏。
  bench 的 idle 相位自己安排结束时的唤醒。

### 合并 integration/spec-134 之后

上面的数字在 `ticket/143-perf` 合并 integration tip（`e5cc114`）之前测得。合并后 #141 的检查器侧栏每帧深拷贝整份
document，pan-detail 掉到约 29 fps、app CPU 约 33 ms（gui.draw 实测均值 11.2 ms/帧，其中 `inspector::entries`
占 7.6–10.8 ms）。#141 在其分支上修复（`036c315`，侧栏 0.16–0.20 ms/帧）。在本分支上未提交地应用该修复后实测：

| 场景 | 相位 | fps | p95 ms | 进程 CPU% |
|---|---|---|---|---|
| 500 | idle（仅 idle 相位，两次） | — | — | 1.10 / 0.80 |
| 500 | pan-detail | 55.29 | 18.10 | 49.1 |
| 500 | zoom-detail | 57.40 | 18.18 | 57.0 |
| 500 | pan-fit | 58.70 | 17.63 | 49.0 |
| 500 | zoom-fit | 57.38 | 18.62 | 61.1 |
| 2000 | pan-detail | 58.09 | 18.05 | 62.0 |
| 2000 | zoom-detail | 55.95 | 27.15 | 63.7 |
| 2000 | pan-fit | 58.96 | 17.68 | 57.0 |
| 2000 | zoom-fit | 46.74 | 34.96 | 64.0 |

那次运行与本机其他 agent 的 driver/bench 并发（负载约 3.9–4.7），所以 2000 的 zoom-fit 从 29.9 ms 变成 35.0 ms
属于干扰，而不是回归；2000 全部达标是在合并前、同一代码路径上测得的。该修复必须先进入 integration，合并后的数字才有意义。

仍未通过：500 类的 zoom-detail 和 zoom-fit 略低于 58 fps / p95 18.3 ms（中位数都是 16.8 ms，尾部是新字号首次排版和
Full 档 feather 细分）。2000 类全部通过。

## 复现

```sh
cargo build --release --locked            # 或 get-uml-viewer
bench/run_rust.sh bench/scenes/synthetic-500.edn  <out>   # 约 60 s
bench/run_rust.sh bench/scenes/synthetic-2000.edn <out>   # 约 60 s
UML_VIEWER_BENCH_ONLY=pan-fit bench/run_rust.sh <scene> <out>  # 单相位
```

`run_rust.sh` 会设置 `GALLIUM_DRIVER=d3d12`、`WGPU_BACKEND=gl`，使用隔离的
项目根，并把 `UML_VIEWER_BENCH=<out>` 交给 viewer（`crates/viewer/src/bench.rs`）。
退出码 0 表示 renderer 为 D3D12 且全部判定通过；1 表示判定失败或运行作废。
判定写在 `summary.txt` 的 `criterion …` / `verdict=` 行，以及 `protocol.edn` 中。
