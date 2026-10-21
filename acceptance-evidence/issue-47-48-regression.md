# #47 / #48 回归验证（2026-10-16）

> **历史记录（JVM 时代）。** 本文的 macOS 结果不构成当前实现的证据；macOS 当前不承诺（ADR 0007）。当前证据见 [106-wsl](106-wsl/README.md)。

基线：`f46def3`，`lua` 工作区；修复未提交或推送。本记录区分测试、独立 JVM 验证和真实原生验收。

## #47：macOS specs

原始执行 `clj -M:spec`：568 examples、9 failures、2464 assertions。Herdr fixture 中的 `C:/herdr/...` 不是 macOS 绝对路径；创建 stub 未处理 `/bin/sh`；Windows recovery 测试默认依赖宿主平台。

修复使用宿主绝对 socket，补充 POSIX server launcher stub，显式选择 Windows/非 Windows recovery 分支。生产 endpoint 校验没有放宽。

## #48：JVM 退出兜底

`start!` 在 companion 创建或验证成功后注册 JVM shutdown hook。hook 与窗口退出共用 `cleanup-on-exit!`；monitor 保护一次性清理，`:keep-agent` 保持重启保留语义。清理仍调用生产 `shutdown-children!`，经既有 ownership 验证，再由 Herdr cleanup 先取消 recovery 后释放资源。

所有模拟 `start!` 的测试隔离真实 hook 注册。启动接线测试单独断言注册被调用。

- 将新增退出测试放入旧源码的隔离 worktree：570 examples、2 failures（旧源码缺少 cleanup-on-exit!）。
- 在隔离候选中仅删除 `start!` 的 hook 注册：572 examples、1 failure，接线断言 Expected 1 / got 0。
- 初始候选复制到 detached `f46def3` worktree：`clj -M:spec` 退出0；572 examples、0 failures、2497 assertions。
- #49 聚焦及测试外部进程隔离全部冻结后，再同步候选至同一隔离 worktree：`clj -M:spec` 退出0；574 examples、0 failures、2506 assertions。
- 独立 JVM 用实际 `install-shutdown-hook!`，仅把外部资源清理替换成追加标记文件：正常 `System/exit 0` 后恰好一条标记；`:keep-agent true` 后无标记文件。两个 JVM 均退出0。该检查证明 JVM hook 调用与重启分支，不等同于真实 ⌘Q 或真实 Herdr 资源释放验收。

独立 JVM 探针的有效调用形式：`clj -Sdeps '{:deps {quil/quil {:mvn/version "4.3.1563"}}}' -M <probe.clj> <result-file> normal|restart`。不能用 `-M:spec` 执行脚本；该 alias 有自己的 main opts。

## Mutation 测试外部进程隔离

首轮 mutation 在 worker-3 下创建了真实 Herdr/Claude；worker 源树删除后 ownership 缺失，残留恢复目录的精确身份记录见 issue-49-terminal-attach.md。随后核对 server/shell/runner/Claude 的 PID/start/cwd 和 Herdr pane 身份，并确认 recovery 为 failed 且无 controller，通过官方显式 session stop 等待五个所属进程消失后 session delete，均退出0。未伪造 ownership、绕过 missing ownership 拒绝或 kill 进程。

已定位可达路径：`start!` 的 `if restart?` 变为 `if-not` 时，“opens the saved detail card when start! restores its id” 用例只替换 remember-companion!，却进入真实 open-in-terminal! → open-herdr-companion! → companion/create!。安全内存探针实际反转该分支，mailbox 读写全内存，在 Herdr session list 请求处由 process! 替身截断，证明外部调用可达。由于 worker 已删除，不能将残留进程唯一归因于某一 mutant。

sketch、herdr_lifecycle、core、ownership 测试组现于最外层用 variadic AssertionError 替身阻止 companion/process! 与 sketch/run-command；显式边界测试仍可使用内层 fake。新增回归验证错误启动分支在 process 执行前失败。未撤掉护栏再次创建真实进程，也未因本次覆盖缺口重跑 mutation。

## 质量结果

- 冻结候选 `clj -M:crap`：退出0。
- `clj -M:mutate src/uml_viewer/adapters/sketch.clj`：baseline PASS；26/26 covered mutants killed；5 uncovered mutations，退出3。按项目纪律保留快照，不因覆盖缺口重跑。该次包含新退出函数和 manifest 差分。
- 随后 #49 Terminal 字符串修改完成，再次按新源码执行差分 mutation：退出0，No mutations to test。
- `clj -M:ir`：退出0，生成 `examples/uml-viewer.edn`。
- `git diff --check`：通过。

随后补充了真实生产链验证：独立 JVM 通过生产 create! 在新临时项目创建 `/bin/sleep 120` 与完整 ownership/supervisor，安装真实 hook 后 `System/exit 0`。实际退出0，companion 记录为空，recovery 为 cancelled（Viewer closed; recovery revoked），Herdr session 已删除，server/shell/child/controller 四个已验证 PID 均退出。详细命令与身份见 [issue-49-terminal-attach.md](issue-49-terminal-attach.md) 的 Issue #48 节。

真实 ⌘Q、Dock 退出、退避窗口中的退出以及与项目 B 的并行隔离，不能仅由上述 JVM 检查判为已验证；这些原生交互仍未重跑。
