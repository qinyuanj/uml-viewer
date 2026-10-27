# #183 rank 内 span 排序：验收证据

日期 2026-10-27，WSL2 + Ubuntu 24.04 + WSLg。`fs-monopoly/renderer.txt` 记录适配器
（D3D12 / GTX 1080 Ti，`Gl` 后端）。

## 实现

布局在 5 轮重心排序之后，对每个 rank 做一次 rank 内重排，目标是所有依赖两端类中心的横向距离之和最小
（`RankOrder::Span`，默认）。每一步把一个类移到本 rank 内最好的位置，候选位置用相邻交换链增量求值；
只接受严格改进，所以代价相同时保留重心顺序。搜索有界：最多 8 轮；扫描经过的每个位置计 1 加上重新度量的边数，
每次接受的移动计所在 rank 的长度，总工作量上限为 `400 ×（类数 + 边数）`，按计数截止，结果确定。
所有放进同一视图的关系边都计入跨度，与重心排序使用的邻居相同。rank 分配、框尺寸、端口、路由与线束规则不变。

lr/rl 方向的每个 rank 都有居中偏移 `(total_h − colh) / 2`。`colh` 与顺序无关，所以把偏移算进类中心后，
目标函数在水平方向同样成立，两个方向都启用。类中心取实际绘制框的中心：tb 是槽位中点，lr/rl 是
`in-space + content-h / 2`（入端口在上方，出端口在下方，两者高度不同）。

## 基础几何与新布局的区分

- `RankOrder::Base` 只做重心排序，即迁移基础几何。`document::compile_view_with` 与
  `cli::project_with` 显式接收顺序；`compile_view`、`cli::project`、viewer 和 `scene` 命令都用默认的 `Span`。
- 历史 JVM/Lua goldens（`golden_geometry`）和 route goldens 按 `Base` 比对。fixture 与 `1e-6` 容差都没有改动。
- #180/#181 的 `bundle_lifecycle.ir.edn` 和 `bundle_acceptance.ir.edn` 是专门构造的拥挤扇出，而 span
  排序恰好会把这种拥挤消掉。例如 lifecycle 提案层只有一个 rank，`f.s` 会移到 `f.a` 和 `f.b` 之间，
  线束就不再需要。这两份 fixture 固定的是基础布局上的线束规划，所以它们的测试同样按 `Base` 运行：
  `ViewState.rank_order` 不持久化，由 `document::with_rank_order` 设置；导航、重载和 `:display`
  都会沿用它。对应的 native driver 在脚本顶层声明 `:rank-order :base`，这是仅供测试的 driver 字段，
  不是用户配置。fixture 和 driver 坐标都没有改动。
- 线束在默认布局下的行为，仍由 bench 的 `assert_real_bundles`，以及 `dependency_bundles`、
  `bundle_safety`、`short_channels`（均按默认顺序）覆盖。

## 自动检查

- `cargo test --workspace --locked`：626 通过，0 失败。`cargo clippy --workspace --all-targets -- -D warnings`
  与 `cargo fmt --check` 均通过。
- `python3 -I scripts/check.py --native`：24 项全部 PASS，含 bundle_acceptance、bundle_lifecycle、
  bundle_safety、dependency_bundles 四个真实窗口 driver。
- 基准（`bench/`，同机，#172 记录的数值作对照）：

  | 项目 | #172 release | 本分支 release | #172 debug | 本分支 debug | 阈值 |
  | --- | --- | --- | --- | --- | --- |
  | 2000 类 compile | 2.85 s | 2.11 s | 14.6 s | 10.8 s | < 30 s |
  | 500→2000 比值 | 7.3x | 7.3x | 8.5x | 8.1x | < 10x |
  | 240 条平行边开销 | 1.47x | 1.53x | 1.74x | 1.80x | < 2.0x |

  三项都在阈值内。#172 的数值是另一次运行测得的，机器负载不同，这里只作量级对照。平行边开销是线束规划与
  同布局 association 孪生图的耗时比，span 排序对两边相同；debug 下 1.80x 是余量最小的一项，与 #172 记录的
  1.74x 处在同一波动范围。

## fs-monopoly 实图（人工证据）

输入是 #181/#172 用过的同一份现成 IR（sha256 `7f81a996…3fba`），capture 步骤与 #172 相同。
`fs-monopoly/run.sh <binary> <ir>` 用同一个 release 二进制截两次：before 用 `capture-base.edn`
（`:rank-order :base`），after 用 `capture.edn`（默认顺序）。before 的 `root.png` 和 `ui.png` 与
`172-review-fix/fs-monopoly/` 逐字节一致。`report.py` 先断言两次的类集合、rank、框尺寸和关系语义相同，
再从画布 dump 计算 `metrics.json`，并生成 `compare-root.png`（左右排列）与 `compare-ui.png`
（上下排列，上为 before）。画布 dump 体积较大，没有提交。

| 视图 | 版本 | bundles | 主干总长 | 绘制线长 | 交叉 | 依赖跨度 | 同目标箭头最小间距 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 根层 | before | 12 | 849 | 12365 | 74 | 1760 | 10.133 |
| 根层 | after | 12 | 849 | 12365 | 74 | 1760 | 10.133 |
| Ui 层 | before | 16 | 2793 | 31455 | 311 | 25096 | 15.2 |
| Ui 层 | after | 8 | 1009 | 22424 | 147 | 16272 | 10.64 |

- **根层**：现有顺序已经是 span 最优，before 和 after 的截图逐字节相同。左侧纵向通道的拥挤来自跨多个
  rank 的长边，不在本单元范围内。
- **Ui 层**：依赖跨度和原型 span 变体相同（16272），绘制线长下降约 29%，交叉减半。横贯整行的长弧明显减少，
  Ports、View、State 相互靠拢，Screens_registry 移到 Seams 和 Manager 之间。线本身变短后，能共享的
  长平行段也少了，所以线束从 16 个降到 8 个；剩下的线束分支都从基础 stroke 的端点开始和结束
  （错位数为 0，见 `after/ui-analysis.txt`）。底部仍有不少交错曲线。
- 这些数字只是本次实图的观察，没有写成测试常量。
