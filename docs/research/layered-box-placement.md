# 分层布局中框的横向定位：业界做法、撤回尝试的复盘与本仓建议（#185）

调研日期 2026-10-28。范围：`:levels` 布局里，每行顺序已经由重心排序定好之后，框和车道（lane，零宽虚拟节点）的
x 坐标怎样分配，才能让框靠近邻居、减少长距离横穿，同时不变宽、不新增穿框。拆边、虚拟节点、BK 拉直、network simplex
分层已在 [`layered-long-edge-layout.md`](layered-long-edge-layout.md) 中讨论过；展平与基准见
[`layered-edge-routing-performance.md`](layered-edge-routing-performance.md)。本文不重复这两篇的结论。

只引用一手来源（源码、论文）。源码引用指 2026-10-28 抓取的版本：

| 项目 | 分支 | commit |
| --- | --- | --- |
| ELK | master | b715fb1（2026-10-26） |
| dagre | master | 877ead2（2026-10-24） |
| Graphviz | main | 8cc0e9a（2026-10-28） |

没有拿到一手依据的地方标 **未核实**；本仓实测，以及由实测得出的推断，单独注明。

## 结论摘要

1. **撤回的那次尝试并没有消失。** git 对象里找不到它，但会话记录
   `~/.claude/projects/-home-lzx-dev-github-uml-viewer/3c75e0b2-….jsonl`（第 1501 条工具调用，2026-10-28 06:37 UTC）
   保存了完整补丁（函数 `straighten()`，`LANE_PULL = 8`，8 轮扫描），`/tmp/c2t/` 里留有它的 scene 导出
   （`base-*`、`s1-*`、`s2-*`）和补丁后的源码 `layout.rs.s1`。补丁打在 `4bed821` 之上：当时子行**没有上限**，
   foreign oval 也**还没有**下沉到 sink rank（`6ba7715` 之前）。所以 issue 里的 “950→1380px” 和 “companion 新增 3 条穿框”
   测的是那时的布局，不是 HEAD。
2. **companion 新增的穿框，根因是车道被推出了框的外包络，撞上了包框右侧的 foreign oval 列。** 把补丁重建出来并给
   `route.rs` 加日志后发现，4 条被拒的车道路径**全部**撞的是同一个 oval `:std`（rect x=1224, y=1954, 80×44）。`hits()`
   拒掉车道路径以后，`route()` 回退到 `candidates()`/`choose()`，选出一条单段斜线，穿过了 posix、herdr、recovery、
   preflight（§3.2）。车道本身没有撞框。
3. **变宽的原因是保序回归没有边界。** `straighten()` 只要求行内保持最小间距，没有把行限制在任何范围内；最宽的一行也会被
   邻居拉着整体平移，总宽度取的是所有行的 max(right) − min(left)。turn 视图里最宽的 rank 28 从 x=68 被推到 x=499（§3.1）。
   dot 用左右边界虚拟节点 `ln/rn` 和压缩边限制宽度，ELK 和 dagre 从 BK 的四种对齐里取最窄的一种：三者都显式控制了宽度（§1）。
4. **原型（本文 §4）：只拉框，行限制在最宽的紧排行之内，车道不施加拉力，只在两侧框之间的空隙里按顺序靠近自己的链。**
   在 fs-monopoly 和自举图的 14 个视图上：包框宽度**全部不变**（turn 仍是 944px），长边穿框为 0，全量测试（含 JVM goldens
   和 route goldens）通过，2000 类基准的耗时在噪声范围内。框中心横向距离之和：rules 降 23%，turn 降 15%，host 降 57%。
   代价是 companion 和 scene 两个视图的车道锯齿变多（§4.3）。
5. **用真实坐标重排行序**（重心排序和定位交替进行）会振荡，companion 的锯齿翻了 2–3 倍，不建议做。要真正对齐车道，需要
   ELK `LinearSegmentsNodePlacer` 的做法：把一条长边的车道链当作一个整体段，用段之间的依赖图保证各行次序一致，遇到环就
   拆段（§1.2）。这属于 “车道锯齿” 这个次要目标，可以放到下一步。

## 1. 各方案

### 1.1 Graphviz dot：辅助图上的 network simplex

- x 坐标的求法：把原图的每条边 e 换成一个 slack 节点 sn 和两条辅助边 sn→tail、sn→head，权重都是 `ED_weight(e)`。
  然后在辅助图上求 rank，求出来的 rank 就是 x（[gv-position] `position.c:341-366` `make_edge_pairs`）。
  最小化 Σ ω·|x(head) − x(tail)| 就是 “框靠近邻居” 的精确形式。
- 行内次序和最小间距：用 `make_LR_constraints` 给同一 rank 的相邻节点加 `len = 宽度和 + nodesep`、权重 0 的约束边
  （`position.c:224-269`）。
- **宽度控制**：`contain_nodes` 给图（或 cluster）建左右边界虚拟节点 `ln/rn`，把每个 rank 的首节点和末节点约束在两者之间
  （`position.c:1101-1125`，`make_lrvn` 在 `:1078-1095`）。cluster 内还会额外加一条 `ln→rn`、权重 128 的 “compaction edge”，
  把 cluster 往窄里压（`position.c:372-384`）。`ratio=compress` 时再加一条 `ln→rn`、长度为页面宽度、权重 1000 的边
  （`position.c:519-540` `compress_graph`）。这几处说明 dot 的宽度不是副产品，而是目标函数里显式的一项。
- 边权：[TSE93] §4 按端点类型给 Ω = 1/2/8（两端真实 / 一端虚拟 / 两端虚拟），用来抑制虚拟链的 “spaghetti effect”。
  前一篇已经注明当前源码 `mincross.c` 的表是 1/2/4。本次 PDF 文本没能提取，§4 原文没有重读，沿用前一篇的引用（**未重新核实**）。

### 1.2 ELK Layered 的三种节点放置器

- **NetworkSimplexPlacer**：注释明说实现的是 [TSE93] 的辅助图方法（[elk] `p4nodes/NetworkSimplexPlacer.java:66-77`）。
  边权表 `getEdgeWeight` 为 NORMAL–NORMAL 1、一端 NORMAL 2、两端都是 dummy 8（`:803-811`），与论文的 1/2/8 一致。
  注释还指出辅助图存在对称的最优解，会产生 “stair-case”，所以另外提供 `favorStraightEdges` 选项（`:83-106`）；
  `LONG_EDGE_VS_PATH_FACTOR = 2` 让直的长边优先于直的节点路径（`:196-200`，`:1063-1074`）。
- **BKNodePlacer**：BK 的四种方向组合各算一次（`p4nodes/bk/BKNodePlacer.java:56-57`）。不要求 balanced 时，取**占用空间最小**
  的那一种；要求 balanced 时取四者的中位数，并检查次序约束，不满足就退回最小的那一种（`:100-104`，`:236-250`）。
  type 1 conflict（短边与长边的内段交叉）总是按 “保长边竖直” 来解决（`:79-88`）。
- **LinearSegmentsNodePlacer**：注释写 “aligns long edges using linear segments”，出处是 Sander 的 GD’95 论文 §4
  （`p4nodes/LinearSegmentsNodePlacer.java:42-47`）。流程依次为：给 linear segment 排序 → 不平衡的初始放置 → 平衡 → 后处理
  （`:185-200`）。
  - 段的构成：一条长边的 LONG_EDGE dummy 首尾相连，组成**一个**段（`fillSegment`，`:466-500`）。所以同一条长边的所有
    dummy 必定共用一个坐标，结构上就不可能出现锯齿。
  - 段的次序：建一张段之间的依赖图并做拓扑排序；某条依赖会形成环时，就把那个段拆成两段（`:318-324`，`:337-395`）。
    issue 所说的 “每行的车道按重心各自排序，不同长边的车道在行与行之间互相交叉”，在这张依赖图里正好表现为环。
    ELK 的处理是拆段，不是硬对齐。
  - 平衡：先做 pendulum 迭代（只看一个方向的边），再做 rubber 迭代（两个方向都看）。每次迭代先算每段的 deflection，
    即相对相邻段的最优位移；**互相接触的段合并成 region，取 deflection 的加权均值后整体移动**（`:583-630`，`calcDeflection` 在
    `:671-735`，合并权重 `region1.weight + region2.weight` 在 `:780`）。这正是带最小间距的保序回归（PAVA 合并相邻违例块）的
    迭代版本，撤回的那次尝试属于同一类。区别在于 ELK 是**按段**合并，而不是按单个车道。
  - Sander GD’95 原文本文没有读到，只经由 ELK 注释转述（**未核实**）。

### 1.3 dagre `position/bk.ts`

- 水平压缩没有照搬原论文，而是自己建一张 block graph 做两遍扫描：第一遍把块放到尽量小的坐标，第二遍在不违反间距的前提下
  把块推向尽量大的坐标，回收多余空间（[dagre] `lib/position/bk.ts:283-287`）。注释说原论文的这一步 “存在若干问题”，
  与 [BWZ20] 指出的缺陷吻合（dagre 注释没有引用 BWZ20，两者的对应关系属推断）。
- 宽度：`findSmallestWidthAlignment` 从四种对齐里取 max(x+w/2) − min(x−w/2) 最小的一种（`bk.ts:401-421`）；
  `alignCoordinates` 把其余三种对齐到它的最小（左偏）或最大（右偏）坐标（`:424-432`），最后 `balance`（`:508-510`）。
  type 1 conflict 的标记在 `findType1Conflicts`（`:45`）。

### 1.4 Brandes–Köpf 与勘误

[BWZ20] §1–2：BK 在 proper layering 上给出线性时间的 x 坐标分配，保持行内次序和最小间距 δ；“vertical alignment” 与
“horizontal compaction” 两步按四种方向组合各做一次。原论文的两处错误都出在 compaction 这一步，修正后运行时间不变（摘要、§1）。
本文只读了摘要、§1 和 §2 开头，勘误的具体内容 **未核实**。issue 已明确暂不引入完整 BK，这里只用它说明 “对齐必须以全局一致的块为单位”。

### 1.5 Sugiyama 优先级布局法、Eiglsperger 长边段

Sugiyama 1981 年的 priority layout method（按优先级逐行把节点移向上下行邻居的重心，虚拟节点优先级最高），本文没有拿到原文
（**未核实**）。Eiglsperger 等的 segment 表示见前一篇 [ESK05]：长边以整段表示，把 dummy 开销降到线性。这与 ELK linear segment
的 “一条长边一个坐标” 方向一致。

## 2. 本仓现状（HEAD 9e02107）

- `place()`（`crates/scene/src/layout.rs:263-648`）：5 轮重心排序（`:527-533`），排序时的 `pos` 是**下标 ×80**，不是真实坐标。
  邻居 `nbr` 包含所有行的端点，也包含车道的前后节点（`:479`）。之后每行从 0 开始紧排，间距由 `gap()` 决定：
  框–框 40、车道–框 20（`LANE_MARGIN`）、车道–车道 12（`LANE_GAP`）（`:488-496`，`:577-623`）。
- `route()`（`crates/scene/src/route.rs:600-631`）：先用 `through_lanes` 走车道。只要 `hits()` 发现任何一段的**包围盒**与非端点框
  重叠，整条车道路径就会被丢掉，回退到 `candidates()` 的候选并由 `choose()` 挑选；`hits()` 检查的是轴对齐包围盒，不是线段本身
  （`route.rs:337-354`）。这一回退不报告、不计数。

### 2.1 实测（fs-monopoly 当前源码与本仓 `examples/uml-viewer.edn`）

方法：`target/release/uml-viewer ir policy.edn /tmp/r185/fs.edn`，再对每个视图执行 `scene --focus …`；从导出中统计：
类框外包宽度 W；每条边两端框中心的横向距离 |Δx| 之和（“横穿”）；路由折线的横向总行程减去首尾横向差（“锯齿”）；
以及 `scripts/route_crossings.py` 的长边穿框数。脚本在 `/tmp/r185/s/metrics.py`。

| 视图 | W | 边数 | Σ\|Δx\| | 最大 \|Δx\| | \|Δx\|>500 的边 | 锯齿 | 长边穿框 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| fs 根 | 280 | 33 | 1544 | 160 | 0 | 5742 | 0 |
| rules | 2106 | 45 | 34572 | 1699 | 31 | 833 | 0 |
| turn | 920 | 27 | 8360 | 800 | 4 | 1314 | 0 |
| host | 2496 | 15 | 10532 | 1664 | 7 | 4082 | 0 |
| 自举 companion | 1744 | 83 | 48328 | 1536 | 41 | 3432 | 0 |
| 自举 scene | 1568 | 58 | 28792 | 1268 | 26 | 1099 | **1** |
| 自举 viewer | 2832 | 88 | 64480 | 2492 | 50 | 96556 | 0 |

（turn 的包框宽度是 944px，即 issue 里说的 “约 950”。自举 scene 在 HEAD 上已经有 1 条长边穿框，用的是仓库里的
`examples/uml-viewer.edn`，这份文件可能落后于源码。）

rules 的形态很典型：rank 8 有 11 个框，紧排到 2106px；rank 6 只有 ports 和 board 两个框，紧排在 x=87 和 300。
于是 `endgame → ports` 的 |Δx| 是 1699。

## 3. 撤回尝试的复盘

补丁来源见摘要第 1 条。`straighten()` 的要点：邻接 `adj` 只连**相邻 rank**（`|kf−kt| == 1` 的边，加上车道链的各段）；
8 轮上下交替扫描，每次只看刚放好的那一行；目标是邻居中心的均值，车道权重 8，框权重 1，没有邻居的节点权重 0.1、目标取自身；
每行做带最小间距的 PAVA；最后整体平移，使最小左边界为 0。

### 3.1 为什么变宽

`/tmp/c2t/base-turn.edn` 与 `s1-turn.edn` 的 rank 28（choice, output, optional_action_choice，最宽的一行）：
左边界从 x=68 移到 x=499，而 rank 32 的 waits 仍在 x=68。总宽度由 952 变成 1383。PAVA 只保证行内间距，不限制行的位置；
最宽的行也会被拉走，其余各行又可以伸到它的另一侧。§1.1 和 §1.3 的三家都显式限制了宽度（dot 的 `ln/rn`，ELK/dagre 取最窄对齐），
`straighten()` 没有这一项。

### 3.2 为什么 companion 新增穿框

`s1-self-companion.edn` 中 3 条穿框的长边，`:points` 只有 4 个点：一条单段斜线，例如
`worker → record`：[579,774] → [912,3566]，中间没有任何车道点。这说明车道路径被拒，走了回退候选。

复现：在 `/tmp` 的一次性 worktree 里检出 `4bed821`，换上 `layout.rs.s1`，给 `route.rs` 的 `.filter(|ps| !hits(…))`
加上日志（`LANE_DEBUG`）。companion 视图输出 4 条拒绝记录，撞的**都是 `:std`**：

```
LANE-REJECT :companion.worker -> :companion.record seg [1256,1926]->[1256,2836] box :std (1224,1954,80,44)
LANE-REJECT :companion.role   -> :companion.vocab  seg [1230,1926]->[1230,2368] box :std
LANE-REJECT :companion.worker -> :companion.posix  seg [1242,1926]->[1242,2368] box :std
LANE-REJECT :companion.worker -> :companion.vocab  seg [1280,1170]->[1280,3570] box :std
```

因果链如下：

1. 在 `6ba7715` 之前，foreign oval 叠成一列，放在 `bounds.right() + 40` 处（`layout()` 末尾的 stacked 分支，至今仍在
   `layout.rs:888-924`）。`bounds` 是包框的并集，包框又只由**真实框和端口**算出（`layout.rs:818-853`），不含车道。
2. `straighten()` 以权重 8 把车道拉向链上邻居的均值。几条从右侧出发的长边，车道被拉到 x≈1230–1280，越过了最右边的真实框；
   这一带正是 oval 列所在的位置。
3. 车道的竖直段穿过 `:std` → `hits()` 为真 → 整条车道路径作废 → `choose()` 从 `candidates()` 里挑了一条没有车道的斜线，
   这条斜线穿过了中间几行的真实框。

所以这次穿框不是 “车道交叉” 造成的，而是**定位把车道放到了布局之外的障碍物上，而路由对车道失败的处理是静默回退**。
HEAD 上单个 `:levels` 包的 oval 已经下沉到 sink rank，这一具体碰撞不会再出现。但两个前提依然成立：包框和 oval 列只看真实框；
车道路径被拒时静默回退。多包视图里 oval 仍按旧方式叠在右侧（`layout.rs:762-771` 只对单包下沉）。（推断，依据上面的复现。）

### 3.3 车道没有对齐

`s2-rules.edn` 中 `choice_handlers → ports` 各行车道的 x 依次为 568、390、307、200、68、203、313…，与 issue 一致。
车道顺序在坐标分配之前就由重心排序定死了，排序用的是下标 ×80；坐标阶段只能在这个次序内移动。一旦两条长边的车道在相邻两行里
相对次序相反，任何保序的坐标调整都无法把它们同时拉直。ELK 把这种情况视为段依赖图中的环，并用拆段处理（§1.2）。

## 4. 原型：只拉框、限宽、车道填空

在 `/tmp` 的一次性 worktree（HEAD 9e02107）中实现了一个 `nearby()`，补丁 189 行，存于 `/tmp/r185/nearby-p2.diff`。
主仓库没有改动。

### 4.1 做法

- 只在 `levels && !horizontal` 时启用，插在 `gs` 算完之后、发射循环之前（对应 `layout.rs:544-565`）。发射循环里，用
  `start[id]` 取代累加的 `across`。非 `:levels` 布局走原路径，所以 JVM goldens 不受影响。
- 每行先按现有 `gap()` 规则算出紧排偏移 `offs` 和行宽；`wmax` = 所有行紧排宽度的最大值。
- 8 轮上下交替扫描。每个**框**的目标是它所有**框邻居**（任意行，即 `nbr` 中不是车道的项）中心的均值，权重为邻居数；
  没有邻居的框（如 grid 行）权重 0.01、目标取自身。sink rank 里的 oval 也按框处理。
- 每行在框的位移 z 上做加权 PAVA，然后把结果**截到 [0, wmax − 行宽]**。最宽的行 slack 为 0，钉死在原位，
  所以整体宽度不可能超过 HEAD。（对单调回归的解按统一上下界截断后仍是带界约束问题的最优解，这是保序回归的标准性质，
  本文未查一手出处。）
- **车道不施加拉力**：框的目标里不含车道。每一段夹在两个框之间的连续车道，保持次序和 12px 间距，在
  [左框右缘 + 20, 右框左缘 − 20] 这个区间内做一次 PAVA，目标是链上前后节点中心的均值。车道永远落在两侧框之间，
  落不到外包络之外，因此不会重演 §3.2。

### 4.2 结果（HEAD → 原型）

| 视图 | W | Σ\|Δx\| | \|Δx\|>500 | 锯齿 | 长边穿框 |
| --- | --- | --- | --- | --- | --- |
| fs 根 | 280 → 280 | 1544 → 1317 | 0 → 0 | 5742 → 4908 | 0 → 0 |
| rules | 2106 → 2106 | 34572 → 26556 | 31 → 25 | 833 → 2131 | 0 → 0 |
| turn | 920 → 920 | 8360 → 7108 | 4 → 3 | 1314 → 546 | 0 → 0 |
| app | 2064 → 2064 | 5048 → 4739 | 3 → 1 | 1765 → 205 | 0 → 0 |
| host | 2496 → 2496 | 10532 → 4538 | 7 → 3 | 4082 → 17 | 0 → 0 |
| ui | 1216 → 1216 | 14260 → 11942 | 10 → 7 | 1663 → 1591 | 0 → 0 |
| state | 1296 → 1296 | 2448 → 2173 | 1 → 0 | 1293 → 575 | 0 → 0 |
| foundation | 1602 → 1602 | 5267 → 4821 | 4 → 4 | 140 → 787 | 0 → 0 |
| config | 303 → 367 | 317 → 245 | 0 → 0 | 116 → 260 | 0 → 0 |
| 自举根 | 508 → 508 | 2444 → 1575 | 0 → 0 | 2098 → 2267 | 0 → 0 |
| companion | 1744 → 1744 | 48328 → 36383 | 41 → 30 | 3432 → 9290 | 0 → 0 |
| scene | 1568 → 1568 | 28792 → 21101 | 26 → 18 | 1099 → 5767 | 1 → 0 |
| viewer | 2832 → 2832 | 64480 → 46413 | 50 → 39 | 96556 → 42603 | 0 → 0 |
| ir | 624 → 624 | 3412 → 2892 | 0 → 0 | 1614 → 2028 | 0 → 0 |

- 包框宽度（`:packages` 的 `:rect :w`）逐项核对：turn 944/944，rules 2130/2130，config 438/438，fs 根 304/304，
  companion 1768/1768，viewer 2856/2856，全部不变。config 的类框外包宽度多了 64px，但仍在包框之内（包框还含端口）。
- issue 点名的边：`endgame → ports` 1699 → 796，`land → board` 1287 → 302，`items → board` 786 → 199；
  `actions → output` 640 → 536，`deadlines → output` 320 → 216。rules 的 ports、board 从 x=87/300 移到 990/1285。
  turn 的 rank 14 有 6 个框，已经紧排到 920，是最宽的一行，钉死不动，所以 turn 的改善有限。
- fs 根视图 `app → foundation` 车道 x：276、240、276、380 → 347、332、290、308、380，锯齿基本没变（次要目标，未处理）。
- 测试：`cargo test --workspace --locked`（debug）全部通过，含 `fixtures/jvm-goldens` 与 route goldens。
- 基准：`synthetic-2000.edn` 每个 class 都带 `:level`，所以会走新路径。release CLI 各跑 3 次：HEAD 2.75/3.06/2.91 s，
  原型 3.15/2.81/2.66 s，峰值 RSS 282→283 MB；`bench_scene` release 报告 2.862 s。差异在噪声内。`nearby()` 每轮为
  O(行内节点 + 邻接)，8 轮共 O(8·(V + E + 车道数))。

### 4.3 代价与否定的变体

- companion 锯齿 3432 → 9290，scene 1099 → 5767。逐边检查：companion 中 `mailbox → :std` 的车道 x 为 1740、1493，
  而它的端点在 753 和 788。这条车道被重心排序（用下标 ×80）排在了行末，右侧没有框，区间上界就是 wmax，PAVA 只能停在
  左框右缘附近。车道次序是在坐标之前定的，这是 §3.3 同一个根源。
- 变体 p3：定位后把每行按真实中心重新排序，再定位一次，做 1–3 轮。结果振荡：rules Σ\|Δx\| 26556 → 28447–31789，
  companion 锯齿 14455–25032，viewer 宽度 2832 → 2904。**否定。**
- 变体 p1：车道放在两侧框位移的中点，不向链靠拢。它的框位置与 p2 相同，但 host 锯齿 3636（p2 为 17），viewer 51953（p2 为 42603）。
  p2 更好。

## 5. 与约束的对照

| 约束 | 原型的情况 |
| --- | --- |
| 只改 `:levels` 布局 | 入口条件是 `levels && !horizontal`；非 level 路径的代码完全不变 |
| JVM goldens 1e-6 | 全量测试通过（实测） |
| `route_crossings.py` 0 穿框 | 14 个视图长边穿框全为 0，scene 由 1 降到 0（实测）；其他类别也是 0 |
| 2000 类基准不变慢 | 在噪声内（实测，机器负载未控制） |
| 人工截图验收 | **未做** |

## 建议

**车道是否参与定位：** 车道继续参与**排序**，因为它要预留槽位，`5db3fa6` 的 0 穿框保证依赖这一点；但在**坐标**上，车道不给框施加任何拉力。
框只看框邻居，各行按带界 PAVA 定位；之后车道在两侧框之间的区间里按既有次序和 12px 间距放置，靠近链上前后节点（§4.1 的 p2）。
这样框的位置只由真实依赖决定，issue 的主目标（rules −23%，host −57%）能直接拿到，车道也永远落在两侧框之间。
dot 和 ELK NetworkSimplexPlacer 给虚拟链更高权重（1/2/8），是为了拉直，这正是 issue 已经降为次要的目标。如果以后要处理锯齿，
应该按 ELK `LinearSegmentsNodePlacer` 的方式，把一条长边的车道链做成一个段，用段依赖图（遇环拆段）保证各行次序一致，
不要再在单个车道上加权。

**如何不变宽：** 先把每行紧排，取最宽一行的宽度 wmax；每行的 PAVA 解截到 [0, wmax − 行宽]，最宽的行 slack 为 0、原地不动。
这对应 dot 的 `ln/rn` 边界节点，以及 ELK 和 dagre “取最窄对齐” 的思路。宽度上界在构造上就成立：实测 14 个视图包框宽度全部不变，
turn 仍是 944px。代价是最宽那一行本身不会改善（turn 的 rank 14、rules 的 rank 8）。真要改善，只能动行序或分层，不属于定位阶段。
建议给 `place()` 加一条单元测试：启用 levels 后，任一行的范围不超出 wmax。

**为什么撤回的尝试新增穿框：** 不是车道交叉，而是定位把车道推出了真实框的外包络。当时（`4bed821`，oval 尚未下沉）foreign oval
叠在 `bounds.right() + 40` 处，`bounds` 只由真实框算出；被权重 8 拉到右边的车道撞上 `:std`，`route()` 的 `hits()` 拒掉整条车道路径，
静默回退成一条斜线，斜线穿过了 posix、herdr、recovery、preflight（§3.2 的复现日志）。新做法里车道被夹在两侧框之间，不会再越界。
另外建议让 `route()` 统计 “有车道却被回退” 的次数，测试里要求为 0，把这类问题从静默变为可见。

## 参考文献

- [TSE93] Gansner, Koutsofios, North, Vo. *A Technique for Drawing Directed Graphs*. IEEE TSE 19(3), 1993, §4. https://graphviz.org/documentation/TSE93.pdf
- [gv-position] Graphviz `lib/dotgen/position.c`（8cc0e9a）。https://gitlab.com/graphviz/graphviz/-/blob/main/lib/dotgen/position.c
- [elk] ELK `plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/`：`NetworkSimplexPlacer.java`、`LinearSegmentsNodePlacer.java`、`bk/BKNodePlacer.java`（b715fb1）。https://github.com/eclipse-elk/elk
- [dagre] dagrejs/dagre `lib/position/bk.ts`（877ead2）。https://github.com/dagrejs/dagre/blob/master/lib/position/bk.ts
- [BK01] Brandes, Köpf. *Fast and Simple Horizontal Coordinate Assignment*. GD 2001, LNCS 2265, 31–44.（见前一篇）
- [BWZ20] Brandes, Walter, Zink. *Erratum: Fast and Simple Horizontal Coordinate Assignment*. arXiv:2008.01252, 2020. https://arxiv.org/abs/2008.01252 （读了摘要、§1、§2 开头）
- [Sander96] G. Sander. *A fast heuristic for hierarchical Manhattan layout*. GD’95, LNCS 1027, 447–458, 1996.（经 ELK 注释转述，未读原文）
- [ESK05] Eiglsperger, Siebenhaller, Kaufmann. JGAA 9(3), 2005.（见前一篇）
- Sugiyama, Tagawa, Toda. *Methods for Visual Understanding of Hierarchical System Structures*. IEEE SMC 11(2), 1981.（priority layout method，未读原文）
- 撤回尝试的记录：`~/.claude/projects/-home-lzx-dev-github-uml-viewer/3c75e0b2-b0c4-4bc7-a03f-a3771dbfd677.jsonl`（第 1501–1597 条），导出 `/tmp/c2t/`。
