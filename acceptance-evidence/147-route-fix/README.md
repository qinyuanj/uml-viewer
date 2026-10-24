# #147 验收记录：连线路由修复前后对照（#148 / #150）

## 采集平台

- WSL2（内核 `6.18.33.2-microsoft-standard-WSL2`）+ Ubuntu 24.04.5 LTS + WSLg
  （`DISPLAY=:0`、`WAYLAND_DISPLAY=wayland-0`），符合 ADR 0007 的交付目标。
- Ryzen 5 5500 / NVIDIA GeForce GTX 1080 Ti；Rust 1.99.1，egui/eframe 0.37，
  wgpu GL 后端。
- `renderer.txt` 里的 `D3D12 (NVIDIA GeForce GTX 1080 Ti)` 是 WSLg 下 Mesa
  25.2.8（`libgl1-mesa-dri` / `mesa-libgallium` 25.2.8-0ubuntu0.24.04.2）的
  gallium d3d12 驱动（`/usr/lib/x86_64-linux-gnu/dri/d3d12_dri.so`，经
  `/usr/lib/wsl/lib/libd3d12.so` 转到 Windows 宿主的 GPU）给出的 GL renderer
  字符串，不是 Windows 原生 D3D12 后端。2026-10-24 在同一台机器上重跑
  `route_shots`，`renderer.txt`、`route148.png`、`route148-view.edn` 与
  `after/` 中的文件逐字节相同（stderr 里有 WSLg 常见的 `libEGL warning: DRI3
  error`，不影响渲染）。
- 窗口 1500 × 920，DPI 1。项目里没有放 Sarasa 字体，所以回退到 Noto CJK，修复前后相同。

## 二进制

- `before/`：e4e1705（#148 修复前）的 release 构建。
- `after/`：#148 修复后（`route148*` 为 109020d，其余为 #150 分支 a979f90）的
  release 构建。两者之间 `crates/` 的源码差异只有 `layout.rs` 中 `hide-members`
  的一行。

## driver 与命令

修复前后用同一个脚本，只换二进制（`UML_VIEWER_RUST_EXECUTABLE`）。每张截图前先
dump-view，所以每个 `*.png` 都配有同名的 `*-view.edn`。

| driver | 输入（放进项目目录） | 产出 |
|---|---|---|
| `scripts/drivers/route_shots.edn` | `fixtures/route-goldens/route148.ir.edn`，另存为 `route148.edn` | `route148.png`、`route148-view.edn` |
| `scripts/drivers/route_library.edn` | `examples/library.edn` | `library-{full,detail,frames,blocks}.png` 及 view |
| `scripts/drivers/route_proposal.edn` | `examples/uml-viewer.edn` | `proposal-{full,frames,blocks}.png` 及 view、`proposal-fit-view.edn` |

```
python3 -I scripts/run_viewer_driver.py route_shots --project-root <dir> \
  --diagram route148.edn --evidence <out> --seconds 40 --pass-marker route148.png
python3 -I scripts/run_viewer_driver.py route_library --project-root <dir> \
  --diagram library.edn --evidence <out> --seconds 40 --pass-marker library-blocks.png
python3 -I scripts/run_viewer_driver.py route_proposal --project-root <dir> \
  --diagram uml-viewer.edn --evidence <out> --seconds 40 --pass-marker proposal-blocks.png
```

`renderer.txt` 由启动器写入，不由 driver 写。

## 观察

| 文件 | 修复前后 | 观察 |
|---|---|---|
| `route148.png` | 不同 | #148 最小复现。修复前 LoanService → Loan 是直角 S 形绕行，类框缺成员；修复后两条边都是直线，与原版一致 |
| `library-full.png` / `library-detail.png` | 不同 | 修复前所有类框缩成名字条，LoanService 左侧有钩形回退线；修复后成员行恢复，连线直接 |
| `library-frames.png`（0.25） | 不同 | 只画框、类名和抽取后的边，走向与 Full 一致 |
| `library-blocks.png`（0.1） | 不同 | 远档不画边（spec #134），只有块和包 |
| `proposal-{full,frames,blocks}.png` | 逐字节相同 | 层级 IR 不经过 `:hide-members false` 分支，提案视图修复前后无差异；其中的绕行和交叉是原版行为 |

这些截图是人工验收证据，不是像素 golden。几何回归由
`crates/scene/tests/route_goldens.rs` 对照原版 Lua 的静态 golden 覆盖。
