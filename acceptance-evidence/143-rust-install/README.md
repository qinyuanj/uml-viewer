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
| 2000 类平移/缩放 p95 ≤ 33.3 ms | **未通过** | `rust-2000-d3d12/summary.txt`：p95 为 96.7 / 128.3 / 222.6 / 255.8 ms |
| 500 类稳定 60 fps | **未通过** | `rust-500-d3d12/summary.txt`：fps 为 30.6 / 20.8 / 22.3 / 14.6 |
| 空闲 CPU 近零（≤ 2%） | **未通过** | 500：6.41%；2000：19.81% |
| 大图首帧不崩溃 | 通过 | 2000 类完整跑完 warmup 与五个相位，stderr 中没有 panic；`rust-2000-d3d12/render.png` 是第 4 帧截图 |
| 测量结果与结论写入验收证据 | 通过 | 本文件与两个结果目录 |

所有运行的 renderer 均为 `D3D12 (NVIDIA GeForce GTX 1080 Ti)`，backend 为 `Gl`，
因此有效，不属于作废情况（见 `renderer.txt` 与 `protocol.edn` 的 `:environment`）。
窗口 1500 × 920，DPI 1，vsync 用 eframe 默认值（AutoVsync）。

## Protocol 1 结果（Rust viewer，D3D12）

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

viewer 的性能**未**达到 protocol 1 门槛。#131 原型在同一台机器上达标
（2000 类 p95 ≤ 26.6 ms），所以差距在 viewer 的实现，而不在平台。
`app_cpu_ms_p95`（eframe 每帧 CPU）与帧间隔基本一致，说明瓶颈在 CPU 端：

- `crates/viewer/src/paint.rs` 中的 `paint()` 每帧都要从 EDN 场景 `Value`
  重建完整的 `commands::commands(...)`（第 343 行）和 `commands::edge_commands(...)`
  （第 754 行），之后才做视口剔除。这正是 ADR 0008 认定的瓶颈：每帧在 CPU 上
  重建全部几何。原型是一次性预先计算几何和每条边的包围盒。
- 空闲 CPU 是同一种开销按 250 ms 轮询节奏触发的结果：`app.rs` 每帧都会调用
  `request_repaint_after(POLL_INTERVAL)`，所以空闲时每秒重绘约 4 次，每次都要
  重建完整的命令列表（500 类约 15 ms，2000 类约 63 ms）。

这里没有修复。修复需要按场景 revision/选中/悬停缓存命令列表（在 `paint.rs`/`app.rs`），
而这两个文件属于并行的 #140；我也不在验收工单里重设计 viewer 的渲染路径。
建议另开工单：缓存命令与边几何（含预先计算的包围盒），只在 revision、选中或悬停
变化时失效；并且只在 mtime 变化时才重绘，而不是按轮询节奏重绘。修复后用下面的
命令复测即可。

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
