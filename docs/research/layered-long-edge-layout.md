# 分层图中长边的布局与路由：业界做法与本仓对照

调研日期 2026-10-28。范围：Sugiyama 类分层布局（Sugiyama-style layered layout）里，跨多个 rank 的边怎样
避免穿过类框、怎样保持可读。源码引用指 2026-10-28 抓取的各项目默认分支；行号可能随上游变化。
只引用一手来源（论文、官方文档、源码）；没有拿到一手依据的地方标 **未核实**。

## 结论摘要

1. **所有主流实现都先把长边拆成虚拟节点（virtual/dummy node），再做排序和坐标分配。** Graphviz dot、
   ELK Layered、dagre、OGDF 都是这样做的；yFiles 文档的距离语义也指向同一模型（实现闭源，细节未核实）。
   跨 k 个 rank 的边在每个中间 rank 都占一个槽位，参与交叉最小化和坐标分配。路由只需穿过这些预留槽位，
   “不穿框”靠构造保证，而不是事后搜索。本仓 `place()` 没有这一步，`route()` 只能在已经排满的 rank 里
   找缝（`hops` 取起点和终点的中点 x）。#172（共用通道的线束）和 #183（rank 内 span 重排）都是在
   **没有预留槽位**的布局上做的事后补救，这是它们失败的结构性原因。
2. **长边要尽量少、尽量短、尽量直，分别由三个阶段负责。** 分层阶段用 network simplex 使
   Σ ω·span 最小（dot、ELK、dagre 的默认做法）。排序阶段让 dummy 参与重心排序。坐标分配阶段要么给
   dummy–dummy 段很高的权重（dot 论文用 1/2/8），要么用 Brandes–Köpf 把内段对齐成竖直线。
   这三步都建立在拆边的前提上。
3. **在本仓自己的图上，穿框几乎全部来自 foreign oval 的边，不是多 rank 长边**（实测见 §2.2）。
   foreign oval 被放在包框右侧的一列，不在分层之内。dot 的 `rank=sink` 和 ELK 的
   `layerConstraint=LAST` 都把这类节点作为普通节点纳入分层，这是本仓最便宜、收益最大的一步。
4. **`:level` 模式下，下钻视图整行只有一个 rank**：同一 crate 的类共用一个 level，结果全部排成一行，
   边全是同 rank 的平边。ELK 的 partitioning 和 dot 的 cluster 都把这种外部分层当作**约束**，
   在约束内部仍按依赖细分层。本仓则直接用 level 代替了分层。
5. **线束 / 边集中只能建在 dummy 之上。** dot 的 `concentrate` 在 mincross 之后、x 坐标分配之前合并
   **相邻的虚拟节点**（`conc.c`），合并后的通道在定位时自然分到空间。如果要重提线束，必须先完成第 1 条。

## 1. 各方案

### 1.1 Graphviz dot（Gansner, Koutsofios, North, Vo 1993）

- **分层**：第一遍用 network simplex 求最优 rank 分配，目标是 Σ ω(e)·(λ(head)−λ(tail)) 最小，
  即加权总跨度最小（[TSE93] 摘要）。源码入口是 `rank1()`，对每个连通分量跑 network simplex
  （[gv-rank] `rank.c:452-458`）。
- **虚拟节点**：`make_chain()` 为跨越的每个中间 rank 创建一个 virtual node，并用 virtual edge 串起来
  （[gv-class2] `class2.c:68-96`，注释 “create chain of dummy nodes for edge orig”）。virtual node
  和真实节点一起参与 mincross。
- **拉直**：x 坐标分配被转成辅助图上的 network simplex。论文按端点类型给边权 Ω=1、2、8
  （两端真实、一端虚拟、两端虚拟），理由是：真实节点之间的短边总能画直，更要紧的是把虚拟节点链竖直对齐，
  否则会出现 “spaghetti effect”（[TSE93] §4，[BK01] 转述了同一组权重）。当前源码的权表是 1/2/4
  （`C_EE/C_VS/C_VV`，[gv-mincross] `mincross.c:1695-1731`），与论文数值不同。
- **路由**：每条边先得到一串必须穿过的矩形区域：尾端附近几个框，然后是 **rank 间框与 virtual node 框交替**，
  最后是头端附近几个框；样条（spline）在这串框内拟合（[TSE93] §5）。源码在 `make_regular_edge`
  中沿虚拟链依次追加 `rank_box()` 和 `maximal_bbox()`（[gv-splines] `dotsplines.c:1771-1796`）。
  `maximal_bbox` 把 “直到左右邻居为止的全部可用空间” 交给虚拟节点（`dotsplines.c:2175-2185`）。
  对齐的虚拟链走 `straight_path` 直线段（`dotsplines.c:1779-1830`）。
- **对穿框的效果**：只要虚拟节点不与真实节点重叠，路由区域本来就不含任何类框；穿框基本不会发生。
  `splines=ortho` 是另一套迷宫式路由（`lib/ortho`），官方说明它在 dot 中不处理 port 和边标签
  （[gv-splines-attr]）。
- **可用旋钮**：`minlen`（最小 rank 差）、`constraint=false`（该边不参与分层）、`rank=same/min/max/source/sink`
  （[gv-rank-attr]）、`newrank`（忽略 cluster 的全局分层）、`concentrate`。`concentrate` 会合并
  multiedge，并让部分平行的边共享一段路径（[gv-conc-attr]）。其实现是在 `dot_position` 一开始、
  x 定位之前合并同方向的相邻虚拟节点（[gv-position] `position.c:126-131`；[gv-conc] `conc.c:202-237`）。
- **复杂度**：dummy 数等于 Σ(span−1)。Eiglsperger 等指出，大量使用 dummy 会让整个 Sugiyama 流程在最坏情况下
  耗时 O(|V||E| log|E|)、内存 O(|V||E|)；他们提出的 segment 技术能把 dummy 数降到线性（[ESK05] 摘要）。
  network simplex 的理论上界本文未核实。

### 1.2 Eclipse ELK Layered

- **管线**：cycle breaking → layering → crossing minimization → node placement → edge routing。
  默认路由是 ORTHOGONAL，默认分层是 NETWORK_SIMPLEX，默认节点放置是 BRANDES_KOEPF（[elk-layered]）。
- **分层策略**：可选 `NETWORK_SIMPLEX`、`LONGEST_PATH`、`LONGEST_PATH_SOURCE`、`COFFMAN_GRAHAM`、
  `INTERACTIVE`、`STRETCH_WIDTH`、`MIN_WIDTH`、`BF_MODEL_ORDER`、`DF_MODEL_ORDER`（[elk-layering]）。
  `NetworkSimplexLayerer` 的注释写明 “optimal layering … concerning a minimal length of all edges”，
  并引用 [TSE93]（[elk-src] `p2layers/NetworkSimplexLayerer.java:42-48`）。
- **拆边**：`LongEdgeSplitter` 为每条跨多层的边插入 dummy，使分层成为 proper layering，在 phase 3 之前运行
  （`intermediate/LongEdgeSplitter.java:35-58`）。`LongEdgeJoiner` 在 phase 5 之后删除 dummy，
  把 dummy 位置变成原边的折点（`intermediate/LongEdgeJoiner.java:34-58`）。
- **拉直**：`BKNodePlacer` 基于 [BK01]。第一步标记 “短边与长边交叉” 的 type 1 conflict，解决冲突时
  保长边竖直，使同一长边的 dummy 落在同一个 block 里（`p4nodes/bk/BKNodePlacer.java:44-55, 79-88`）。
  `bk.edgeStraightening=IMPROVE_STRAIGHTNESS`（默认）在四种对齐中尽量多拉直边（[elk-bk-straight]）。
- **正交路由**：`OrthogonalEdgeRouter` 基于 Sander 的正交超边算法（[Sander03]），前提是图已经
  proper layered 且节点、port 已定位（`p5edges/OrthogonalEdgeRouter.java:39-57`）。由于拆过边，
  路由只在**相邻两层之间的空隙**里分配水平段；穿过中间层的部分就是 dummy 的位置，这个位置在节点放置时
  已经留好。相关间距项有 `spacing.edgeNodeBetweenLayers`、`spacing.edgeEdgeBetweenLayers`
  （[elk-layered] 选项表）。
- **约束**：`layering.layerConstraint` 取 `FIRST/FIRST_SEPARATE/LAST/LAST_SEPARATE`（[elk-layerconstraint]）。
  partitioning：分区号小的节点一律排在前面（[elk-partition]）。实现方式是在分层前加约束边，分层仍在分区内
  正常进行（`intermediate/PartitionMidprocessor.java:36-42`）。另有 Coffman–Graham 的 `layerBound`，
  它限制的是层**宽**，不是跨度（[elk-layered] 选项表）。

### 1.3 dagre 与 Mermaid

- dagre 的 `normalize.run` 把长边拆成每段只跨 1 层的 dummy 链（[dagre-src] `lib/normalize.ts:9-23`）。
  `undo` 把每个 dummy 的 (x, y) 依次推入原边的 `points`（`normalize.ts:72-90`），所以 dagre 输出的边
  折线**就是** dummy 坐标序列。
- 分层默认 `network-simplex`，也可选 `longest-path`、`tight-tree`（`lib/rank/index.ts:30-60`）。
  坐标分配基于 Brandes–Köpf（`lib/position/bk.ts:5-7`）。
- Mermaid 自 v12.0.0 起，flowchart、state、class、ER 等图默认使用打包好的 ELK 布局，
  dagre 需要写 `layout: dagre` 才会启用（[mermaid-layouts]）。两条路径都会拆边。

### 1.4 yFiles Hierarchic（官方开发者指南，yFiles for Java 2.13，旧版但官方）

- 三个阶段：layering → sequencing → drawing（坐标与路由）（[yfiles-ihl]）。
- 分层策略里，`HIERARCHICAL_OPTIMAL` 使所有边的层距之和最小（WeightedLayerer）。`BFS` 的说明是
  “All edges will span at most one layer … Edges between nodes that belong to the same layer are
  possible”，这正是本仓 BFS 分层的形态，代价是会出现大量同层边。
- 距离项里的 “Node to Edge Distance” 指 “同一层内节点与不相邻边之间的最小距离”，并注明 “只对至少跨一层的边生效”。
  这说明长边在中间层占有位置。内部是否用 dummy 实现，因源码闭源未核实。
- 端点处可以做边分组（bus-style edge grouping）：在源或目标端合并路径（[yfiles-ihl] Edge/Port Grouping）。

### 1.5 PlantUML

默认调用 Graphviz dot；`!pragma layout smetana` 改用 dot 的 Java 移植 Smetana（[puml-smetana]）；
`!pragma layout elk` 改用 ELK，自 V1.2024.6 起 ELK 已打包进 plantuml.jar（[puml-elk]）。
所以 PlantUML 类图的长边行为就是 dot 或 ELK 的行为。“用 `--->` 箭头加长边以增加 rank 差” 的说法
本文未在官方文档中核实。

### 1.6 OGDF

- `SugiyamaLayout` 默认用 `LongestPathRanking` 分层（[ogdf] `SugiyamaLayout.h:140-147`）。
  `OptimalRanking` 用 LP 求边长最小的分层（`OptimalRanking.h:44-50`）。
- `FastHierarchyLayout`（Buchheim、Jünger、Leipert，GD 2000）保证每条边至多两个弯折，且两弯折之间的段竖直，
  特别适用于长边（`FastHierarchyLayout.h:45-60`）。`FastSimpleHierarchyLayout` 就是 [BK01]。
  `OptimalHierarchyLayout` 用 LP 和 `weightSegments=2.0` 避免 spaghetti 效果（`OptimalHierarchyLayout.h:45-80`）。

### 1.7 论文

- **[BK01] Brandes–Köpf**：线性时间的水平坐标分配。“inner segment”（两端都是 dummy 的段）被要求竖直；
  算法在上下、左右四个方向各做一次 vertical alignment 和 horizontal compaction，再做平衡（[BK01]，
  [BWZ20] §2）。**[BWZ20] 勘误**指出原算法有两处错误：一处已被多次修正，另一处需要非平凡的改动；
  修正后运行时间不变。照原论文实现时必须参照这份勘误。
- **[Sander03]**：分层的正交超边布局，ELK 正交路由的基础。
- **[ESK05]**：长边以 segment 表示，把 dummy 开销降到线性。图大时值得采用。
- **局部边绘制（Partial Edge Drawing, PED）**：去掉边的中段，只画两端的 stub（[BK12]）。

### 1.8 其他手段

| 手段 | 一手出处 | 说明 |
| --- | --- | --- |
| rank 约束 | dot `rank=`，ELK `layerConstraint`/partition，yFiles `LayerConstraintFactory` | 把汇点类节点（如 foreign）固定在首层或末层 |
| 不参与分层的边 | dot `constraint=false` | 让少数边不拉长分层 |
| 跨度上限 | 未找到 | dot 的 `minlen` 是下限；ELK 的 `layerBound` 限制的是宽度。主流工具里没有找到 max-span 选项（未核实） |
| 长边去强调 | 未找到工具内置项 | 属于绘制层面的选择（变淡、变细），没有找到一手的布局算法依据 |
| 剪断 + 端口标签 | [BK12] PED | 本仓已有带依赖名的 in-ports/out-ports 标签（`layout.rs:182-226`），可以直接复用 |

## 2. 与本仓现状对照

### 2.1 现状（`crates/scene/src/layout.rs`、`route.rs`）

- **分层**：从入度为 0 的类出发做 BFS（`layout.rs:271-295`）。对 s→t，t 最晚在 rank(s)+1 被发现，
  所以 **BFS 模式下向下的边从不跨两层以上**。长边只有两类：一是向上的回边，二是从根不可达的节点
  （默认 rank 0，`layout.rs:311`）。这与 yFiles 的 BFS layering 描述一致。
- **`:level` 模式**：只要有类带 `:level` 且方向是 tb，rank 就直接取 level（`layout.rs:296-312`）。
  level 来自 policy 的层（`ir/src/policy.rs:418-436`，`scene/src/hierarchy.rs:188-196`）。
  在下钻视图里，同一 crate 的类共享一个 level，于是整行只有一个 rank。实测 `[:scene]` 视图中
  18 个类的 y 都是 100。
- **排序**：5 轮重心排序，邻居包括所有 rank 的类，没有 dummy（`layout.rs:328-364`）。
- **坐标**：每行从左往右紧排，横向间距 40、rank 间距 140（`layout.rs:379-417`），没有独立的坐标分配阶段。
- **foreign oval**：放在包框右侧单独一列，不参与分层（`layout.rs:520-558`）。
- **路由**：`hops` 在每个中间 rank 取起点与终点的中点 x（`route.rs:237-262`），没有预留空位。所有候选
  都被挡住时，退回第一个候选（`route.rs:367-368`），这就是穿框的来源。

### 2.2 实测（本仓自举图）

方法：`target/release/uml-viewer scene --ir examples/uml-viewer.edn --focus <f>`（读的是工作区里
已修改的 EDN），统计每条边的路由折线 `:points` 与非端点类框（含 oval）的相交。不计端口和曲线化后的形状。

| 视图 | 同 rank 边 / 穿框次数 | 跨 rank 边 / 穿框次数 | oval 边 / 穿框次数 |
| --- | --- | --- | --- |
| 根 `[]` | 0 / 0 | 6 / 1 | 9 / 14 |
| `[:scene]` | 54 / 0 | 0 / 0 | 11 / 27 |
| `[:viewer]` | 64 / 0 | 0 / 0 | 26 / 139 |
| `[:companion]` | 71 / 0 | 0 / 0 | 12 / 13 |
| `[:ir]` | 17 / 0 | 0 / 0 | 5 / 10 |

在本仓的图上，穿框几乎全部来自 oval 边。#183 证据中 fs-monopoly 根层的 “左侧纵向通道拥挤来自跨多个 rank
的长边” 是另一类图形态。那份 IR 不在仓库里，本文没有复测。

### 2.3 每个方案对应的改动

| 编号 | 改动 | 对应业界做法 | 改哪里 | 代价 | 对 goldens 的影响 |
| --- | --- | --- | --- | --- | --- |
| A | foreign oval 纳入分层：放入末 rank（或依赖者所在 rank 的下一行），按邻居重心排序 | dot `rank=sink`；ELK `layerConstraint=LAST` | `layout()` 的 oval 段并入 `place()` | 小 | `proposal` route golden 和所有含 foreign 的 JVM geometry golden 都会变 |
| B | proper layering：对每条 \|span\|≥2 的包内边，在中间 rank 插入 dummy 槽位（宽约 10–20px），参与重心排序；把 dummy 坐标作为该边的 `:waypoints` 输出；`route()` 对这类边按 waypoints 正交化（竖直穿过槽位，水平段落在 rank 间隙），不再走 `candidates()` | dot `make_chain`；ELK `LongEdgeSplitter` 和 `LongEdgeJoiner`；dagre `normalize` | `place()` 和 `route()`，scene 里新增一个边字段 | 中 | `rank-hop`、`proposal` route golden 和多 rank 的 JVM golden 都会变 |
| C1 | 分层由 BFS 改为 longest-path 加 network simplex（先用 DFS 反转回边破环） | dot、ELK、dagre 的默认做法 | `place()` 的 271-295 行 | 中 | 几乎所有 golden 都会变 |
| C2 | 把 `:level` 当分区约束，在 level 内按依赖再细分 sub-rank | ELK partitioning；dot cluster 内分层 | `place()` 的 296-320 行 | 中 | `proposal` 和带 level 的 golden 会变 |
| D | Brandes–Köpf（按 [BWZ20] 修正）或 dot 式加权坐标分配，把 dummy 链拉直 | ELK `BKNodePlacer`；dagre `bk.ts`；OGDF | `place()` 的 379-417 行，替换紧排逻辑 | 中到高，依赖 B | 坐标全部变化 |
| E | 只改 `route()`：`hops` 不再取中点，而是在每个中间 rank 的空隙中选离中点最近的空位 | 无直接对应（事后搜索） | `route.rs:237-262` | 小 | `rank-hop` 可能变化 |
| F | 剪断：\|span\|≥k 或 oval 边只画端口标签 stub，悬停或选中时再显示完整路径 | PED [BK12] | 只改绘制和 scene 样式字段；`:points` 不变 | 小 | route goldens 不受影响 |

关于 E：它仍属于事后补救，没有预留槽位。在密集的行里会和现在一样无路可走，只能作为 B 落地之前的过渡。
关于 #172 / #183：以下是推断，不是一手结论。#183 只在真实节点之间最小化横向跨度，排序时看不到长边要穿过的
中间 rank，所以无法为它让出空位。#172 在路由之后才合并通道，合并出的通道同样没有被定位阶段分配过空间。
dot 的 `concentrate` 正好相反：它在定位之前合并虚拟节点。

所有布局改动都建议沿用 #183 的做法，保留一个 `Base` 布局变体，让 JVM 和 route goldens 继续按旧几何比对，
新布局另加测试。验收指标可以直接沿用 §2.2 的 “折线穿过非端点框次数”。

## 3. 推荐顺序

1. **A：oval 纳入分层。** 在本仓图上，它覆盖了实测穿框的绝大部分，改动最小。
2. **C2 + C1：** 先把 level 改成约束并在 level 内细分（解决下钻视图 “一整行” 的问题，也就是 #183 证据里
   横贯整行的长弧），再把 BFS 换成 longest-path 或 network simplex，消除向上的长回边。
3. **B：dummy 槽位加 waypoint 路由。** 这是根治多 rank 长边穿框的通用做法，也是之后任何线束方案的前提。
4. **D：BK 拉直。** 在 B 之上让长边变成竖直直线，消除 spaghetti 效果。
5. **F** 作为可选视图模式随时可加。**E** 只在 B 推迟时作为过渡。

## 参考文献

- [TSE93] Gansner, Koutsofios, North, Vo. *A Technique for Drawing Directed Graphs*. IEEE TSE 19(3), 1993. https://graphviz.org/documentation/TSE93.pdf
- [gv-class2] Graphviz `lib/dotgen/class2.c`. https://gitlab.com/graphviz/graphviz/-/blob/main/lib/dotgen/class2.c
- [gv-rank] Graphviz `lib/dotgen/rank.c`. https://gitlab.com/graphviz/graphviz/-/blob/main/lib/dotgen/rank.c
- [gv-mincross] Graphviz `lib/dotgen/mincross.c`. https://gitlab.com/graphviz/graphviz/-/blob/main/lib/dotgen/mincross.c
- [gv-splines] Graphviz `lib/dotgen/dotsplines.c`. https://gitlab.com/graphviz/graphviz/-/blob/main/lib/dotgen/dotsplines.c
- [gv-position] Graphviz `lib/dotgen/position.c`. https://gitlab.com/graphviz/graphviz/-/blob/main/lib/dotgen/position.c
- [gv-conc] Graphviz `lib/dotgen/conc.c`. https://gitlab.com/graphviz/graphviz/-/blob/main/lib/dotgen/conc.c
- [gv-splines-attr] https://graphviz.org/docs/attrs/splines/ ；[gv-conc-attr] https://graphviz.org/docs/attrs/concentrate/ ；[gv-rank-attr] https://graphviz.org/docs/attrs/rank/ ；newrank https://graphviz.org/docs/attrs/newrank/ ；constraint https://graphviz.org/docs/attrs/constraint/ ；minlen https://graphviz.org/docs/attrs/minlen/
- [elk-layered] ELK Layered 参考. https://eclipse.dev/elk/reference/algorithms/org-eclipse-elk-layered.html
- [elk-layering] https://eclipse.dev/elk/reference/options/org-eclipse-elk-layered-layering-strategy.html
- [elk-bk-straight] https://eclipse.dev/elk/reference/options/org-eclipse-elk-layered-nodePlacement-bk-edgeStraightening.html
- [elk-layerconstraint] https://eclipse.dev/elk/reference/options/org-eclipse-elk-layered-layering-layerConstraint.html
- [elk-partition] https://eclipse.dev/elk/reference/options/org-eclipse-elk-partitioning-partition.html
- [elk-src] ELK 源码根：https://github.com/eclipse-elk/elk/tree/master/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered （上文路径均相对此根）
- [dagre-src] https://github.com/dagrejs/dagre/tree/master/lib （`normalize.ts`、`rank/index.ts`、`position/bk.ts`）
- [mermaid-layouts] https://mermaid.js.org/config/layouts.html
- [yfiles-ihl] yFiles for Java 2.13 Developer's Guide, Hierarchical Layout Style. http://docs.yworks.com/yfiles/doc/developers-guide/incremental_hierarchical_layouter.html
- [puml-elk] https://plantuml.com/elk ；[puml-smetana] https://plantuml.com/smetana02
- [ogdf] https://github.com/ogdf/ogdf/tree/master/include/ogdf/layered
- [BK01] Brandes, Köpf. *Fast and Simple Horizontal Coordinate Assignment*. GD 2001, LNCS 2265, 31–44. https://kops.uni-konstanz.de/bitstreams/e3f1cd1e-3fd4-422d-852e-77404160f664/download （DOI 10.1007/3-540-45848-4_3）
- [BWZ20] Brandes, Walter, Zink. *Erratum: Fast and Simple Horizontal Coordinate Assignment*. https://arxiv.org/abs/2008.01252
- [Sander03] Sander. *Layout of Directed Hypergraphs with Orthogonal Hyperedges*. GD 2003, LNCS 2912. https://link.springer.com/chapter/10.1007/978-3-540-24595-7_35
- [ESK05] Eiglsperger, Siebenhaller, Kaufmann. *An Efficient Implementation of Sugiyama's Algorithm for Layered Graph Drawing*. JGAA 9(3), 2005. https://jgaa.info/index.php/jgaa/article/view/paper111
- [BK12] Bruckdorfer, Kaufmann. *Mad at Edge Crossings? Break the Edges!* FUN 2012. https://link.springer.com/chapter/10.1007/978-3-642-30347-0_7
