# 华宇 huayu v4 — AI 编码工作站

> 华宇 (huayu) 是面向 [baizor.com](https://baizor.com) 的 AI 编码工作站，将 **Codex** 与 **Claude Code** 统一封装为 Rust TUI 终端应用，零外部依赖、一行命令安装。

---

## 目录

1. [概览](#概览)
2. [快速开始](#快速开始)
3. [架构总览](#架构总览)
4. [源码详解](#源码详解)
5. [配置体系](#配置体系)
6. [Slash 命令](#slash-命令)
7. [TUI 界面](#tui-界面)
8. [工具进程管理](#工具进程管理)
9. [构建与打包](#构建与打包)
10. [测试策略](#测试策略)
11. [错误处理与安全](#错误处理与安全)
12. [设计原则](#设计原则)
13. [路线图与边界](#路线图与边界)

---

## 概览

| 维度 | 详情 |
|------|------|
| **项目名** | huayu (华宇) |
| **语言与版本** | Rust 2021 edition, v0.2.3 |
| **核心依赖** | clap 4.5, ratatui 0.29, crossterm 0.28, portable-pty 0.8, tokio + reqwest 0.12 |
| **集成工具** | Codex 0.142.5 / Claude Code 1.0.3 |
| **目标平台** | Windows x64, Linux x64+aarch64, macOS x64+aarch64 |
| **配置目录** | ~/.huayu/ (通过 HUAYU_CONFIG_DIR 覆盖) |
| **测试覆盖** | 56 个单元测试全部通过 |
| **安装方式** | 一行 PS1 命令，无需 Node.js/npm/管理员权限 |

---

## 快速开始

### 一键安装 (Windows)

`powershell
irm https://baizor.com/install/huayu.ps1 | iex
`

安装后即可使用：

`ash
huayu login        # 浏览器登录 baizor.com
huayu status       # 查看工具与配置状态
huayu update       # 更新 codex + claude
huayu              # 启动 TUI 工作站
`

### 安装后目录结构

`
~/.huayu/                        # HUAYU_CONFIG_DIR
├── bin/huayu.exe                # 华宇本体
├── tools/                       # 捆绑工具 (由 /update 管理)
│   ├── codex.exe / codex.version
│   └── claude      / claude.version
├── codex/                       # Codex 隔离配置 (CODEX_HOME)
│   ├── config.toml
│   └── auth.json
├── claude/                      # Claude 隔离配置 (CLAUDE_CONFIG_DIR)
│   ├── settings.json
│   └── config.json
├── config.json                  # huayu 主配置
├── history.json                 # 输入历史 (最多 50 条)
└── debug.log                    # 完整工具输出日志
`

---

## 架构总览

`
                      main.rs
                  CLI 解析 → TUI 或子命令
                          │
        ┌─────────────────┼─────────────────┐
        │                 │                 │
   cli/                  tui/           services/
   ├─ login (浏览器认证)    ├─ mod (事件循环)  ├─ installer (下载/版本管理)
   ├─ status (状态查看)     ├─ app (状态机)    ├─ login (OAuth 轮询)
   └─ update (工具安装)     ├─ ui  (渲染)     └─ model_fetch (模型列表)
                           └─ theme (主题)

        command.rs          tool.rs          config.rs
        (斜杠命令解析)       (PTY 进程封装)    (配置读写与注入)
                            portable-pty
                              ↓
                     codex / claude 子进程
`

### 运行时数据流

`
用户输入 → command::parse()
  ├─ 斜杠命令 → AppCommand (本地处理)
  └─ 自由文本 → tool::spawn() → PTY → codex/claude
                   ↓
               ToolEvent (Line/FileWritten/AuthError/...)
                   ↓
               app.rs: push_output → main_lines → ui.rs 渲染
                   ↓
               debug.log (完整未截断输出)
`

---

## 源码详解

### src/main.rs — 入口

- 解析 CLI 参数 (clap derive): 无子命令进入 TUI，有子命令分发到 login/status/update
- Windows 平台 UTF-8 初始化: SetConsoleCP(65001) + SetConsoleOutputCP(65001) 避免中文乱码
- 错误统一由 AppError 类型处理并输出到 stderr

### src/config.rs (503 行) — 配置中心

**数据结构 HuayuConfig**:

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| api_key | String | "" | baizor.com API Key |
| base_url | String | "https://baizor.com" | API 基础地址 |
| default_model | String | "huayu-v2" | 默认模型 |
| active_tool | String | "codex" | 当前活动工具 |
| codex_model | String | "" | Codex 专用模型 (覆盖 default_model) |
| codex_full_auto | bool | true | Codex 全自动模式 |
| codex_reasoning_effort | String | "medium" | Codex 推理深度 |
| claude_model | String | "" | Claude 专用模型 |
| claude_max_turns | u32 | 0 | Claude 最大轮次 |
| claude_permission_mode | String | "bypassPermissions" | Claude 权限模式 |
| model_info | HashMap<String, ModelInfo> | {} | 模型元数据 (context_window, max_output_tokens) |

**关键方法**:

| 方法 | 功能 |
|------|------|
| config_dir() | 返回 $HUAYU_CONFIG_DIR 或 ~/.huayu/ |
| load() / save() | JSON 序列化加载与持久化 |
| write_codex_config() | 注入 Codex config.toml 和 uth.json 到 CODEX_HOME |
| write_claude_config() | 注入 Claude settings.json 和 config.json 到 CLAUDE_CONFIG_DIR |
| effective_codex_model() | codex_model 非空则返回之，否则返回 default_model |
| effective_claude_model() | claude_model 非空则返回之，否则返回 default_model |
| load_input_history() / save_input_history() | 历史记录持久化 (最新 50 条) |

**Codex Config 注入逻辑** (write_codex_config):
1. 生成 config.toml: 包含 model_provider = "custom", ase_url = "{base_url}/v1", wire_api = "responses", 模型名与 model_info 元数据
2. 生成 uth.json: {"OPENAI_API_KEY": "{api_key}"}
3. 自动合并服务器下发的 model_info，补充内置回退值 (huayu 系列模型)

**Claude Config 注入逻辑** (write_claude_config):
1. 生成 settings.json: 包含 ANTHROPIC_AUTH_TOKEN, ANTHROPIC_BASE_URL, ANTHROPIC_MODEL, ypassPermissionsModeAccepted
2. 生成 config.json: 包含 ypassPermissionsModeAccepted, hasCompletedOnboarding

### src/command.rs — Slash 命令解析

`ust
pub enum AppCommand {
    Login,                    // /login
    Switch(String),           // /switch codex|claude
    Model(String),            // /model <name>
    Update(UpdateTarget),     // /update [codex|claude|all]
    Status,                   // /status
    Help,                     // /help | /?
    Clear,                    // /clear
    Quit,                     // /quit | /exit | /q
    Unknown(String),          // any /xxx not matching above
}
`

parse() 函数: 以 / 开头的输入解析为命令，其余视为自由文本直接交给当前工具处理。

### src/tool.rs (424 行) — 工具进程管理

**核心类型**:

| 类型 | 说明 |
|------|------|
| ToolType | 枚举: Codex, Claude；提供 binary(), binary_path(), is_available() |
| ToolEvent | 结构化事件: Line, FileWritten, TestPassed, TestFailed, AuthError, NetworkError, Done, Error |
| ToolProcess | 运行中的 PTY 子进程句柄: writer (stdin), rx (事件通道), process_id |
| Message | 对话消息: role ("user"|"assistant"), text |

**parse_event() 事件识别规则**:

| 事件 | 匹配模式 |
|------|----------|
| AuthError | "401 Unauthorized", "authentication failed", "invalid api key", "incorrect api key" |
| NetworkError | "Connection refused", "network" + "error" |
| FileWritten | "wrote ...", "written ...", "created ..." |
| TestPassed | "test" + ("pass" 或 " ok") |
| TestFailed | "test" + "fail" |

**spawn() PTY 启动流程**:
1. 解析工具路径 (优先 	ools/ 目录，回退 PATH)
2. 构建命令行参数 (包含 model、api_key 等环境变量)
3. 使用 portable_pty 创建 PTY master/slave 对
4. 设置终端大小 (120x40)
5. 启动子进程并连接 PTY
6. 启动读取线程: 移除 ANSI 转义序列 → 写入 debug.log → 解析为 ToolEvent → 发送至 channel
7. 子进程退出时发送 ToolEvent::Done

**安全性保障**:
- 独立 PTY 子进程，每个工具运行在隔离的伪终端中
- debug.log 记录完整输出，不受 TUI 截断
- kill() 方法可随时终止子进程

### src/tui/app.rs (1000+ 行) — 状态机

**App 结构体**:

| 字段 | 类型 | 说明 |
|------|------|------|
| config | HuayuConfig | 当前配置 |
| tool_type | ToolType | 活动工具 |
| tool_process | Option<ToolProcess> | 运行中的工具进程 |
| connection_status | ConnectionStatus | 连接状态 |
| messages | Vec<Message> | 对话历史上下文 |
| main_lines | Vec<String> | 统一输出面板内容 |
| scroll_offset | usize | 滚动偏移 (0=底部) |
| auto_scroll | bool | 自动跟随滚动 |
| input / cursor_pos | String / usize | 输入框状态 |
| input_history | Vec<String> | 会话历史 |
| history_cursor | Option<usize> | 历史导航位置 |
| task_start | Option<Instant> | 任务计时 |
| update_rx | Option<Receiver<String>> | 后台下载进度 |
| login_overlay | Option<LoginOverlay> | 登录弹窗 |
| show_settings | bool | 设置弹窗可见性 |
| recent_commands | Vec<String> | 最近命令 (最多 5 条) |
| debug | bool | 调试模式 |

**连接状态**:

| 状态 | 含义 | 图标 |
|------|------|------|
| Connected | API Key 已配置 | ● |
| NotConfigured | 未配置 API Key | ○ |
| AuthError | 认证失败 | ✗ |
| NetworkError | 服务不可达 | ✗ |
| ToolNotFound(String) | 工具未安装 | ✗ |

**核心方法**:

| 方法 | 功能 |
|------|------|
| submit() | 提交输入: 解析命令 → 或启动 PTY 工具 |
| handle_tool_events() | 处理工具输出事件流 |
| history_up() / history_down() | 输入历史导航 |
| push_output() | 追加行到输出面板 |
| pply_settings() | 保存模型/Base URL 设置 |
| start_login() | 发起浏览器登录流程 |

### src/tui/ui.rs — 界面渲染

**布局结构** (基于 ratatui):

`
+-- header: 工具 | 模型 | 连接状态 -------------------------------+
|  左面板 (70%)              |  右面板 (30%)                    |
|  main_lines 滚动视图       |  快捷键速查                        |
|                            |  当前状态 (工具/模型/目录)           |
|                            |  最近命令                          |
+------------------------------------------------------------------+
| > 输入框                                                        |
+------------------------------------------------------------------+

弹窗覆盖层:
- Login Overlay: 登录 URL + 等待状态
- Settings Overlay: 模型/Base URL 编辑 (Tab 切换字段)
`

**主题色彩** (src/tui/theme.rs):
- TITLE: 标题色
- HIGHLIGHT: 高亮/选中色
- PROMPT: 输入提示色
- STATUS_OK / STATUS_ERR: 状态色
- DIM: 辅助文本色

### src/cli/ — CLI 子命令

| 子命令 | 文件 | 功能 |
|--------|------|------|
| huayu login | cli/commands/login.rs | 浏览器 OAuth 登录，轮询获取 API Key，自动写入 Codex/Claude 配置 |
| huayu status | cli/commands/status.rs | 显示 API Key (掩码)、Base URL、模型、工具可用性 |
| huayu update [tool] | cli/commands/update.rs | 下载/更新指定工具 (默认全部) |

### src/services/ — 服务层

| 服务 | 功能 |
|------|------|
| installer.rs | 工具下载: 目标平台三元组拼接 URL、异步下载解压、版本比较 (.version 文件) |
| login.rs | 登录轮询: 生成 token → 浏览器打开 → 轮询 baizor.com API → 获取 API Key + 模型配置 |
| model_fetch.rs | 获取可用模型列表与元数据 |

**installer.rs 关键常量**:

`ust
const CODEX_VERSION: &str = "0.142.5";
const CLAUDE_VERSION: &str = "1.0.3";
`

**download_tools() 流程**:
1. 创建 tokio runtime (current_thread)
2. 遍历工具列表: 比较本地版本 → 若需更新则下载 archive → 解压到 	ools/ → 写入 .version
3. 通过 mpsc channel 发送进度 (流式)，以 __DONE__ 哨兵标记完成

---

## 配置体系

### 配置注入优先级

`
huayu config.json
    ├─ codex_model    →  CODEX_HOME/config.toml  (model)
    ├─ default_model  →  (fallback if codex_model empty)
    ├─ model_info     →  CODEX_HOME/config.toml  ([model_info])
    ├─ api_key        →  CODEX_HOME/auth.json    (OPENAI_API_KEY)
    │                     CLAUDE_CONFIG_DIR/settings.json (ANTHROPIC_AUTH_TOKEN)
    ├─ claude_model   →  CLAUDE_CONFIG_DIR/settings.json (ANTHROPIC_MODEL)
    └─ base_url       →  CODEX_HOME/config.toml  (base_url)
                          CLAUDE_CONFIG_DIR/settings.json (ANTHROPIC_BASE_URL)
`

### 环境变量

| 变量 | 用途 | 默认值 |
|------|------|--------|
| HUAYU_CONFIG_DIR | 覆盖配置根目录 | ~/.huayu/ |
| CODEX_HOME | Codex 配置目录 (由 huayu 注入) | ~/.huayu/codex/ |
| CLAUDE_CONFIG_DIR | Claude 配置目录 (由 huayu 注入) | ~/.huayu/claude/ |
| DEBUG | 启用调试模式 (显示端点、密钥掩码) | 仅 debug build |

---

## Slash 命令

| 命令 | 说明 |
|------|------|
| /login | 浏览器登录 baizor.com |
| /switch codex\|claude | 切换活动工具 |
| /model <name> | 更改默认模型 |
| /update [codex\|claude] | 下载/更新工具 (默认全部)；/install 为同义词 |
| /status | 显示配置与工具状态 |
| /clear | 清空输出面板 |
| /help 或 /? | 显示帮助 |
| /quit 或 /exit 或 /q | 退出程序 |

---

## TUI 界面

### 快捷键

| 按键 | 动作 |
|------|------|
| Enter | 提交输入 (文本交给当前工具；命令本地执行) |
| Up / Down | 输入历史导航 |
| PgUp / PgDn | 滚动输出面板 |
| 鼠标滚轮 | 滚动输出面板 |
| Alt+Q | 退出程序 |
| Esc | 关闭登录/设置弹窗 |
| Tab | 设置弹窗中切换字段 |
| r | 登录弹窗中重试 |

### 界面组件

**左面板 (70%)**: 统一输出视图
- 工具执行日志、AI 回复、文件事件、错误提示、任务耗时
- 合并展示在同一滚动视图中
- 底部自动锚定，向上滚动后暂停自动滚动，PgDn 恢复

**右面板 (30%)**: 静态参考
- 快捷键速查
- 当前状态: 工具、模型、工作目录
- 最近 5 条 slash 命令

**底部输入栏**: 输入文本或斜杠命令

**弹窗**:
- 登录弹窗: 显示 URL + 等待状态 + 重试/取消
- 设置弹窗: 编辑模型名和 Base URL，Enter 保存 Esc 取消

---

## 工具进程管理

### PTY 架构

`
huayu (TUI)
  │
  ├─ portable_pty::native_pty_system()
  │     ├─ master (读写端, 保持在 huayu 线程)
  │     └─ slave (连接子进程 stdin/stdout/stderr)
  │
  ├─ 子进程: codex/claude
  │     环境变量: CODEX_HOME / CLAUDE_CONFIG_DIR + API Key
  │     参数: --model, --full-auto, --reasoning-effort 等
  │
  └─ 读取线程:
        BufReader → strip_ansi → debug.log → parse_event → mpsc channel
`

### 安全隔离

- **环境变量注入**: 在 spawn() 时设置，不污染父进程
- **配置隔离**: Codex/Claude 只能访问 ~/.huayu/codex/ 和 ~/.huayu/claude/
- **审计日志**: debug.log 记录每次会话完整输出
- **强制终止**: ToolProcess::kill() 可随时停止子进程

---

## 构建与打包

### 构建

`ash
# Windows
.\build.ps1

# Linux
bash build-linux.sh
`

构建脚本使用指纹检测 (.build-state.json)，仅当源文件变更时重新编译。

### 打包

`ash
# Windows: 生成 huayu-x86_64-pc-windows-msvc.zip
.\package.ps1

# Linux: 生成 huayu-x86_64-unknown-linux-gnu.tar.gz
bash package-linux.sh
`

打包内容:
- huayu / huayu.exe: 华宇本体
- 	ools/: 锁版本 codex + claude 二进制 + 版本文件

### 部署

`ash
# Windows
.\deploy.ps1

# Linux 交叉部署
bash huayu-deploy.sh
`

版本管理: ersions.json 为唯一版本来源 (huayu, codex, claude)。

---

## 测试策略

### 测试分布 (56 个)

| 模块 | 测试数 | 覆盖内容 |
|------|--------|----------|
| config.rs | 10 | 配置读写、Codex/Claude 注入、目录隔离、密钥掩码 |
| command.rs | 12 | 所有斜杠命令解析、UpdateTarget 枚举 |
| 	ool.rs | 11 | ToolEvent 事件解析 (认证/网络/文件/测试事件) |
| pp.rs | 18 | 历史导航、提交行为、登录检测、slash 命令、连接状态、设置 |
| services/installer.rs | 5 | URL 生成、版本检查、平台三元组 |

### 测试基础设施

- 	empfile crate: 创建临时配置目录，测试后自动清理
- TempConfigGuard: RAII 守卫，设置 HUAYU_CONFIG_DIR 到临时目录
- 无外部网络依赖: 工具安装测试使用 mock 或条件跳过
- 运行: cargo test 全部通过

---

## 错误处理与安全

### 错误类型 (src/error.rs)

| 变体 | 触发场景 | 用户提示 |
|------|---------|---------|
| Io | 文件读写失败 | "IO error: ..." |
| Config | 配置语法错误 | "Config error: ..." |
| Network | 网络请求失败 | "Network error: ..." |
| Auth | 认证失败 | "please run \huayu login\" |
| ToolNotFound | 工具二进制缺失 | "not installed or not in PATH" |
| Json | JSON 解析错误 | "JSON error: ..." |
| Message | 通用错误 | 自定义消息 |

### 安全设计

1. **API Key 掩码**: 输出时仅显示 sk-xxxx***xxxx (前 4 + 后 4 字符)
2. **调试模式**: 通过 DEBUG 环境变量控制，默认仅 debug build 启用
3. **配置不跨会话污染**: Codex/Claude 配置文件完全由 huayu 管理，不读取全局配置
4. **PTY 隔离**: 每个工具运行在独立的伪终端子进程中
5. **自动配置重写**: 启动时自动重写工具配置，确保格式变更后始终最新
6. **版本锁定**: 工具版本经 CI 兼容测试后更新，不自动升级

---

## 设计原则

- **单根配置目录**: ~/.huayu/ 包含所有配置和工具，可移植
- **零外部依赖**: 捆绑锁定版本二进制，不需要 Node.js/npm
- **一键安装**: PS1 脚本自动下载解压写 PATH，无需管理员权限
- **统一交互**: Codex 与 Claude 共用同一 TUI 界面和快捷键体系
- **启动自动配置**: 工具配置文件由 huayu 生成与管理，不从全局继承
- **bundled 优先**: 	ools/ 目录二进制优先于系统 PATH
- **Windows UTF-8**: 程序启动时设置控制台代码页为 65001
- **debug.log 完整审计**: 工具输出完整记录，不受 TUI 列宽限制
- **非破坏性退出**: Alt+Q 而非单独的 q 键退出
- **输入历史**: 仅记录 slash 命令 (不含自由文本)，最多 50 条

---

## 路线图与边界

### 当前版本 (v0.2.3) 范围外

- macOS / Linux 一键安装脚本 (手动下载 tar.gz)
- Web UI
- Gemini、OpenCode、Hermes 等其他 AI 编码工具
- 跨会话持久化聊天历史
- 多账户管理
- huayu 自身自动更新 (重新运行安装脚本更新)
- 鼠标拖拽分栏调整大小
- 完整终端快照测试 (ratatui 渲染验证)
- 管理用户系统级 Node.js 或 npm 安装
- 修改 baizor.com 服务端 API 行为

### 版本矩阵

| 组件 | 版本 | 更新策略 |
|------|------|----------|
| huayu | 0.2.3 | 发布新 Release |
| Codex | 0.142.5 | CI 兼容测试后更新 CODEX_VERSION |
| Claude Code | 1.0.3 | CI 兼容测试后更新 CLAUDE_VERSION |

---

## 源码文件清单

`
huayu/
├── src/
│   ├── main.rs                  # 入口: CLI 解析, UTF-8 初始化
│   ├── config.rs                # 配置中心 (503 行)
│   ├── command.rs               # 斜杠命令解析器
│   ├── tool.rs                  # PTY 工具进程管理 (424 行)
│   ├── error.rs                 # 统一错误类型 (thiserror)
│   ├── cli/
│   │   ├── mod.rs               # clap 子命令定义
│   │   └── commands/
│   │       ├── mod.rs
│   │       ├── login.rs         # 浏览器登录 CLI
│   │       ├── status.rs        # 状态查看 CLI
│   │       └── update.rs        # 工具安装 CLI
│   ├── tui/
│   │   ├── mod.rs               # ratatui + crossterm 事件循环
│   │   ├── app.rs               # App 状态机 (1000+ 行)
│   │   ├── ui.rs                # 界面渲染
│   │   └── theme.rs             # 主题色彩
│   └── services/
│       ├── mod.rs
│       ├── installer.rs         # 工具下载与版本管理 (349 行)
│       ├── login.rs             # 浏览器登录轮询
│       └── model_fetch.rs       # 模型列表获取
├── Cargo.toml                   # 项目配置
├── Cargo.lock
├── versions.json                # 版本唯一来源
├── build.ps1 / build-linux.sh   # 构建脚本
├── package.ps1 / package-linux.sh # 打包脚本
├── deploy.ps1 / deploy.sh       # 部署脚本
├── huayu.ps1 / huayu.sh         # 安装脚本
├── PRD20260705.md               # 产品需求文档
└── doc/
    ├── readme.md                # 原始文档
    ├── readmev1.md              # v1 版文档
    ├── readmev2.md              # v2 版文档
    ├── readmev3.md              # v3 版文档
    ├── readmev4.md              # 本文档
    └── build-deploy.md          # 构建部署文档
`

---

*最后更新: 2026-07-07 · huayu v0.2.3 · 56 测试全通过 · 基于完整源码深度分析生成*
