# 修复 upstream 差异审查发现的回归

## Goal

修复 fork 与 `refs/remotes/upstream/main` 对比审查中发现的 Critical / Warning 回归，同时保留本地 fork 已确认需要的功能增强与发布配置。

## Requirements

- 恢复非 Windows CLI 版本探测对用户 shell 的支持，避免 GUI 环境 PATH 与用户登录 shell PATH 不一致导致误判工具未安装。
- 恢复 Codex 升级策略中 upstream 对 `codex update` 的规避与损坏安装自愈逻辑，不重新引入“升级成功但安装仍损坏”的回归。
- 恢复 macOS provider terminal 启动链路中 upstream 的用户 shell、Terminal / iTerm / Warp 等兼容修复；不得破坏本地 provider-specific config 注入。
- 恢复 Claude Desktop provider 页面状态告警、导入后的状态查询失效刷新，以及 `Ctrl/Cmd+F` 在输入区域不被全局搜索抢占的交互保护。
- 修复 OpenCode Go 模型拉取对裸 `opencode` 命令的依赖，优先解析实际可执行路径，提升桌面 GUI 环境稳定性。
- 保留本地功能：版本 `3.16.7`、fork updater/release 配置、OpenCode Go provider 导入、JSONC/CST 写回能力、Trellis/AGENTS/OpenCode 工作流资产。

## Acceptance Criteria

- [x] `pnpm typecheck` 通过。
- [x] Rust 定向检查或测试覆盖被修改的 Tauri 命令模块；如无法运行，需说明原因。
- [x] `ProviderList` 同时保留本地 stream check 确认弹窗、同步按钮、Claude Desktop 专用导入分支，并恢复 upstream 状态告警/快捷键保护。
- [x] `misc.rs` 的版本探测、升级命令、macOS terminal 启动不再明显回退 upstream 修复。
- [x] `fetch_opencode_go_models` 不再直接依赖裸命令名。

## Notes

- 本任务只修复已审查确认的 Critical / Warning 项，不做额外重构。
- 如 upstream 与 fork 功能存在冲突，以“恢复 upstream 修复 + 保留 fork 功能”为目标做最小整合。
