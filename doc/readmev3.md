# 华宇 huayu v3 — AI 编码工作站

> 华宇 (huayu) 是面向 [baizor.com](https://baizor.com) 的 AI 编码工作站，以 Rust TUI 终端应用形式将 **Codex** 与 **Claude Code** 整合为统一交互界面。

---

## 目录

1. [项目概述](#项目概述)
2. [核心价值](#核心价值)
3. [快速安装](#快速安装)
4. [架构设计](#架构设计)
5. [模块详解](#模块详解)
6. [数据流](#数据流)
7. [配置体系](#配置体系)
8. [Slash 命令体系](#slash-命令体系)
9. [快捷键](#快捷键)
10. [智能构建与部署](#智能构建与部署)
11. [测试策略](#测试策略)
12. [错误处理](#错误处理)
13. [设计约定](#设计约定)
14. [工作范围外](#工作范围外)

---

## 项目概述

| 维度 | 详情 |
|------|------|
| **语言** | Rust 2021 edition |
| **技术栈** | clap 4.5 / ratatui 0.29 + crossterm 0.28 / tokio + reqwest 0.12 / portable-pty 0.8 |
| **版本** | 0.2.0（huayu）/ 0.142.5（Codex）/ 1.0.3（Claude） |
| **平台** | Windows x64 / Linux x64+aarch64 / macOS x64+aarch64 |
| **安装** | 一行命令一键安装，零外部依赖 |
| **配置目录** | `~/.huayu/`（通过 `HUAYU_CONFIG_DIR` 环境变量可覆盖） |
| **测试** | 56 个单元测试全部通过 |

---

## 核心价值

| 痛点 | 解决方案 |
|------|---------|
| Codex / Claude 需分别通过 npm 安装，依赖 Node.js | 内嵌锁定版本二进制，零外部依赖 |
| 两个工具交互方式不同，学习成本高 | 统一 TUI 界面 + 一致快捷键体系 |
| API Key 配置繁琐 | 浏览器 OAuth 一键登录，自动生成所有配置文件 |
| Windows 终端中文乱码 | 启动时 `SetConsoleCP(65001)` + `SetConsoleOutputCP(65001)` |
| 安装门槛高 | `irm ... \| iex` 一行命令完成安装，无需管理员权限 |
| 工具版本不可控 | Release 包内捆绑锁定版本二进制，独立于系统 PATH |

---

## 快速安装

### Windows（一键安装）

```powershell
irm https://baizor.com/install/huayu.ps1 | iex
```

安装脚本行为：
1. 检测 CPU 架构（当前仅支持 x64）
2. 通过 GitHub API 获取最新 Release 版本
3. 下载 `huayu-x86_64-pc-windows-msvc.zip`
4. 解压至 `%USERPROFILE%\.huayu\`（bin/ + tools/）
5. 将 `bin\` 追加到 User PATH（持久化注册表）
6. 无需管理员权限，适合企业环境

### macOS / Linux

从 [GitHub Releases](https://github.com/BaizorAI/huayu/releases) 下载对应平台的 `tar.gz`，解压并放入 PATH。

### 验证安装

```bash
huayu status       # 查看配置与工具状态
huayu login        # 浏览器登录 baizor.com
huayu              # 启动 TUI 工作站
```

### 安装后目录结构

```
%USERPROFILE%\.huayu\          # HUAYU_CONFIG_DIR
├── bin\huayu.exe              # 华宇本体
├── tools\                     # 工具二进制（由 /update 管理）
│   ├── codex.exe / codex.version
│   └── claude      / claude.version
├── codex\                     # Codex 隔离配置（CODEX_HOME）
│   ├── config.toml
│   └── auth.json
├── claude\                    # Claude 隔离配置（CLAUDE_CONFIG_DIR）
│   ├── settings.json
│   └── config.json
├── config.json                # 华宇主配置
├── history.json               # 输入历史（最多 50 条）
└── debug.log                  # 完整工具输出日志
```

---

## 架构设计

### 分层架构

```
                    ┌──────────────────────────────┐
                    │         main.rs               │
                    │   CLI 解析 → TUI 或子命令      │
                    └────────────┬─────────────────┘
                                 │
          ┌──────────────────────┼──────────────────────┐
          │                      │                      │
   ┌──────▼──────┐     ┌────────▼────────┐     ┌───────▼───────┐
   │  cli/        │     │  tui/            │     │  services/    │
   │  ├─ login    │     │  ├─ mod (事件循环) │     │  ├─ installer │
   │  ├─ status   │     │  ├─ app (状态机)   │     │  ├─ login     │
   │  └─ update   │     │  ├─ ui (渲染)      │     │  └─ model_fetch│
   └──────┬───────┘     │  └─ theme (主题)   │     └──────┬───────┘
          │              └────────┬─────────┘            │
          │                       │                      │
   ┌──────▼───────────────────────▼──────────────────────▼──────┐
   │                      共享层                                 │
   │  config.rs (配置)  │  command.rs (命令解析)  │  tool.rs (PTY) │
   │  error.rs (错误)                                      │
   └───────────────────────────────────────────────────────────┘
```

### 模块职责

| 模块 | 文件 | 职责 |
|------|------|------|
| **入口** | `main.rs` | 解析 CLI 参数，路由到 TUI 或子命令；Windows UTF-8 初始化 |
| **配置** | `config.rs` | 配置加载/保存、Codex TOML 生成、Claude JSON 生成、模型元数据管理 |
| **命令** | `command.rs` | TUI 斜杠命令解析、帮助文本 |
| **工具** | `tool.rs` | PTY 子进程管理、输出事件解析（认证/网络/文件/测试） |
| **错误** | `error.rs` | 统一 `AppError` 类型（Io/Config/Network/Auth/ToolNotFound/Json/Message） |
| **CLI** | `cli/` | clap derive 子命令：login、status、update |
| **TUI** | `tui/` | 事件循环、App 状态机、界面渲染、主题 |
| **服务** | `services/` | 工具下载安装、浏览器登录轮询、模型列表获取 |

---

## 模块详解

### `main.rs` — 入口

```
huayu                          → 加载配置 → 启动 TUI
huayu login                    → 浏览器登录流程
huayu status                   → 输出配置与工具状态
huayu update [codex|claude]    → 下载/更新工具
```

Windows 平台在启动时调用 `SetConsoleOutputCP(65001)` + `SetConsoleCP(65001)` 避免中文乱码。

### `config.rs` — 配置中心（503 行）

**核心数据结构：**

```rust
pub struct HuayuConfig {
    pub api_key: String,               // baizor.com API Key
    pub base_url: String,              // 默认 https://baizor.com
    pub default_model: String,         // 默认 huayu-v2
    pub active_tool: String,           // codex / claude
    // Codex 专有
    pub codex_model: String,
    pub codex_full_auto: bool,         // 默认 true
    pub codex_reasoning_effort: String,// low / medium / high
    // Claude 专有
    pub claude_model: String,
    pub claude_max_turns: u32,         // 0 = 不限
    pub claude_permission_mode: String,// 默认 bypassPermissions
    // 模型元数据（从服务端同步）
    pub model_info: HashMap<String, ModelInfo>,
}
```

**关键函数：**

| 函数 | 功能 |
|------|------|
| `config_dir()` | 返回 `$HUAYU_CONFIG_DIR` 或 `~/.huayu/` |
| `load()` / `save()` | 读取/写入 `config.json` |
| `write_codex_config()` | 生成 `codex/config.toml` + `auth.json` |
| `write_claude_config()` | 生成 `claude/settings.json` + `config.json` |
| `effective_codex_model()` | 解析 Codex 实际使用的模型（专有 > 默认） |
| `effective_claude_model()` | 解析 Claude 实际使用的模型 |
| `load_input_history()` / `save_input_history()` | 命令历史持久化（最多 50 条） |

**配置生成细节：**

- `codex/config.toml`：模型 provider、base_url（`/v1` 后缀）、wire_api = `"responses"`、[model_info] 节
- `codex/auth.json`：`{"OPENAI_API_KEY": "sk-xxx"}`
- `claude/settings.json`：env 注入 `ANTHROPIC_AUTH_TOKEN`、`ANTHROPIC_BASE_URL`、`ANTHROPIC_MODEL`，`bypassPermissionsModeAccepted: true`
- `claude/config.json`：`bypassPermissionsModeAccepted: true`，`hasCompletedOnboarding: true`

### `tool.rs` — PTY 工具管理（424 行）

**ToolType 枚举：** Codex / Claude，提供 `binary_path()`、`is_available()` 方法。

**ToolEvent 枚举（结构化事件解析）：**

| 事件 | 触发条件 |
|------|---------|
| `Line(String)` | 普通输出行 |
| `FileWritten(String)` | 匹配 `wrote` / `written` / `created` 关键词 |
| `TestPassed` | 匹配 `test` + `pass` / `ok` |
| `TestFailed(String)` | 匹配 `test` + `fail` |
| `AuthError` | 匹配 `401 Unauthorized` / `invalid api key` / `authentication failed` |
| `NetworkError` | 匹配 `connection refused` / `network error` |
| `Done` | 进程退出哨兵 |
| `Error(String)` | 进程启动失败 |

**ToolProcess 结构：**

```rust
pub struct ToolProcess {
    process_id: Option<u32>,   // 用于 kill
    writer: Box<dyn Write + Send>,  // PTY master 写端
    pub rx: mpsc::Receiver<ToolEvent>,  // 事件接收端
}
```

**spawn 流程：**
1. 通过 `portable_pty` 创建 PTY pair
2. 设置 PTY 尺寸（120×40）
3. 注入 `CODEX_HOME` / `CLAUDE_CONFIG_DIR` 环境变量
4. 启动子进程（`codex exec` 或 `claude --print`）
5. 后台线程读取 master 输出 → strip ANSI → 写入 `debug.log` → 发送 `ToolEvent`
6. 进程结束发送 `ToolEvent::Done`

### `command.rs` — 命令解析（纯函数，可测试）

```
/开头 → 解析为 AppCommand
非/开头 → 返回 None（作为普通 prompt 发送给工具）
```

| 命令 | AppCommand | 说明 |
|------|-----------|------|
| `/login` | `Login` | 浏览器登录 |
| `/switch <tool>` | `Switch(String)` | 切换 Codex/Claude |
| `/model <name>` | `Model(String)` | 更改默认模型 |
| `/update [tool]` | `Update(UpdateTarget)` | 下载/更新工具 |
| `/install [tool]` | `Update(UpdateTarget)` | `/update` 同义词 |
| `/status` | `Status` | 显示状态 |
| `/help` / `/?` | `Help` | 帮助 |
| `/clear` | `Clear` | 清空面板 |
| `/quit` / `/exit` / `/q` | `Quit` | 退出 |

### `services/login.rs` — 浏览器登录

```
┌─────────┐     ┌──────────────┐     ┌─────────────┐
│  huayu   │────▶│ baizor.com   │────▶│ 浏览器       │
│ 生成UUID │     │ /code/token  │     │ 用户授权     │
└────┬─────┘     └──────┬───────┘     └──────┬──────┘
     │                  │                    │
     │  轮询 /api/cli/poll (2s 间隔)         │
     │◀─────────────────┘                    │
     │  status=pending → 继续轮询             │
     │  status=done    → 返回 LoginOutcome    │
     │                                       │
     ▼                                       │
  LoginOutcome {                             │
    api_key,                                 │
    default_model,                           │
    codex: CodexSettings,                    │
    claude: ClaudeSettings,                  │
    model_info: HashMap<...>                 │
  }                                          │
```

- 超时时间：300 秒（5 分钟）
- Token：UUID v4，32 位 hex
- 服务端返回内容包括：API Key、默认模型、Codex 专有设置、Claude 专有设置、模型元数据

### `services/installer.rs` — 工具安装（349 行）

**版本锁定常量：**

```rust
const CODEX_VERSION: &str = "0.142.5";
const CLAUDE_VERSION: &str = "1.0.3";
```

**二进制查找优先级：**
1. `tools/` 目录下 bundled 二进制（如 `tools/codex.exe`）
2. npm `node_modules/.bin/` 回退（兼容旧安装）
3. 系统 PATH 中的二进制（`which` 查找）

**下载策略：**
1. 优先从 `https://baizor.com/install/{name}-{version}-{triple}.zip` 下载编译好的二进制包
2. 若下载失败或 baizor 未提供，自动回退到 `npm install -g @anthropic-ai/claude-code@version` 或对应的 codex npm 包
3. 安装完成后写入 `.version` 文件标记版本

**平台三元组映射：**

| 平台 | 三元组 |
|------|--------|
| Windows x64 | `x86_64-pc-windows-msvc` |
| macOS x64 | `x86_64-apple-darwin` |
| macOS ARM | `aarch64-apple-darwin` |
| Linux x64 | `x86_64-unknown-linux-gnu` |
| Linux ARM | `aarch64-unknown-linux-gnu` |

**进度通道：** `mpsc::channel<String>` 流式输出，`"__DONE__"` 为结束哨兵。

### TUI 层 — 界面与事件循环

**`tui/mod.rs` — 事件循环（100ms tick）：**
- 启动时自动重写 Codex/Claude 配置（确保格式最新）
- 未登录自动弹出登录引导弹窗
- 事件循环：绘制 → poll 事件 → drain PTY 输出 → 处理

**`tui/app.rs` — App 状态机（1000+ 行）：**

核心状态字段：

```rust
pub struct App {
    pub config: HuayuConfig,
    pub tool_type: ToolType,          // 当前活跃工具
    pub tool_process: Option<ToolProcess>,  // 运行中的 PTY 进程
    pub connection_status: ConnectionStatus,
    pub messages: Vec<Message>,       // 对话上下文
    pub main_lines: Vec<String>,      // 主面板输出行
    pub scroll_offset: usize,         // 滚动偏移
    pub auto_scroll: bool,            // 自动跟底
    pub input: String,                // 输入框内容
    pub input_history: Vec<String>,   // 输入历史
    pub history_cursor: Option<usize>,// 历史浏览游标
    pub task_start: Option<Instant>,  // 任务计时
    pub login_overlay: Option<LoginOverlay>,  // 登录弹窗
    pub show_settings: bool,          // 设置弹窗
    pub recent_commands: Vec<String>, // 最近命令（最多 5 条）
}
```

**关键方法：**
- `submit()` — 处理输入：空输入忽略、slash 命令执行、普通文本发送给工具
- `switch_tool()` — 切换 Codex ↔ Claude
- `open_login_overlay()` — 生成 token → 打开浏览器 → 启动轮询
- `apply_settings()` — 保存模型/URL 设置并重写配置
- `history_up()` / `history_down()` — 输入历史导航（从空开始循环，到底恢复草稿）

**`tui/ui.rs` — 界面渲染：**

```
┌── 华宇 huayu | codex [Tab切换] | huayu-v2 | ●连接中 ──────────┐
│                                                                │
├──────────────────────────────┬─────────────────────────────────┤
│  主工作区 (左 ~70%)            │  帮助与参考 (右 ~30%)            │
│                              │                                  │
│  执行日志、AI 输出、文件事件   │  快捷键速查                     │
│                              │  最近命令                       │
│                              │  工具版本状态                    │
│                              │                                  │
├──────────────────────────────┴─────────────────────────────────┤
│  输入框: /help                                                  │
│  [Enter]发送 [Shift+Enter]换行 [Tab]切换工具 [Alt+Q]退出        │
└────────────────────────────────────────────────────────────────┘
```

**`tui/theme.rs` — 主题定义：**
- border_color、bg_color、text_bright、text_dimmed 等色彩常量

---

## 数据流

### 用户输入 → 工具执行

```
用户按键 (Enter)
  │
  ▼
tui/mod.rs: handle_key() → key.code == Enter
  │
  ▼
app.rs: submit()
  │
  ├── (1) 解析命令: command::parse(&input)
  │   ├── Some(AppCommand::Login)  → 打开登录弹窗
  │   ├── Some(AppCommand::Switch) → 切换工具
  │   ├── Some(AppCommand::Model)  → 更新模型
  │   ├── Some(AppCommand::Update) → 后台下载工具
  │   ├── Some(AppCommand::Help)   → 显示帮助
  │   ├── Some(AppCommand::Clear)  → 清空面板
  │   ├── Some(AppCommand::Quit)   → 标记退出
  │   └── Some(AppCommand::Status) → 显示状态
  │
  └── (2) None（普通文本）→ 发送给当前工具
      │
      ▼
tool.rs: ToolProcess::write_input(text + "\n")
      │
      ▼
PTY 子进程 (codex exec / claude --print)
      │
      ▼
后台线程: 读取输出 → strip ANSI → 写入 debug.log
      │
      ▼
mpsc::channel → ToolEvent
      │
      ▼
app.rs: drain_tool_events() → main_lines.push(...)
      │
      ▼
ui.rs: render() → ratatui 渲染到终端
```

### 登录流程

```
用户运行 /login 或首次启动
  │
  ▼
LoginService::generate_token() → UUID v4 (32 hex)
  │
  ▼
打开浏览器: https://baizor.com/code/token?token=xxx
  │
  ▼
后台 tokio 任务: 每 2s 轮询 GET /api/cli/poll?token=xxx
  │
  ├── status=pending → 继续轮询
  ├── 超时 300s → LoginState::Error
  └── status=done + key → LoginOutcome
      │
      ▼
写入 config.json, codex/config.toml, codex/auth.json,
     claude/settings.json, claude/config.json
```

---

## 配置体系

### 主配置 `~/.huayu/config.json`

```json
{
  "api_key": "sk-...",
  "base_url": "https://baizor.com",
  "default_model": "huayu-v2",
  "active_tool": "codex",
  "codex_model": "",
  "codex_full_auto": true,
  "codex_reasoning_effort": "medium",
  "claude_model": "",
  "claude_max_turns": 0,
  "claude_permission_mode": "bypassPermissions",
  "model_info": {
    "huayu-v2": { "context_window": 200000, "max_output_tokens": 32000 }
  }
}
```

### 环境变量

| 变量 | 用途 | 默认值 |
|------|------|--------|
| `HUAYU_CONFIG_DIR` | 配置根目录 | `~/.huayu/` |
| `CODEX_HOME` | Codex 配置目录（注入子进程） | `$HUAYU_CONFIG_DIR/codex/` |
| `CLAUDE_CONFIG_DIR` | Claude 配置目录（注入子进程） | `$HUAYU_CONFIG_DIR/claude/` |
| `DEBUG` | 调试模式开关 | `cfg!(debug_assertions)` |

---

## Slash 命令体系

| 命令 | 参数 | 说明 |
|------|------|------|
| `/login` | — | 浏览器 OAuth 登录 baizor.com |
| `/switch` | `codex` / `claude` | 切换当前活跃工具 |
| `/model` | 模型名称 | 更改默认模型 |
| `/update` | `[codex\|claude]` | 下载/更新工具（默认全部） |
| `/install` | `[codex\|claude]` | `/update` 的同义词 |
| `/status` | — | 显示配置摘要与工具可用性 |
| `/help` | — | 显示使用帮助 |
| `/?` | — | 同 `/help` |
| `/clear` | — | 清空主面板输出 |
| `/quit` | — | 退出程序 |
| `/exit` | — | 同 `/quit` |
| `/q` | — | 同 `/quit` |

---

## 快捷键

### 全局

| 快捷键 | 功能 |
|--------|------|
| `Alt+Q` | 退出程序（任何状态下有效） |

### 主界面

| 快捷键 | 功能 |
|--------|------|
| `Enter` | 发送消息 / 执行命令 |
| `Shift+Enter` | 输入框换行 |
| `Esc` | 取消当前任务 / 关闭弹窗 |
| `Tab` | 切换 Codex ↔ Claude |
| `↑ / ↓` | 输入历史导航（语义单一，仅斜杠命令） |
| `Page Up / Page Down` | 主面板翻页（↑ 暂停自动滚动，↓ 到底恢复） |
| 鼠标滚轮 | 上下滚动 |
| `s`（输入框为空） | 打开设置弹窗 |
| `Space`（输入框为空） | 切换自动滚动 |

### 登录弹窗

| 快捷键 | 功能 |
|--------|------|
| `Esc` | 关闭登录弹窗 |
| `r` | 重新发起登录 |

### 设置弹窗

| 快捷键 | 功能 |
|--------|------|
| `Enter` | 保存设置并关闭 |
| `Esc` | 关闭（不保存） |
| `Tab` | 切换字段焦点 |
| `Backspace` | 删除字符 |

---

## 智能构建与部署

### 版本管理

`versions.json` 是三个组件的唯一版本来源：

```json
{
  "huayu": "0.2.0",
  "codex": "0.142.5",
  "claude": "1.0.3"
}
```

- **huayu** 版本由 `build.ps1` 自动管理（源码变化时自动 bump patch）
- **codex/claude** 版本需手动编辑（外部 npm 包升级时）

### 智能构建（build.ps1）

```
versions.json
     │
     ▼
build.ps1
     │
     ├── 计算指纹（src/**/*.rs + Cargo.toml + Cargo.lock 的 SHA256）
     ├── 对比 .build-state.json（上次构建状态）
     │
     ├── [指纹不变] → 跳过
     └── [指纹变化] → 自动 bump patch → 同步版本到所有文件 → 构建
```

**版本同步目标：**

| 目标文件 | 同步内容 |
|---------|---------|
| `Cargo.toml` | huayu version |
| `src/services/installer.rs` | `CODEX_VERSION`, `CLAUDE_VERSION` |
| `package-tools.ps1` | `$CodexVersion`, `$ClaudeVersion` |
| `package-linux.sh` | `CODEX_VERSION`, `CLAUDE_VERSION` |

**使用方式：**

```powershell
.\build.ps1                     # 构建有变化的组件
.\build.ps1 -Force              # 强制全量构建
.\build.ps1 -Component huayu    # 仅处理 huayu
.\build.ps1 -NoBump             # 不自动增加版本号
```

### 智能部署（deploy.ps1）

```
.build-state.json（本地版本）
     │
     ▼
deploy.ps1
     │
     ├── SSH 读取远程版本文件
     ├── 对比本地 vs 远程
     │
     ├── [版本相同] → 跳过
     └── [版本差异] → scp 到 baizor:/lucky/NewApi/data/install/
```

**使用方式：**

```powershell
.\deploy.ps1                    # 部署有版本变化的组件
.\deploy.ps1 -DryRun            # 仅显示差异
.\deploy.ps1 -Force             # 强制部署全部
```

### 打包（package.ps1 / package-linux.sh）

```powershell
# Windows
.\package.ps1                   # 完整构建 + 打包
.\package.ps1 -SkipBuild        # 复用现有二进制

# Linux
bash package-linux.sh
bash package-linux.sh --skip-build
```

### 交叉部署（huayu-deploy.sh）

```bash
bash huayu-deploy.sh                # 完整：编译 Win+Linux → 打包 → scp
bash huayu-deploy.sh --skip-win     # 仅 Linux
bash huayu-deploy.sh --skip-linux   # 仅 Windows
bash huayu-deploy.sh --skip-build   # 复用已有二进制
```

---

## 测试策略

### 运行

```bash
cargo test
```

当前 **56 个测试全部通过**。

### 覆盖范围

#### 配置层 (`config.rs`)
- 默认值验证 / 序列化 round-trip
- Codex `config.toml` + `auth.json` 生成正确性
- Claude `settings.json` + `config.json` 生成正确性
- 工具专用模型覆盖逻辑
- 所有配置文件位于 `HUAYU_CONFIG_DIR` 内
- API Key 脱敏输出

#### 命令解析 (`command.rs`)
- 全部 slash 命令 + 未知命令 + 非命令返回 `None`
- `UpdateTarget::tool_names()` 正确性

#### 事件解析 (`tool.rs`)
- 认证错误（401 / invalid key / auth failed）
- 网络错误（connection refused）
- 文件写入（wrote/created）+ 测试通过/失败
- 普通行直通

#### 二进制管理 (`installer.rs`)
- bundled 优先于 PATH / npm node_modules 回退
- tools_dir 为空/不存在返回 `None`
- 版本匹配/过期/缺失
- Windows `.exe` 后缀处理

#### TUI 状态机 (`app.rs`)
- 输入历史导航（空历史 / 循环 / 到底恢复草稿）
- submit 行为（空输入 / 未登录 / /help / /clear）
- `apply_settings` 更新内存并关闭弹窗

#### 登录服务 (`services/login.rs`)
- token 格式（32 位 hex）
- token 唯一性
- URL 格式正确性

### 测试隔离 Seam

| Seam | 用途 |
|------|------|
| `HUAYU_CONFIG_DIR` 环境变量 | 覆盖配置目录路径 |
| `CONFIG_LOCK` + `TempConfigGuard` | 并发测试互斥 |
| Command parser | 纯函数，无副作用 |
| Tool event parser | 纯函数，无副作用 |
| App 状态方法 | 不渲染终端即可验证行为 |

### 已知未覆盖项（设计上推迟）

- PS1 安装脚本 → 独立 Pester 测试
- login polling → 需 mocked HTTP
- model fetch → 需 mocked API
- 下载进度 channel → 需 mocked HTTP

---

## 错误处理

### 错误类型（`AppError`）

| 变体 | 错误信息 | 用户操作 |
|------|---------|---------|
| `Io` | IO error: ... | 检查磁盘权限与路径 |
| `Config` | Config error: ... | 检查 `config.json` 语法 |
| `Network` | Network error: ... | 检查网络与 baizor.com 可达性 |
| `Auth` | please run `huayu login` | 运行 `/login` 重新认证 |
| `ToolNotFound` | not installed or not in PATH | 运行 `/update` |
| `Json` | JSON error: ... | 检查对应 JSON 文件格式 |
| `Message` | 自定义消息 | 查看错误详情 |

### 连接状态

| 状态 | 图标 | 含义 |
|------|------|------|
| `Connected` | ● | API Key 已配置，网络正常 |
| `NotConfigured` | ○ | 未配置 API Key |
| `AuthError` | ✗ | 认证失败，需重新登录 |
| `NetworkError` | ✗ | 服务不可达 |
| `ToolNotFound` | ✗ | 对应工具未安装 |

---

## 设计约定

- **单根配置目录** `~/.huayu/`，通过 `HUAYU_CONFIG_DIR` 可覆盖
- **Codex 配置注入**：`CODEX_HOME` 环境变量；Claude：`CLAUDE_CONFIG_DIR`
- **工具版本锁定**：Codex `0.142.5`，Claude `1.0.3`；版本号经 CI 兼容测试后更新
- **安装进度异步流式输出**，以 `__DONE__` 哨兵结束
- **最近命令仅记录 slash 命令**（不含自由文本 prompt），最多 50 条
- **PS1 安装脚本无需管理员权限**：安装到 User Profile，PATH 写入 User 级注册表
- **Windows UTF-8 初始化**：`SetConsoleCP(65001)` + `SetConsoleOutputCP(65001)` 避免乱码
- **bundled 二进制优先**：`tools/` 目录优先于 PATH；兼容 npm node_modules 回退
- **启动时自动重写工具配置**：确保格式变更后配置始终最新
- **未登录自动弹出登录弹窗**
- **debug.log 记录完整工具输出**：不受 TUI 列宽截断
- **PTY 隔离**：每个工具独立的 PTY 子进程，支持交互式 y/n 确认
- **配置不跨会话污染**：Codex/Claude 配置文件由 huayu 完全管理，不继承全局设置

---

## 源码文件清单

```
huayu/                          # 项目根目录
├── src/
│   ├── main.rs                 # 入口：CLI 解析、UTF-8 初始化
│   ├── config.rs               # 配置中心（503 行）
│   ├── command.rs              # 斜杠命令解析器
│   ├── tool.rs                 # PTY 工具进程管理（424 行）
│   ├── error.rs                # 统一错误类型
│   ├── cli/
│   │   ├── mod.rs              # clap 子命令定义
│   │   └── commands/
│   │       ├── login.rs        # 浏览器登录 CLI
│   │       ├── status.rs       # 状态查看 CLI
│   │       └── update.rs       # 工具更新 CLI
│   ├── tui/
│   │   ├── mod.rs              # 事件循环（ratatui + crossterm）
│   │   ├── app.rs              # App 状态机（1000+ 行）
│   │   ├── ui.rs               # 界面渲染
│   │   └── theme.rs            # 主题色彩
│   └── services/
│       ├── mod.rs
│       ├── installer.rs        # 工具下载与版本管理（349 行）
│       ├── login.rs            # 浏览器登录轮询
│       └── model_fetch.rs      # 模型列表获取
├── Cargo.toml                  # Rust 项目配置
├── Cargo.lock
├── versions.json               # 版本唯一来源
├── build.ps1                   # 智能构建脚本（指纹检测）
├── deploy.ps1                  # 智能部署脚本（版本对比）
├── package.ps1                 # Windows 打包脚本
├── package-tools.ps1           # 工具打包辅助
├── package-linux.sh            # Linux 打包脚本
├── build-linux.sh              # Linux 构建脚本
├── build-tools-linux.sh        # Linux 工具构建
├── deploy.sh                   # Linux 部署
├── huayu-deploy.sh             # 交叉部署脚本
├── huayu-deploy-readme.md      # 部署文档
├── .gitignore
├── .build-state.json           # 构建状态（自动生成，不提交）
├── PRD20260705.md              # 产品需求文档
├── doc/                        # 文档目录
│   ├── readme.md
│   ├── readmev1.md
│   ├── readmev2.md
│   └── build-deploy.md
├── docs/
│   └── huayu-readme.md
└── release/                    # 构建产物（不提交）
```

---

## 工作范围外

以下功能明确不在当前版本范围内：

- macOS / Linux 一键安装脚本（仅 Windows PS1；其他平台手动下载 tar.gz）
- Web UI
- Gemini、OpenCode、Hermes 等其他 AI 编码工具
- 跨会话持久化聊天历史
- 多账户管理
- 华宇自身自动更新（重新运行安装脚本更新）
- ratatui 完整终端快照测试
- 管理用户系统级 Node.js 或 npm 安装
- 鼠标拖拽分栏调整大小
- 修改 baizor.com 服务端 API 行为

---

*最后更新：2026-07-07 · huayu v0.2.0 · 56 测试全通过 · 基于源码深度分析生成*
