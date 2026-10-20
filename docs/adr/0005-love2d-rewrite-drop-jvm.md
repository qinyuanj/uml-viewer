# 0005 Drop the JVM: rewrite viewer and IR generation in love2d + cimgui-love

**Status**: accepted

无论被分析项目是什么语言，用户都必须安装 Java 21+ 和 Clojure CLI，因为 viewer、IR 生成和 Herdr preflight 都跑在 JVM 上。非 Clojure 项目的质量工具早已是 Python（crapper / mutator），JVM 只为 uml-viewer 自身存在，冷启动还拖慢首次启动和 `--restart`。因此改用 love2d + cimgui-love（LuaJIT FFI 绑定 Dear ImGui）完全重写 viewer 与 IR 生成，整条链路不再依赖 JVM。规格见 #50。

同时放弃 Clojure 语言支持，不提供过渡 tag。保留 Clojure 就要保留 JVM：Clojure 扫描依赖 `clojure.core/read`，crap4clj / clj-mutate 要运行被测代码，留着它就达不到去掉 JVM 的目的。手写一个 Lua 版 Clojure reader 只能省掉扫描这一步，质量工具仍然要用 JVM。

mail、IR、policy、metrics 的 EDN 协议，以及 companion / Herdr 契约保持不变；PowerShell 7 wrapper 保留，只把启动命令换成 `love`。

**修订（2026-10-21，spec #106）**：上句"PowerShell 7 wrapper 保留"由 ADR 0007 取代——
交付平台改为 WSL2 + WSLg 与 macOS Apple Silicon，公共入口改为 POSIX shell + Python，
不再维护 Windows 原生实现或 PowerShell 入口。本 ADR 其余决定（放弃 JVM、保留
LOVE/cimgui-love、LuaJIT 与 Lua 5.4 双解释器验证纯逻辑）继续有效。新旧实现并存到功能对等后再删除 JVM 实现，切换日本仓库改为 `:lang :lua`，用 crapper / mutator 自举。

## Considered Options

- **纯 love2d**：Lua 适合移植 Clojure 风格的动态代码，但 inspector、列表、弹窗、输入框、IME 都得手写。
- **Dear ImGui C++**：控件齐全、性能上限最高，但约 4,000 行纯逻辑要设计静态类型，每次修改都要编译，CI 要为每个平台出二进制。这个场景用不上它的性能。
- **love2d + cimgui-love**（选中）：ImGui 提供控件，同时保留 Lua 免编译、便于移植的优点。代价是每个平台要随附一个 cimgui 动态库。

## Consequences

- mutator 用原生 Lua 5.4 加载 busted，而 love 跑 LuaJIT（Lua 5.1 语义）。纯逻辑模块只用两者的公共子集，CI 在两种解释器下都跑 busted。只有 cimgui / FFI 适配层依赖 LuaJIT；适配层没有质量数据时按规则显示红色，不用 `:omit` 隐藏。
- love 没有进程身份 API，PID 和启动时间通过 FFI 获取，写入 `viewer-process-identity.json`。并存期 wrapper 同时识别它和 `viewer-jvm.json`。
- 现有 Clojure 用户升级后将无法使用；需要 Clojure 支持的用户只能自行固定旧提交。
