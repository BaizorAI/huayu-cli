# PRD — huazhen（华珍）

## Problem Statement

开发者在使用 AI 编程工具（Codex、Claude Code）时面临两个痛点：

1. **配置分散**：需要手动登录 baizor.com 获取 API Key、手动写配置文件，每次换机都要重复。
2. **使用割裂**：Codex 和 Claude Code 各自独立运行，没有统一的交互界面，任务下发和进度观察需要在多个终端窗口间切换，认知负担重。

## Solution

**huazhen** 是一个 Rust 编写的 **TUI 全屏应用**，以统一界面整合 AI 编程工作流：

- **主工作区**由聊天输入框和任务输出显示框组成，用户在输入框写需求，huazhen 将其转发给 Codex 或 Claude Code 执行。
- **进度工作区**实时展示底层工具（codex / claude）的运行过程、日志和状态。
- **配置完全自治**：codex / claude 所需的 API Key、模型、base_url 全部由 huazhen 管理，通过环境变量注入到子进程，**不读取、不修改用户系统上已有的 codex / claude 配置文件**，两者互不干扰。
- **首次运行**自动引导完成 baizor.com 登录，此后无需再关心底层配置细节。

---

## 界面布局

```
┌──────────────────────────────────────────────────────────────┐
│  huazhen（华珍）                    [codex ▼]  [●连接中]     │  ← 顶部状态栏
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
  [Enter 发送]  [Tab 切换工具]  [Esc 取消]  [q 退出]            ← 快捷键提示
```

**三个主要区域**：

| 区域 | 位置 | 职责 |
|------|------|------|
| 任务输出显示区 | 左侧主区 | 展示 AI 回复内容、任务执行结果、文件变更摘要 |
| 工具进度工作区 | 右侧面板 | 实时流式展示 codex / claude 的运行日志和状态 |
| 聊天输入框 | 底部 | 用户输入自然语言需求，回车发送给当前选中工具 |

---

## User Stories

### 主工作区 — 聊天与输出

1. 作为用户，我想在底部聊天输入框中用自然语言描述编程需求，使我不需要记忆 CLI 命令语法。
2. 作为用户，我想按 Enter 发送消息并立即看到工具响应，使工作流不被打断。
3. 作为用户，我想在左侧显示区看到 AI 回复的结构化内容（代码、说明、文件变更列表），使我快速理解执行结果。
4. 作为用户，我想支持多轮对话，使我可以基于上一次结果继续追问或修正。
5. 作为用户，我想在输入框中用 Ctrl+C 取消正在进行的任务，使我可以随时中断长时间运行的操作。
6. 作为用户，我想用方向键浏览历史输出，使我可以回顾之前的任务结果。
7. 作为用户，我想输入框支持多行编辑（Shift+Enter 换行），使我可以输入包含代码示例的复杂需求。

### 工具进度工作区 — 运行过程可视化

8. 作为用户，我想在右侧进度区实时看到 codex 或 claude 的运行日志流，使我了解工具正在做什么。
9. 作为用户，我想看到工具执行的关键步骤标记（如"写入文件"、"运行测试"、"完成"），使我不需要逐行解读日志。
10. 作为用户，我想在工具出错时在进度区看到醒目的错误提示和错误信息，使我快速定位问题。
11. 作为用户，我想进度区在任务完成后保留最后一次运行的日志，使我可以回溯执行过程。
12. 作为用户，我想进度区显示当前任务耗时，使我了解执行效率。

### 工具选择与切换

13. 作为用户，我想通过顶部状态栏的下拉菜单或 Tab 快捷键切换 codex / claude，使我根据任务特点选择最合适的工具。
14. 作为用户，我想切换工具后输入框内容保留，使我不需要重新输入相同的需求。
15. 作为用户，我想在状态栏看到当前工具的连接状态（已连接 / 未配置 / 错误），使我知道工具是否可用。

### 首次运行与配置

16. 作为新用户，我想首次运行 huazhen 时自动进入引导流程（登录），使我无需阅读文档即可开始使用。
17. 作为用户，我想登录流程在 TUI 内以弹窗形式引导（显示登录 URL），而不是跳出到另一个命令，使体验连贯。
18. 作为用户，我想配置完成后无缝回到主工作区，使我立即可以开始发送任务。
19. 作为用户，我想默认使用 `huazhen-fable-5` 模型，无需手动指定。
20. 作为高级用户，我想通过设置面板切换模型或修改 base_url，使我可以在不同环境间灵活切换。
21. 作为用户，我想 huazhen 的 API Key 和模型配置完全存储在 `~/.huazhen/` 下，使它与我系统上已有的 codex / claude 配置互不干扰，两套环境可以独立共存。
22. 作为用户，我想即使系统中已经安装并配置了其他版本的 codex 或 claude，huazhen 调用的仍是其自身管理的配置，使我不用担心 huazhen 破坏我现有的工具配置。

### 辅助功能

21. 作为用户，我想用 `huazhen login` 子命令在非 TUI 环境下单独完成登录，使我在 CI / SSH 环境中也能配置。
22. 作为用户，我想用 `huazhen status` 查看当前配置（Key 遮掩显示、当前工具、模型），使我快速确认配置有效。
23. 作为用户，我想配置持久化在 `~/.huazhen/config.json`，重启终端后无需重新登录。

---

## Implementation Decisions

### TUI 布局引擎

使用 ratatui 的 Layout 系统实现三区域分割：
- 顶部固定高度状态栏（1 行）
- 中部主区用垂直分割线划分为左（输出区）和右（进度区），比例约 6:4，支持拖拽调整宽度。
- 底部动态高度输入框（最小 1 行，最大 5 行，超出后内部滚动）。

### 工具调用架构

huazhen 以**子进程**方式启动 codex 或 claude，**通过环境变量注入所有配置**，完全不依赖系统级别的 codex/claude 配置文件：

- 启动 codex 子进程时注入：
  - `OPENAI_API_KEY=<huazhen api_key>`
  - `OPENAI_BASE_URL=https://baizor.com/v1`
  - `CODEX_MODEL=huazhen-fable-5`（或用户选定模型）
- 启动 claude 子进程时注入：
  - `ANTHROPIC_AUTH_TOKEN=<huazhen api_key>`
  - `ANTHROPIC_BASE_URL=https://baizor.com/v1`
  - `ANTHROPIC_MODEL=huazhen-fable-5`（或用户选定模型）

子进程从 stdin 接收 prompt，stdout/stderr 流实时写入右侧进度工作区。子进程的 HOME 或配置目录**不做重定向**，仅通过环境变量覆盖鉴权和端点，确保与用户自有配置隔离但不破坏其运行环境。

### 工具适配层

定义统一的 `ToolAdapter` trait，codex 和 claude 各自实现：
- `spawn(prompt: &str) -> ChildProcess`
- `parse_event(line: &str) -> Option<ToolEvent>`（解析日志行为结构化事件）

`ToolEvent` 枚举覆盖：FileWritten、TestRun、TestPassed、TestFailed、Error、Done。

### 登录集成

登录弹窗（overlay）在主 TUI 内渲染，不跳出到子命令。流程：
- 生成 token → 显示登录 URL（可按 O 尝试自动打开浏览器）
- 在后台 tokio 任务中轮询 `/api/cli/poll`，通过 mpsc channel 将结果发回 TUI 主循环。
- 收到 key 后关闭弹窗，自动触发 codex 配置写入，回到主工作区。

### 配置存储与隔离

huazhen 的所有配置存储在 `~/.huazhen/config.json`，**不读取、不写入** `~/.codex/`、`~/.claude/` 等系统配置目录。配置通过环境变量在运行时注入子进程，两套环境完全独立：

```json
{
  "api_key": "sk-...",
  "base_url": "https://baizor.com",
  "default_model": "huazhen-fable-5",
  "active_tool": "codex"
}
```

可通过 `HUAZHEN_CONFIG_DIR` 环境变量覆盖配置目录（用于测试和多环境隔离）。

### 默认模型

所有场景默认使用 `huazhen-fable-5`。服务端登录响应中的 `default_model` 字段可覆盖此值。

### Codex 配置写入

**不写入** `~/.codex/` 任何文件。所有 codex 配置通过环境变量在运行时注入子进程（见"工具调用架构"节）。

### Claude 配置写入（可选）

**不写入** `~/.claude/` 任何文件。同上，通过环境变量注入。

---

## Testing Decisions

**好的测试原则**：只测外部可观测行为（进程 stdin 内容、文件写入结果、channel 消息），不测 ratatui widget 的像素级渲染。

**重点测试模块**：

- `LoginService` — token 格式、URL 拼接、轮询响应解析（mock HTTP）。
- `installer.rs` — config.toml / auth.json 写入内容正确性，合并写入幂等性。
- `ToolAdapter` — codex / claude 的 `parse_event` 对各类日志行的解析（表格测试）。
- `config.rs` — 序列化/反序列化，缺字段时使用默认值 `huazhen-fable-5`。

**隔离策略**：通过 `HUAZHEN_CONFIG_DIR` 环境变量将所有文件操作重定向到临时目录，不写真实用户目录。

---

## Out of Scope

- **内置 HTTP 代理**：不实现流量转发，huazhen 只负责调用 codex/claude 子进程。
- **系统 codex / claude 配置隔离**：huazhen 通过环境变量注入配置，不读写 `~/.codex/` 或 `~/.claude/`，与用户已有工具配置完全独立共存。
- **Gemini / OpenCode / Hermes** 等其他工具：首期只支持 codex（默认）和 claude（可选）。
- **多账号 / 多 base_url 切换**：首期单账号单端点。
- **WebDAV 同步 / 配置备份**：不在范围内。
- **Windows 安装 codex**：首期聚焦 macOS/Linux。
- **自动更新**：不内置自更新机制。
- **会话历史持久化**：TUI 关闭后聊天记录不保存（后续版本考虑）。

---

## Further Notes

- 工具进度区的日志流是核心差异化体验，需要在流速过快时自动暂停滚动（用户可手动恢复）。
- codex 若支持 `--json` 输出模式，优先采用以获得结构化事件；否则回退到正则解析纯文本日志。
- huazhen 与 baizor-cli 共用相同的 baizor.com 服务端接口（login poll、models），未来可考虑互相识别对方写入的配置文件。
