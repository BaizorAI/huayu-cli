# PRD — huazhen（华珍）

## Problem Statement

开发者在使用 AI 编程工具（Codex、Claude Code）时面临两个痛点：

1. **配置分散**：需要手动登录 baizor.com 获取 API Key、手动写配置文件，每次换机都要重复。
2. **使用割裂**：Codex 和 Claude Code 各自独立运行，没有统一的交互界面，任务下发和进度观察需要在多个终端窗口间切换，认知负担重。

## Solution

**huazhen** 是一个 Rust 编写的 **TUI 全屏应用**，以统一界面整合 AI 编程工作流：

- **主工作区**由聊天输入框和任务输出显示框组成，用户在输入框写需求，huazhen 将其转发给 Codex 或 Claude Code 执行。
- **进度工作区**实时展示底层工具（codex / claude）的运行过程、日志和状态。
- **配置完全自治**：codex / claude 所需的 API Key、模型、base_url 全部由 huazhen 管理，通过隔离的配置目录注入，**不读取、不修改用户系统上已有的 codex / claude 配置**，两者互不干扰。
- **首次运行**自动引导完成 baizor.com 登录，此后无需再关心底层配置细节。

---

## 界面布局

```
┌──────────────────────────────────────────────────────────────┐
│  huazhen（华珍）          [codex ▼]  [●连接中]  [s]设置      │  ← 顶部状态栏
├────────────────────────────┬─────────────────────────────────┤
│                            │                                 │
│                            │   工具进度工作区                │
│   任务输出显示区            │   (codex / claude 运行输出)    │
│   (AI 回复 / 结果)         │                                 │
│                            │   > codex running...           │
│                            │   > Wrote src/main.rs          │
│                            │   > Running tests...           │
│                            │   > ✓ All tests passed         │
│                            │                                 │
├────────────────────────────┴─────────────────────────────────┤
│  聊天输入框                                                   │  ← 底部输入区
│  > 帮我实现一个用户登录接口，包含 JWT 验证_                   │
└──────────────────────────────────────────────────────────────┘
  [Enter 发送]  [Tab 切换工具]  [←/→ 调整面板宽度]  [Esc 取消]  [q 退出]
```

**三个主要区域**：

| 区域 | 位置 | 职责 |
|------|------|------|
| 任务输出显示区 | 左侧主区 | 展示 AI 回复内容、任务执行结果、文件变更摘要 |
| 工具进度工作区 | 右侧面板 | 实时流式展示 codex / claude 的运行日志和状态 |
| 聊天输入框 | 底部 | 用户输入自然语言需求，回车发送给当前选中工具 |

**设置面板**：按 `s` 键以 overlay 方式打开，可切换模型、修改 base_url，按 Esc 关闭。

---

## User Stories

### 主工作区 — 聊天与输出

1. 作为用户，我想在底部聊天输入框中用自然语言描述编程需求，使我不需要记忆 CLI 命令语法。
2. 作为用户，我想按 Enter 发送消息并立即看到工具响应，使工作流不被打断。
3. 作为用户，我想在左侧显示区看到 AI 回复的结构化内容（代码、说明、文件变更列表），使我快速理解执行结果。
4. 作为用户，我想支持多轮对话，huazhen 自动将本轮及历史消息拼接后传给工具，使对话上下文连贯。
5. 作为用户，我想在输入框中用 Esc 取消正在进行的任务，使我可以随时中断长时间运行的操作。
6. 作为用户，我想用方向键浏览历史输出，使我可以回顾之前的任务结果。
7. 作为用户，我想输入框支持多行编辑（Shift+Enter 换行），使我可以输入包含代码示例的复杂需求。

### 工具进度工作区 — 运行过程可视化

8. 作为用户，我想在右侧进度区实时看到 codex 或 claude 的运行日志流，使我了解工具正在做什么。
9. 作为用户，我想看到工具执行的关键步骤标记（如"写入文件"、"运行测试"、"完成"），使我不需要逐行解读日志。
10. 作为用户，我想在工具出错时在进度区看到醒目的错误提示和错误信息，使我快速定位问题。
11. 作为用户，我想进度区在任务完成后保留最后一次运行的日志，使我可以回溯执行过程。
12. 作为用户，我想进度区显示当前任务耗时，使我了解执行效率。
13. 作为用户，我想在日志滚动过快时按空格键暂停自动滚动，再按空格恢复，使我可以仔细阅读中间输出。

### 工具选择与切换

14. 作为用户，我想通过顶部状态栏的下拉菜单或 Tab 快捷键切换 codex / claude，使我根据任务特点选择最合适的工具。
15. 作为用户，我想在有任务正在执行时切换工具，huazhen 先终止当前任务再切换，并在进度区提示"任务已取消，已切换到 X"，使我明确知道发生了什么。
16. 作为用户，我想切换工具后输入框内容保留，使我不需要重新输入相同的需求。
17. 作为用户，我想在状态栏看到当前工具的连接状态（已连接 / 未配置 / 不可用 / 错误），使我知道工具是否可用。
18. 作为用户，我想 `huazhen status` 同时检测 codex / claude 二进制是否在 PATH 中可执行，并展示检测结果，使我快速确认工具依赖是否满足。

### 首次运行与配置

19. 作为新用户，我想首次运行 huazhen 时自动进入引导流程（登录），使我无需阅读文档即可开始使用。
20. 作为用户，我想登录流程在 TUI 内以弹窗形式引导（显示登录 URL），而不是跳出到另一个命令，使体验连贯。
21. 作为用户，我想配置完成后无缝回到主工作区，使我立即可以开始发送任务。
22. 作为用户，我想默认使用 `huazhen-fable-5` 模型，无需手动指定。
23. 作为高级用户，我想按 `s` 打开设置面板后切换模型或修改 base_url，使我可以在不同环境间灵活切换。
24. 作为用户，我想 huazhen 的 API Key 和模型配置完全存储在 `~/.huazhen/` 下，与系统已有的 codex / claude 配置互不干扰，两套环境可以独立共存。
25. 作为用户，我想 API Key 过期或失效时，huazhen 在进度区和状态栏显示明确的"认证失败"提示，并引导我重新运行登录流程，使我不会面对静默失败。
26. 作为用户，我想在 baizor.com 服务不可达时，huazhen 在状态栏展示"服务不可用"并在进度区说明原因，使我知道是网络/服务问题而非工具 bug。

### 辅助功能

27. 作为用户，我想用 `huazhen login` 子命令在非 TUI 环境下单独完成登录，使我在 CI / SSH 环境中也能配置。
28. 作为用户，我想用 `huazhen status` 查看当前配置（Key 遮掩显示、当前工具、模型、工具二进制可用性），使我快速确认一切就绪。
29. 作为用户，我想配置持久化在 `~/.huazhen/config.json`，重启终端后无需重新登录。

---

## Implementation Decisions

### TUI 布局引擎

使用 ratatui 的 Layout 系统实现三区域分割：
- 顶部固定高度状态栏（1 行），右侧含 `[s]设置` 入口提示。
- 中部主区用垂直分割线划分为左（输出区）和右（进度区），初始比例约 6:4，通过 `←/→` 键调整分割比例（不使用鼠标拖拽，ratatui 不提供该原语）。
- 底部动态高度输入框（最小 1 行，最大 5 行，超出后内部滚动）。

### 工具调用架构（PTY 模式）

huazhen 通过 **PTY（伪终端）** 启动 codex / claude 子进程，使工具以为运行在真实终端中，从而保留流式输出行为：

- 使用 `portable-pty` crate 创建 PTY pair，子进程在 slave 端运行，huazhen 在 master 端读取输出流。
- 子进程的 stdout/stderr 合并后实时写入右侧进度工作区。
- 多轮对话上下文：huazhen 维护本次会话的消息历史列表，每次发送前将历史消息与当前 prompt 按工具要求的格式拼接后写入 PTY stdin。

### 配置隔离机制

huazhen 通过**重定向配置目录**而非仅注入环境变量来实现隔离，确保工具加载的配置完全由 huazhen 控制：

- **Codex**：注入 `CODEX_HOME=~/.huazhen/codex/`，huazhen 在该目录写入 `config.toml` 和 `auth.json`。
- **Claude Code**：注入 `CLAUDE_CONFIG_DIR=~/.huazhen/claude/`，huazhen 在该目录写入最小 `settings.json`（仅含 API Key 和 base_url）。

两个目录均在 huazhen 首次运行时自动创建，与用户的 `~/.codex/` 和 `~/.claude/` 完全隔离。

子进程注入的环境变量：

| 工具 | 变量 | 值 |
|------|------|----|
| codex | `CODEX_HOME` | `~/.huazhen/codex/` |
| codex | `OPENAI_API_KEY` | huazhen api_key |
| codex | `OPENAI_BASE_URL` | `https://baizor.com/v1` |
| claude | `CLAUDE_CONFIG_DIR` | `~/.huazhen/claude/` |
| claude | `ANTHROPIC_AUTH_TOKEN` | huazhen api_key |
| claude | `ANTHROPIC_BASE_URL` | `https://baizor.com/v1` |

### 工具适配层

定义统一的 `ToolAdapter` trait，codex 和 claude 各自实现：
- `spawn(history: &[Message]) -> PtySession`
- `parse_event(line: &str) -> Option<ToolEvent>`（解析日志行为结构化事件）

`ToolEvent` 枚举：FileWritten、TestRun、TestPassed、TestFailed、AuthError、NetworkError、Error、Done。

`AuthError` 和 `NetworkError` 触发状态栏更新和引导提示。

### 工具切换竞态处理

切换工具时：
1. 若当前有子进程运行，先向其发送 SIGTERM，等待最多 3 秒，超时则 SIGKILL。
2. 进度区追加一行"[任务已取消] 已切换到 X"。
3. 切换完成，输入框内容保留。

### 登录集成

登录 overlay 在主 TUI 内渲染。流程：
- 生成 token → 显示登录 URL（可按 `o` 自动打开浏览器）。
- 后台 tokio 任务轮询 `/api/cli/poll`，每 2 秒一次，超时 5 分钟。
- 超时时 overlay 显示"登录超时，按 r 重试"。
- 网络不可达时 overlay 显示"无法连接 baizor.com，请检查网络"。
- 收到 key 后关闭 overlay，写入隔离配置目录，回到主工作区。

### 离线与认证失败处理

| 场景 | 状态栏 | 进度区 |
|------|--------|--------|
| baizor.com 不可达 | `[●服务不可用]` | 说明原因，建议检查网络 |
| API Key 失效 | `[●认证失败]` | 提示运行 `huazhen login` 重新登录 |
| 工具二进制不存在 | `[●工具未找到]` | 提示安装 codex 或 claude 的命令 |

### 配置存储

`~/.huazhen/config.json`：

```json
{
  "api_key": "sk-...",
  "base_url": "https://baizor.com",
  "default_model": "huazhen-fable-5",
  "active_tool": "codex"
}
```

隔离子目录：
- `~/.huazhen/codex/config.toml` + `auth.json`（由 huazhen 管理）
- `~/.huazhen/claude/settings.json`（由 huazhen 管理）

可通过 `HUAZHEN_CONFIG_DIR` 环境变量覆盖根目录（用于测试）。

### 默认模型

所有场景默认使用 `huazhen-fable-5`。服务端登录响应中的 `default_model` 字段可覆盖此值。

---

## Testing Decisions

**好的测试原则**：只测外部可观测行为（进程输入内容、文件写入结果、channel 消息、状态转换），不测 ratatui widget 的像素级渲染。

**重点测试模块**：

- `LoginService` — token 格式（32 位十六进制）、URL 拼接、轮询响应解析（mock HTTP）、超时路径、网络失败路径。
- `config.rs` — 序列化/反序列化，缺字段时使用默认值 `huazhen-fable-5`，`HUAZHEN_CONFIG_DIR` 重定向生效。
- `ToolAdapter::parse_event` — 对 codex / claude 各类日志行的解析（deterministic 表格测试），包括 AuthError 和 NetworkError 的识别。
- `隔离目录写入` — huazhen 写入 `~/.huazhen/codex/` 和 `~/.huazhen/claude/` 的文件内容正确性，验证不触碰系统 `~/.codex/` 和 `~/.claude/`。
- `工具切换` — 竞态处理：模拟子进程运行中切换工具，验证 SIGTERM 发送、进度区消息追加、输入框内容保留。

**隔离策略**：通过 `HUAZHEN_CONFIG_DIR` 将所有文件操作重定向到临时目录，测试结束后清理，不写真实用户目录。

---

## Out of Scope

- **内置 HTTP 代理**：不实现流量转发，huazhen 只负责调用 codex/claude 子进程。
- **Gemini / OpenCode / Hermes** 等其他工具：首期只支持 codex（默认）和 claude（可选）。
- **多账号 / 多 base_url 切换**：首期单账号单端点。
- **WebDAV 同步 / 配置备份**：不在范围内。
- **Windows 支持**：PTY 模式在 Windows 上需要 ConPTY，首期聚焦 macOS/Linux，Windows 作为后续任务。
- **自动更新**：不内置自更新机制。
- **会话历史持久化**：TUI 关闭后聊天记录不保存（后续版本考虑）。
- **面板拖拽调整宽度**：ratatui 不提供鼠标拖拽原语，改为键盘 `←/→` 调整。

---

## Further Notes

- PTY 模式是核心架构选择，确保工具的流式输出行为不受影响，同时 huazhen 可完整捕获输出流。需在动工前验证 `portable-pty` 在目标平台（macOS / Linux）的兼容性。
- 日志解析 `parse_event` 采用正则匹配纯文本作为主路径；若 codex/claude 未来提供结构化 JSON 输出模式，作为优先路径接入，正则作为 fallback。建议记录工具版本号以便追踪格式变化。
- huazhen 与 baizor-cli 共用相同的 baizor.com 服务端接口（login poll、models），未来可考虑互相识别对方的隔离配置目录。
