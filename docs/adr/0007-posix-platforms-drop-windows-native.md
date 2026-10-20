# 0007 停止 Windows 原生支持：WSL 与 macOS Apple Silicon 作为交付平台

**Status**: accepted

规格 #106。本 ADR 取代 ADR 0005 中"保留 PowerShell 7 wrapper"的部分；ADR 0002 的
恢复行为、ADR 0003/0004 的 Herdr 版本与可执行文件选择规则继续有效，只把其中的
平台专用描述按本 ADR 解释。ADR 0006 的 Lua 表格式决定不在此实施：本轮仍读写 EDN。

## 决定

交付平台为 **WSL2 + Ubuntu 24.04 LTS + WSLg**，随后 **macOS Apple Silicon**。
不支持 Windows 原生运行，不支持 Intel Mac，也不单独交付或验收普通 Linux 发行版；
Linux 执行路径由 WSL 承担验证。

理由：Windows 原生实现把进程管理、路径、文件锁与安装入口绑在 Win32 与 PowerShell 上，
维护成本集中在平台差异而不是产品能力；开发与运行统一到 WSL 后，同一套 POSIX 适配层
同时服务 WSL 与 macOS。LOVE 11.5 / LuaJIT、cimgui-love 1.90-1、Herdr 与三个
Python 质量工具都可复用，不需要重写核心。

## 平台边界

平台差异只允许出现在 adapter 层，且**一次选择、长期固定**：

| 能力 | 选择点 | Windows | POSIX |
|---|---|---|---|
| 路径身份与目录枚举 | `adapters/platform_path` | `windows_path` | `posix_path`（`realpath`） |
| 文件 mtime | `adapters/platform_watch` | FILETIME 两段 | `statx` 纳秒 |
| 进程边界 | `adapters/process` | `windows_process`（FFI/PowerShell） | `posix_process`（`posix_spawn`） |
| OS 锁 | `adapters/lock` | 共享模式 0 | `flock` |
| DPI | `adapters/dpi` | `user32` 查询 | 由合成器与 LOVE 帧缓冲缩放提供 |

调用方（domain/application）不感知平台。进程创建身份对调用方是**不透明文本**，
只做精确相等比较：Windows 用 FILETIME，POSIX 用 `/proc/<pid>/stat` 的 starttime。
禁止把 Win32 的表示（ticks、毫秒）强加给 POSIX，也禁止引入时间窗口容差。

## POSIX 进程契约

- `posix_spawn` 传字面 argv，永不经过 shell，因此空格、Unicode 与 shell 元字符不会被重解析。
- 每次 spawn 都显式传环境：`envp = NULL` 在 glibc 下不是"继承"，子进程会得到空环境。
- 已退出但未被回收的僵尸不算存活；detached 子进程没有 waiter，这是设计而非泄漏。
- 超时只终止本次调用自己派生的子进程。
- Herdr 的 Unix socket 路径受 `sockaddr_un` 102 字节限制，过长的 root 必须在启动前
  以真实原因拒绝，而不是让 server 报不透明错误。

## 安装与入口

公共入口是 POSIX shell + Python 3.11+，不要求 PowerShell，也不调用 Windows 可执行文件。
安装幂等，保留手写 policy、提案与有价值的用户状态；已有 ownership 或恢复记录时拒绝安装。
`./uml ir` 无窗口运行，输出字节稳定（同一 policy 多次运行结果一致）。

## 影响

- 既有 Windows 专用测试断言改为**声明平台**的 fixture，而不是依据宿主系统推断；
  行为契约不变时保留，被新平台取代时删除。
- 历史上 JVM 时代的 macOS 验收不作为当前证据。Apple Silicon 验收在真实硬件上执行前
  一律标为**待验收**，不得以编译通过或上游包存在代替。
- Windows 本地仓库、安装副本与专用环境的最终清理需逐项清单与单独确认，
  不属于本 ADR 的自动结果。
