# 0003 Herdr version floor with pinned protocol

**Status**: accepted

Herdr 预检从精确钉死 `herdr 0.9.3` 改为版本下限 `>= 0.9.3`，同时 bundled schema 与实际 endpoint 仍必须为 protocol 22。原契约"不虚构范围"拒绝包括更高版本在内的一切其他版本，导致用户升级 Herdr 后无法启动 companion。协议才是 uml-viewer 依赖的兼容边界；版本下限只排除已知过旧的 CLI，因此不设上限，protocol 变化即拒绝。

版本串必须是纯数字 `X.Y.Z`；预发布、build 后缀或 `v` 前缀均拒绝，避免对非正式版本做顺序推断。

唯一有原生验收证据的仍是 0.9.3。高于 0.9.3 的版本按协议门槛放行，但不宣称已验收；历史验收文档保持原样。

增补（2026-10-15，spec #32）：上一句的验收证据范围自此扩大——0.9.4 取得
首个端到端原生验收证据（macOS，Darwin 27.0.0 arm64，验收日志见
acceptance-evidence/acceptance-log.md）。验收暴露并修复三个 macOS 阻断缺陷
（session socket 路径超 sun_path 上限、登录 shell argv0 前导 dash、fresh pty
canonical mode 行长限制），详见证据与 #45。高于 0.9.4 的版本仍按协议门槛
放行但不宣称已验收。

修订（2026-10-21，spec #106）：上述 macOS 证据为 JVM 时代的历史记录，macOS
当前不承诺（ADR 0007）。当前实现的端到端证据是 WSL2 + WSLg 上的 Herdr 0.9.5 /
protocol 22（acceptance-evidence/106-wsl/）；版本下限 `>= 0.9.3` 与 protocol 22
门槛不变，高于 0.9.5 的版本按协议门槛放行但不宣称已验收。
