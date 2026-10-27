# #172 整分支 review 修复：验收证据

日期 2026-10-27，WSL2 + Ubuntu 24.04 + WSLg，`renderer.txt` 记录适配器
（D3D12 / GTX 1080 Ti，`Gl` 后端，driver 名为空，所以不写 `driver:` 行）。

## 自动检查

- `cargo test --workspace`：615 通过，0 失败。`cargo clippy --workspace --all-targets -- -D warnings`
  与 `cargo fmt --check` 均干净。
- `python3 -I scripts/check.py --native`：24 项全部 PASS，含 bundle_acceptance、bundle_lifecycle、
  bundle_safety、dependency_bundles 四个真实窗口 driver（`check-native.txt`）。
- 基准（`bench/`，同机；基线是 d5fe2dfd）：

  | 项目 | 基线 release | 本分支 release | 基线 debug | 本分支 debug | 阈值 |
  | --- | --- | --- | --- | --- | --- |
  | 2000 类 compile | 2.35 s | 2.85 s | 9.7 s | 14.6 s | < 30 s |
  | 500→2000 比值 | 6.3x | 7.3x | 7.9x | 8.5x | < 10x |
  | 240 条平行边开销 | 1.08x | 1.47x | 1.06x | 1.74x | < 2.0x |

  三项都在阈值内，但规划开销比基线高。debug 下平行边开销 1.74x，离 2.0x 只差约 13%，
  这是余量最小的一项。2000 类图和平行边压力仍然产生非空、有界的线束（由 `assert_real_bundles` 断言）。

## fs-monopoly 实图（人工证据）

使用与 #181 相同的现成 IR（sha256 `7f81a996…3fba`）和 capture driver（`capture.edn`）。
fs-monopoly 仓库未做改动。`run.sh` 是启动包装，使用隔离的 XDG 环境，不指定 CJK 字体，
与 #181 一样走 fallback 字体。根层和双击 Ui 后的相机视图与 #181 after 完全一致
（`root-view.edn`、`ui-view.edn` 内容逐字节相同）。

`compare-root.png` 从左到右依次是 #181 before、#181 after、本分支；`compare-ui.png`
从上到下是同样的顺序。`analysis.txt` 由 `analyze.py` 从本分支画布 dump 计算得到。

- **端点**：两层 68 条逻辑边的 from/to/kind/points/tip/behind/curve/head/deps 与类布局，
  和 d5fe2dfd 的 `scene` 投影逐项相同。每条成束边的第一段分支都从基础 stroke 的起点开始，
  最后一段都在基础 stroke 的终点结束（不符的边数为 0），头部仍是每条边各自的。
- **折返和极短分支**：#181 after 里的直角折线和小方框形折返已经消失。分支只沿展平后的
  基础曲线切出，最大转角两层分别是 48° 和 31°；根层同一条边的基础曲线自身已达 48°。
  除了基础 stroke 里本来就有的 1.8px 裁剪残段，没有短于 24px 的分支。截图里还能看到少量
  小弧钩，出现在主干端靠近箭头的弯段处，例如 Host→Ui 上方左侧。
- **根层拥挤**：线束从 3 个增加到 12 个（成员 2–5，主干 32–121px）。Player→Rules、
  Rules→Config 之间的近平行扇形合成了几股，线条数量明显减少。左侧最密的纵向通道和进入
  Config/Foundation 的扇入只在靠近目标处短暂合并，整体密度和 #181 before 差不多。
  结论：**有改善，但幅度不大**。
- **Ui 层拥挤**：线束从 6 个增加到 16 个，其中 4 个长主干在 280–485px 之间，几组长的水平
  平行线合成一股。主干两端的汇合处呈透镜状，多股线在那里收拢再分开，下方交错的曲线仍然很多。
  结论：**比 #181 before 稍清楚，没有 #181 after 的折返，但离「一眼辨认结构」仍有距离**。
- 总评：方向一（准入）的目标达到了，没有折返、极短分支或端点漂移。方向二（密集通道成束）
  只部分达到。剩下的拥挤主要来自布局：扇入和扇出线各自从不同端口出发，彼此只在一端接近，
  要进一步减轻，需要端口或布局层面的配合，不在本单元范围内。

## 线束 fixture 与 golden 的变化

历史 JVM/Lua golden、route golden、fixture 和 `1e-6` 容差都没有改动。下面的变化只涉及
#178–#181 新增的线束 fixture 和最终线束 golden，原因是这些小图上旧主干依赖的就是本次要拒绝的几何：

- `bundle-goldens/safe-channel.geometry.edn`：旧主干由 3 名成员组成，长 32px，是靠
  `[118.88 270]→[137.12 270]` 这样的直角 jog 把成员接进来的。新主干只有 c、d 两条长平行段，
  主干沿 c 的曲线走，b 和 a 保留基础路径。
- `bundle-goldens/shared-channel.{ir,geometry}.edn`：旧图里两条边的共享段只有 47px，
  还要减去圆角，留不出主干加两段弯。新图改成两个源跨 6 个 rank 汇入同一目标，两条线只在
  靠近目标处合并，成员仍然来自不同源，所以依赖链的半点亮语义还能测到。
- `bundle_lifecycle.ir.edn`：改为 s 跨 3 个 rank 扇出到 a、b，左侧放填充类；提案增加 Shell 层，
  因为 Unassigned 层不带层级，不会生成近平行族。driver 的坐标、依赖链检查和三角数量（4→3）随之更新。
  单一源扇出没法表达「主干亮而某个分支暗」，所以 lifecycle driver 改为检查选中 s 时全亮、
  选中 m 时全暗；混合语义继续由 dependency_bundles 覆盖。
- `bundle_acceptance.ir.edn`：根层增加 p→q、p→r 的跨 rank 扇出，作为普通主干。App 从侧面
  出发的扇出线会穿过 v 框，弯段放不下。ui 子层的单 rank 四目标扇出只能容下 2 名成员，三条
  水平段的公共区间约 50px，扣掉两端弯段后为负，所以宽度断言从 ≥3 改为 ≥2。提案层仍然得到
  3 名成员。
- 原输入保留为反例：旧 shared-channel 输入在 `bundle-goldens/short-channel.ir.edn`，旧 lifecycle
  输入在 `bundle_lifecycle_short.ir.edn`，旧的一 rank 扇出跨层输入内联在测试里。
  `crates/scene/tests/short_channels.rs` 断言它们在根层、子层、提案和提案层里都没有 bundles 和
  branches，并保留基础 strokes。
- bundle_safety 的测试：扇出断言从 `[3]` 改为 `[2]`。扇入改用长跨度汇聚图，一 rank 的短扇入
  现在断言为不成束。rank-hop 改用有真实通道的布局，阻挡用例仍然保留基础路径。
