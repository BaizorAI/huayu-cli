# 华宇 huayu — 技术文档

> Rust TUI 终端 AI 客户端，统一封装 Codex 与 Claude Code，面向 [baizor.com](https://baizor.com) 平台。

---

## 目录

1. [项目概览](#项目概览)
2. [技术栈](#技术栈)
3. [源码架构](#源码架构)
4. [模块详解](#模块详解)
5. [数据流](#数据流)
6. [配置体系](#配置体系)
7. [构建与部署](#构建与部署)
8. [测试策略](#测试策略)
9. [安全设计](#安全设计)

---

## 项目概览

| 维度 | 详情 |
|------|------|
| **语言** | Rust 2021 edition |
| **版本** | 0.2.6 |
| **二进制大小** | ~2.1 MB (Windows x64 zip) |
| **集成工具** | Codex 0.142.5 + Claude Code 1.0.3 |
| **平台** | Windows x64, Linux x64 / aarch64, macOS x64 / aarch64 |
| **配置目录** | `~/.huayu/` (可通过 `HUAYU_CONFIG_DIR` 覆盖) |
| **依赖** | clap, serde, tokio, reqwest, ratatui, crossterm, portable-pty, zip, tar, flate2 等 |
| **测试** | 56 个单元测试全部通过 |

### 核心特性

- 统一 TUI 界面，Tab 一键切换 Codex ↔ Claude
- PTY 伪终端子进程运行 AI 工具，实时输出流式渲染
- 67% / 33% 分栏布局：左主输出 + 右帮助面板
- 斜杠命令体系（`/model`、`/switch`、`/update`、`/clear` 等）
- 浏览器 OAuth 登录，自动拉取 API Key 和服务器端配置
- 一键安装脚本，无需 Node.js / npm / 管理员权限
- 隔离配置：Codex 和 Claude 的配置文件完全由 huayu 管理
- 版本锁定：工具版本经 CI 兼容性测试后更新
- 完整 debug.log 审计日志

---

## 技术栈

| 领域 | 库 | 用途 |
|------|-----|------|
| **CLI 解析** | clap 4.5 (derive) | 子命令路由（login / status / update） |
| **TUI 渲染** | ratatui 0.29 + crossterm 0.28 | 终端 UI 界面、键盘/鼠标事件 |
| **异步运行时** | tokio (rt-multi-thread) | 网络请求、OAuth 轮询 |
| **HTTP 客户端** | reqwest 0.12 (rustls-tls) | API 调用、工具下载 |
| **序列化** | serde + serde_json | 配置读写、JSON 解析 |
| **PTY** | portable-pty 0.8 | 伪终端子进程管理 |
| **ANSI 清理** | strip-ansi-escapes | 去除终端控制字符 |
| **压缩** | zip + flate2 + tar | 工具二进制解压 |
| **跨平台** | dirs + which | 用户目录、PATH 解析 |
| **UUID** | uuid (v4) | OAuth 登录 token 生成 |

---

## 源码架构

```
src/
├── main.rs              # 入口点：CLI 解析 → 路由到 CLI 命令或 TUI
├── config.rs            # 配置管理：HuayuConfig、Codex/Claude 配置生成、输入历史
├── error.rs             # 统一错误类型 (thiserror)
├── command.rs           # 斜杠命令解析器 (/login、/switch、/model 等)
├── tool.rs              # 工具抽象层：ToolType、ToolEvent、ToolProcess、PTY 管理
├── cli/
│   ├── mod.rs           # CLI 子命令枚举 (clap)
│   └── commands/
│       ├── login.rs     # huayu login — 浏览器 OAuth 登录
│       ├── status.rs    # huayu status — 配置与工具状态
│       └── update.rs    # huayu update — 工具下载/更新
├── services/
│   ├── mod.rs
│   ├── installer.rs     # 工具下载/安装（优先二进制包，回退 npm）
│   ├── login.rs         # OAuth 轮询逻辑、LoginService
│   └── model_fetch.rs   # 从 /v1/models 拉取可用模型列表
└── tui/
    ├── mod.rs           # TUI 运行循环、键盘/鼠标事件处理
    ├── app.rs           # App 状态机：输入管理、工具启动、历史导航、Overlay
    ├── theme.rs         # 颜色常量定义
    └── ui.rs            # ratatui 渲染：状态栏、主面板、输入框、弹窗
```

---

## 模块详解

### `main.rs` — 入口点

```
main()
  ├── Windows: SetConsoleOutputCP(65001) + SetConsoleCP(65001)
  ├── Cli::parse() → 子命令路由
  ├── None           → config::load() → tui::run(config)
  ├── Login(args)    → cli::commands::login::execute(args)
  ├── Status         → cli::commands::status::execute()
  └── Update {tool}  → cli::commands::update::execute(target.tool_names())
```

Windows 下设置 UTF-8 代码页（65001），解决中文、框线符号、状态图标的乱码问题。

### `config.rs` — 配置管理

- **`HuayuConfig`**：主配置结构，包含 API Key、Base URL、模型、工具偏好、Codex/Claude 专属设置、模型元数据。
- **`config_dir()`**：根配置目录 `~/.huayu/`，可通过 `HUAYU_CONFIG_DIR` 环境变量覆盖（测试隔离 seam）。
- **`write_codex_config()`**：生成 Codex 的 `config.toml`（含 model_provider、base_url、wire_api、model_info）+ `auth.json`（含 OPENAI_API_KEY）。
- **`write_claude_config()`**：生成 Claude 的 `settings.json`（含 ANTHROPIC_AUTH_TOKEN、ANTHROPIC_BASE_URL、ANTHROPIC_MODEL）+ `config.json`。
- **`load_input_history()` / `save_input_history()`**：持久化斜杠命令历史（最多 50 条），不含自由文本 prompt。

### `error.rs` — 错误类型

| 变体 | 触发场景 |
|------|---------|
| `Io` | 文件读写失败 |
| `Config` | 配置语法/格式错误 |
| `Network` | 网络请求失败 |
| `Auth` | 认证失败（提示 `huayu login`） |
| `ToolNotFound` | 工具二进制缺失 |
| `Json` | JSON 解析错误 |
| `Message` | 通用自定义错误 |

### `command.rs` — 斜杠命令解析

纯函数 `parse(input: &str) -> Option<AppCommand>`，输入以 `/` 开头则解析为命令：

| 命令 | 变体 |
|------|------|
| `/login` | `AppCommand::Login` |
| `/switch <tool>` | `AppCommand::Switch(String)` |
| `/model <name>` | `AppCommand::Model(String)` |
| `/update [codex\|claude]` | `AppCommand::Update(UpdateTarget)` |
| `/install` | 与 `/update` 同义（肌肉记忆兼容） |
| `/status` | `AppCommand::Status` |
| `/help`、`/?` | `AppCommand::Help` |
| `/clear` | `AppCommand::Clear` |
| `/quit`、`/exit`、`/q` | `AppCommand::Quit` |
| 其他 | `AppCommand::Unknown(String)` |

非 `/` 开头的输入返回 `None`，由 App 作为普通 prompt 处理。

### `tool.rs` — 工具抽象层

#### `ToolType`

```rust
pub enum ToolType { Codex, Claude }
```

- `binary_path()`：优先返回 `~/.huayu/tools/` 下的本地二进制，其次 PATH。
- `is_available()`：检查二进制是否存在。

#### `ToolEvent`

```rust
pub enum ToolEvent {
    Line(String),           // 普通输出行
    FileWritten(String),    // 文件写入事件
    TestPassed,             // 测试通过
    TestFailed(String),     // 测试失败
    AuthError,              // 认证错误 (401 / invalid key)
    NetworkError,           // 网络错误
    Done,                   // 子进程结束
    Error(String),          // 通用错误
}
```

`parse_event(line)` 通过关键字匹配将 PTY 输出的原始行分类为结构化事件。

#### `ToolProcess`

```rust
pub struct ToolProcess {
    process_id: Option<u32>,
    writer: Box<dyn Write + Send>,    // PTY master 写入端
    pub rx: mpsc::Receiver<ToolEvent>, // 事件接收通道
}
```

- `spawn()`：创建 PTY、启动子进程、后台线程读取输出并写入 `debug.log`。
- `write_input()`：向工具 stdin 发送文本。
- `kill()`：终止子进程。

### `services/installer.rs` — 工具下载

- 优先从 `https://baizor.com/install/` 下载预编译二进制包（zip/tar.gz）。
- 若二进制不可用，回退到 `npm install`。
- 版本锁定：`CODEX_VERSION = "0.142.5"`、`CLAUDE_VERSION = "1.0.3"`。
- `download_tools()` 通过 `mpsc::channel` 异步推送进度，`"__DONE__"` 哨兵结束。

### `services/login.rs` — OAuth 登录

- `LoginService::generate_token()`：生成 32 位 hex UUID 作为轮询 token。
- `LoginService::poll_for_key()`：每 2 秒轮询 `/api/cli/poll?token=xxx`，最多 5 分钟超时。
- 服务器返回 `LoginOutcome`：api_key、模型配置、Codex/Claude 专属设置、model_info。

### `tui/mod.rs` — TUI 运行循环

```rust
pub fn run(config: HuayuConfig) -> Result<()>
```

- 启用 raw mode + alternate screen。
- 创建 `App` 状态机，若未登录自动弹出登录 Overlay。
- 100ms tick 循环：渲染 UI → 读取键盘/鼠标事件 → 排空工具事件 → 排空更新进度 → 轮询登录。
- `handle_key()` 按状态分层处理：Login Overlay > Settings Overlay > Main View。

### `tui/app.rs` — App 状态机

核心状态字段：

| 字段 | 说明 |
|------|------|
| `config` | 当前配置 |
| `tool_type` | 当前激活工具 (Codex/Claude) |
| `tool_process` | 运行中的工具子进程 |
| `messages` | 对话上下文 |
| `main_lines` | 主输出面板文本行 |
| `scroll_offset` / `auto_scroll` | 滚动状态 |
| `input` / `cursor_pos` | 输入框状态 |
| `input_history` / `history_cursor` | 输入历史导航 |
| `login_overlay` | 登录弹窗 |
| `show_settings` | 设置弹窗 |

关键方法：
- `submit()`：提交输入，处理斜杠命令和普通 prompt。
- `switch_tool()`：切换 Codex/Claude，终止当前进程。
- `start_tool()`：通过 `ToolProcess::spawn()` 启动工具并注入环境变量。
- `drain_tool_events()`：排空工具输出事件，分类渲染。

### `tui/ui.rs` — 界面渲染

布局结构：

```
+-- 状态栏 (height=1) ------------------------------------+
| 华宇 huayu | codex | [Tab切换] | 模型名 | ● 已连接       |
+-- 主面板 (min=5) ---------------------------------------+
| [左: 70%]                     | [右: 30%]               |
| 工具流式输出                   | 快捷键提示               |
| 支持滚动、文件事件高亮          | 最近命令                |
+-- 输入框 (height=4) ------------------------------------+
| > 在此输入...                                           |
+-- 快捷键栏 (height=1) -----------------------------------+
| [Enter]发送 [Esc]取消 [Tab]切换 [PgUp/PgDn]翻页 ...     |
+---------------------------------------------------------+
```

### `tui/theme.rs` — 颜色常量

```rust
pub const BORDER: Color     = Rgb(70, 90, 120);   // 边框
pub const TITLE: Color      = Rgb(140, 180, 255); // 标题
pub const STATUS_OK: Color  = Rgb(80, 200, 120);  // 连接正常
pub const STATUS_ERR: Color = Rgb(220, 80, 80);   // 错误
pub const STATUS_WARN: Color = Rgb(220, 180, 60); // 警告
pub const HIGHLIGHT: Color  = Rgb(255, 210, 100); // 高亮
pub const DIM: Color        = Rgb(100, 100, 100); // 次要文本
```

---

## 数据流

### 启动流程

```
huayu (无子命令)
  └── config::load()
  └── tui::run(config)
        ├── App::new(config)
        ├── write_codex_config() + write_claude_config()  (若已登录)
        ├── 若未登录 → open_login_overlay()
        └── 事件循环 (100ms tick)
              ├── 渲染 UI
              ├── 键盘事件 → handle_key()
              ├── drain_tool_events() → 分类渲染
              ├── drain_update() → 安装进度
              └── poll_login() → OAuth 轮询
```

### 用户发送 Prompt 流程

```
用户输入 + Enter
  └── App::submit()
        ├── 斜杠命令? → 执行命令（如 /help 打印帮助行）
        ├── 普通文本:
        │     ├── 未登录? → 提示 /login
        │     └── 已登录:
        │           ├── 写入输入历史
        │           ├── push_output("> {input}")
        │           └── ToolProcess::spawn(tool_type, config, messages)
        │                 ├── 配置 PTY 环境变量 (CODEX_HOME, CLAUDE_CONFIG_DIR, API key)
        │                 ├── 启动 codex/claude 子进程
        │                 └── 后台线程: 读取输出 → strip ANSI → 写入 debug.log → parse_event → tx.send
        └── drain_tool_events()
              ├── AuthError → 更新 connection_status → 提示重登录
              ├── FileWritten → 绿色高亮
              ├── TestPassed / TestFailed → 状态标记
              ├── NetworkError → 更新连接状态
              └── Done → 清理 tool_process
```

### OAuth 登录流程

```
huayu login
  └── LoginService::generate_token()
  ├── 打开浏览器: https://baizor.com/code/token?token=xxx
  └── 轮询: GET /api/cli/poll?token=xxx (每 2s，最多 5 分钟)
        ├── status=pending → 继续轮询
        ├── status=done + key → LoginOutcome
        └── 超时 → 错误
  └── 更新 config.json + codex 配置 + claude 配置
```

---

## 配置体系

### 目录结构

```
~/.huayu/                        # HUAYU_CONFIG_DIR
├── bin/
│   └── huayu.exe                # 华宇主程序
├── tools/                       # 捆绑工具（由 /update 管理）
│   ├── codex.exe
│   ├── codex.version
│   ├── claude
│   └── claude.version
├── codex/                       # Codex 隔离配置（CODEX_HOME）
│   ├── config.toml
│   └── auth.json
├── claude/                      # Claude 隔离配置（CLAUDE_CONFIG_DIR）
│   ├── settings.json
│   └── config.json
├── config.json                  # huayu 主配置
├── history.json                 # 输入历史（最多 50 条）
└── debug.log                    # 完整工具输出日志
```

### 设计原则

| 原则 | 说明 |
|------|------|
| **单根配置目录** | `~/.huayu/` 包含所有配置和工具，可整体迁移 |
| **零外部依赖** | 捆绑锁定版本二进制，不需要 Node.js / npm |
| **启动自动配置** | 工具配置文件由 huayu 生成与管理 |
| **bundled 优先** | `tools/` 目录二进制优先于系统 PATH |
| **隔离管理** | Codex/Claude 配置完全由 huayu 管理，不读取全局配置 |
| **版本锁定** | 工具版本号写入 `*.version` 文件，不会自动升级 |

### 环境变量注入

当 huayu 启动 Codex 子进程时注入：
- `CODEX_HOME` = `~/.huayu/codex/`
- `OPENAI_API_KEY` (通过 auth.json)
- `CODEX_BASE_URL` (通过 config.toml)

当 huayu 启动 Claude 子进程时注入：
- `CLAUDE_CONFIG_DIR` = `~/.huayu/claude/`
- `ANTHROPIC_AUTH_TOKEN`
- `ANTHROPIC_BASE_URL`
- `ANTHROPIC_MODEL`

---

## 构建与部署

### 构建系统

```
versions.json            → 版本唯一来源
    ↓
build.ps1                → 智能构建（指纹对比 → 版本 bump → 同步 → 构建）
    ↓
    ├── Cargo.toml, installer.rs, package-tools.ps1, package-linux.sh  (版本同步)
    ├── cargo build --release → package.ps1                             (huayu)
    └── package-tools.ps1                                               (codex/claude)
    ↓
.build-state.json        → 构建状态记录（指纹+版本，不提交）
    ↓
deploy.ps1               → 智能部署（本地版本 vs 远程版本 → 按需 scp）
    ↓
baizor:/lucky/NewApi/data/install/  (服务器)
```

### 组件变更检测

| 组件 | 指纹来源 | 变更含义 |
|------|---------|---------|
| huayu | `src/**/*.rs` + `Cargo.toml` + `Cargo.lock` 的 SHA256 | 源代码修改 |
| codex | `versions.json` 中的版本字符串 | 手动升级版本 |
| claude | `versions.json` 中的版本字符串 | 手动升级版本 |

### 构建命令

```powershell
# 构建有变更的组件
.\build.ps1

# 强制全量构建
.\build.ps1 -Force

# 仅构建指定组件
.\build.ps1 -Component huayu
```

### 打包命令

```powershell
# 完整编译 + 打包 zip
.\package.ps1

# 跳过编译，复用已有二进制
.\package.ps1 -SkipBuild
```

### 部署命令

```powershell
.\deploy.ps1              # 部署有版本变更的组件
.\deploy.ps1 -DryRun      # 仅显示差异
.\deploy.ps1 -Force       # 强制部署全部
```

---

## 测试策略

### 测试覆盖

**56 个单元测试全部通过**，覆盖范围：

| 模块 | 覆盖内容 |
|------|---------|
| **config** | 默认值、save/load round-trip、Codex/Claude 配置文件内容、文件隔离、API Key 掩码 |
| **command** | 全部斜杠命令解析（login/switch/model/update/install/status/help/?/clear/quit/exit/q/unknown）、非命令返回 None、UpdateTarget::tool_names |
| **tool event** | Auth 错误（401 / invalid key / auth failed）、Network 错误、File written、Test pass/fail、Normal line 直通 |
| **binary resolution** | bundled 优先于 PATH、tools_dir 为空/不存在返回 None、版本匹配/过期/缺失、Windows `.exe` 后缀 |
| **app** | 输入历史导航（up from empty / cycle / down past end restores draft）、submit 行为（empty / not logged in / help / clear）、apply_settings 更新内存并关闭弹窗 |

### 测试 Seam

- `HUAYU_CONFIG_DIR` 环境变量 + `CONFIG_LOCK` + `TempConfigGuard`：并发安全的配置隔离。
- 命令解析器 seam：纯函数，无需外部环境。
- Tool event 解析器 seam：纯函数，输入文本行即可验证。
- App 状态方法：直接测试 TUI 行为逻辑，不渲染终端。

---

## 安全设计

1. **API Key 掩码**：输出时仅显示 `sk-xxxx***xxxx`（前 4 + 后 4 字符）。
2. **调试模式**：通过 `DEBUG` 环境变量控制，默认仅 debug build 启用。
3. **配置隔离**：Codex / Claude 配置文件完全由 huayu 管理，不读取全局配置。
4. **PTY 隔离**：每个工具运行在独立的伪终端子进程中。
5. **启动自动重写**：每次启动自动重写工具配置，确保格式变更后始终正确。
6. **版本锁定**：工具版本经 CI 兼容测试后才更新，不自动升级。
7. **输入历史隐私**：仅记录 slash 命令（不含自由文本 prompt），最多 50 条。
8. **非破坏性退出**：`Alt+Q` 而非单独 `q` 键退出。

---

## 版本矩阵

| 组件 | 版本 | 更新策略 |
|------|------|----------|
| huayu | 0.2.6 | 发布新 Release |
| Codex | 0.142.5 | CI 兼容测试后更新 CODEX_VERSION |
| Claude Code | 1.0.3 | CI 兼容测试后更新 CLAUDE_VERSION |

---

*最后更新: 2026-07-07 · huayu v0.2.6 · 56 测试全通过 · 基于完整源码分析生成*
