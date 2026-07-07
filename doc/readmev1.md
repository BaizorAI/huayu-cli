# 华宇 huayu v1 — AI 编码工作站

华宇 (huayu) 是面向 [baizor.com](https://baizor.com) 的 AI 编码工作站，以 Rust TUI 终端应用形式将 **Codex** 与 **Claude Code** 整合为统一交互界面。

---

## 目录

- [项目概览](#项目概览)
- [技术栈](#技术栈)
- [核心功能](#核心功能)
- [架构设计](#架构设计)
- [快速开始](#快速开始)
- [使用指南](#使用指南)
- [配置说明](#配置说明)
- [目录结构](#目录结构)
- [源码架构](#源码架构)
- [构建与部署](#构建与部署)
- [测试](#测试)
- [设计约定](#设计约定)
- [错误处理](#错误处理)
- [工作范围外](#工作范围外)

---

## 项目概览

| 属性 | 值 |
|------|-----|
| 项目名称 | 华宇 (huayu) |
| 当前版本 | `0.2.0` |
| 编程语言 | Rust 2021 edition |
| 界面类型 | TUI（终端用户界面） |
| 支持平台 | Windows / Linux / macOS |
| 测试覆盖 | 56 个单元测试全部通过 |
| 安装方式 | 一行命令一键安装，无需 Node.js / npm |

华宇将 Codex 和 Claude Code 两个主流 AI 编码工具封装在统一 TUI 内，通过 PTY 子进程管理底层工具，提供：

- 统一的交互界面与快捷键体系
- 浏览器 OAuth 登录，自动配置 API Key 与工具参数
- 离线工具版本锁定，零外部依赖
- 分栏布局：左侧主输出面板 + 右侧帮助面板 + 底部输入框 + 状态栏
- 设置弹窗热更新（模型、推理深度、自动模式等）

---

## 技术栈

### 运行时依赖 (Cargo.toml)

| crate | 版本 | 用途 |
|-------|------|------|
| `clap` | 4.5 (derive) | CLI 参数解析 |
| `ratatui` | 0.29 | TUI 渲染框架 |
| `crossterm` | 0.28 (event-stream) | 终端控制与事件 |
| `tokio` | 1 (multi-thread) | 异步运行时 |
| `reqwest` | 0.12 (rustls-tls) | HTTP 客户端 |
| `serde` / `serde_json` | 1.0 | 序列化 |
| `portable-pty` | 0.8 | 伪终端子进程管理 |
| `colored` | 2.1 | 终端彩色输出 |
| `dirs` | 5.0 | 系统目录获取 |
| `uuid` | 1 (v4) | 唯一标识生成 |
| `thiserror` | 1.0 | 错误类型派生 |
| `strip-ansi-escapes` | 0.2 | ANSI 转义序列剥离 |
| `which` | 6.0 | 系统 PATH 二进制查找 |
| `zip` / `flate2` / `tar` | — | 压缩包处理 |

### 开发依赖

| crate | 用途 |
|-------|------|
| `tempfile` | 测试隔离的临时目录 |

---

## 核心功能

### 统一 TUI 界面

- **分栏布局**：左侧主输出面板（~70%）+ 右侧帮助面板（~30%）
- **状态栏**：显示当前工具（codex/claude）、模型名、连接状态、任务计时
- **底部输入框**：支持自由文本 prompt 与 slash 命令
- **快捷键提示栏**：底部显示常用快捷键
- **弹窗**：登录弹窗、设置弹窗（居中覆盖主界面）

### 双工具管理

- Tab 键在 Codex 与 Claude 之间一键切换
- 底层通过 `portable-pty` 管理工具子进程
- 工具版本锁定：Codex `0.142.5`，Claude `1.0.3`
- 本地 bunded 二进制优先于系统 PATH
- 独立的配置目录注入（`CODEX_HOME`、`CLAUDE_CONFIG_DIR`）

### 浏览器登录

- `/login` 命令启动本地 HTTP 回调服务器
- 打开浏览器完成 baizor.com OAuth 流程
- 自动写入 API Key 到 `config.json`
- 自动生成 Codex `config.toml` + `auth.json` 和 Claude `settings.json`
- 从服务端同步模型元数据（context_window / max_output_tokens）

### Slash 命令体系

| 命令 | 说明 |
|------|------|
| `/login` | 浏览器登录 baizor.com |
| `/switch codex\|claude` | 切换当前工具 |
| `/model <name>` | 切换默认模型 |
| `/update [codex\|claude]` | 下载/更新工具（默认全部） |
| `/install [codex\|claude]` | 同 `/update`，别名 |
| `/status` | 显示配置与工具状态 |
| `/clear` | 清空输出面板 |
| `/help` 或 `/?` | 显示帮助 |
| `/quit` / `/exit` / `/q` | 退出程序 |

### 输入历史

- 上下箭头浏览最近 50 条命令历史
- 仅记录 slash 命令，不含自由文本 prompt（避免泄露业务内容）
- 浏览历史时保存当前草稿，到底后恢复

### 其他

- **任务计时**：状态栏实时显示当前任务执行时长
- **滚动**：PgUp/PgDn 或鼠标滚轮；上滚暂停自动跟底，下滚到底恢复
- **设置弹窗**：修改模型、推理深度、自动模式，即时生效
- **进度 channel**：更新/下载通过 `__DONE__` 哨兵流式输出进度

---

## 架构设计

### 分层架构

```
CLI 入口 (main.rs)
  ├── TUI 模式 (tui/)
  │   ├── 事件循环 (mod.rs) — crossterm 事件 → App 状态机
  │   ├── 状态机 (app.rs)   — 输入/命令/Tool 事件/登录 处理
  │   ├── 界面渲染 (ui.rs)   — ratatui 布局与组件
  │   └── 主题 (theme.rs)   — 颜色常量
  ├── CLI 子命令 (cli/)
  │   ├── login  — 浏览器登录 + 轮询
  │   ├── status — 配置与工具状态输出
  │   └── update — 工具下载
  ├── 配置层 (config.rs)  — 加载/保存/Codex/Claude 配置生成
  ├── 命令解析 (command.rs) — slash 命令解析
  ├── 工具管理 (tool.rs)  — PTY 进程 + 事件解析
  └── 服务层 (services/)
      ├── login.rs        — OAuth 登录流程
      ├── installer.rs    — 工具下载与版本管理
      └── model_fetch.rs  — 模型列表获取
```

### 数据流

1. 用户在输入框输入文本/命令 → `App` 状态机处理
2. slash 命令 → `command::parse()` → `AppCommand` → 本地动作或 tool 交互
3. 自由文本 → 通过 `ToolProcess` PTY 发送给活跃工具（codex/claude）
4. PTY 子进程输出 → `parse_event()` → `ToolEvent` → 更新 `main_lines` 与状态
5. 每 100ms tick → 消费 tool channel + login channel + update channel → 渲染

### 关键设计决策

- **PTY 子进程 vs API 调用**：选择 PTY 管理原生工具二进制，保留工具完整功能
- **配置隔离**：每个工具通过环境变量注入独立配置目录，避免与系统安装冲突
- **bundled 二进制优先**：`tools/` 目录下的本地二进制优先于 PATH，实现版本锁定
- **事件分类解析**：PTY 输出通过关键词匹配分类为 AuthError / NetworkError / FileWritten / TestPassed 等

---

## 快速开始

### 一键安装

**Windows**
```powershell
irm https://baizor.com/install/huayu.ps1 | iex
```

**Linux / macOS**
```bash
curl -fsSL https://baizor.com/install/huayu.sh | bash
```

安装后 `huayu` 自动加入 PATH，直接运行进入 TUI 工作站。

### 首次使用

```bash
huayu login    # 浏览器登录，获取 API Key
huayu update   # 下载 Codex + Claude 工具
huayu          # 启动 TUI 工作站
```

---

## 使用指南

### CLI 子命令

```bash
huayu               # 启动 TUI 工作站（默认）
huayu login         # 浏览器登录
huayu status        # 查看配置与工具状态
huayu update         # 更新全部工具
huayu update codex   # 仅更新 Codex
huayu update claude  # 仅更新 Claude
```

### TUI 快捷键

| 按键 | 功能 |
|------|------|
| `Enter` | 发送消息 / 确认 |
| `Esc` | 取消任务 / 关闭弹窗 |
| `Tab` | 切换 Codex ↔ Claude |
| `↑ / ↓` | 浏览输入历史 |
| `PgUp / PgDn` | 滚动输出面板 |
| 滚轮 | 上下滚动 |
| `s` | 打开设置弹窗（输入框为空时） |
| `Alt+Q` | 退出程序 |
| `Shift+Enter` | 输入框换行 |

---

## 配置说明

### 主配置文件 `~/.huayu/config.json`

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| `api_key` | string | baizor.com API Key | — |
| `base_url` | string | API 基础地址 | `https://baizor.com` |
| `default_model` | string | 默认模型 | `huayu-v2` |
| `active_tool` | string | 活跃工具 | `codex` |
| `codex_model` | string | Codex 专用模型（覆盖 default_model） | — |
| `codex_full_auto` | bool | Codex 全自动模式 | `true` |
| `codex_reasoning_effort` | string | 推理深度 | `medium` |
| `claude_model` | string | Claude 专用模型 | — |
| `claude_max_turns` | u32 | 最大轮次（0 = 不限） | `0` |
| `claude_permission_mode` | string | 权限模式 | `bypassPermissions` |
| `model_info` | map | 模型元数据（服务端同步） | — |

### 环境变量

| 变量 | 用途 |
|------|------|
| `HUAYU_CONFIG_DIR` | 覆盖配置根目录（测试隔离） |
| `CODEX_HOME` | 注入 Codex 配置目录（自动设为 `~/.huayu/codex/`） |
| `CLAUDE_CONFIG_DIR` | 注入 Claude 配置目录（自动设为 `~/.huayu/claude/`） |

### 工具版本锁定

| 工具 | 锁定版本 | 关键特性要求 |
|------|----------|-------------|
| Codex | `0.142.5` | Responses API + `--dangerously-bypass-approvals-and-sandbox` |
| Claude | `1.0.3` | `--print` + `--dangerously-skip-permissions` 可用 |

版本号硬编码在 `src/services/installer.rs`，仅在 CI 兼容测试通过后更新。

---

## 目录结构

### 安装后 (`~/.huayu/`)

```
~/.huayu/
├── config.json          # 主配置
├── debug.log            # 调试日志
├── history.json         # 输入历史（最多 50 条 slash 命令）
├── bin/
│   └── huayu.exe        # 主程序
├── codex/
│   ├── config.toml      # Codex 配置（含 [model_info]）
│   └── auth.json        # Codex API Key
├── claude/
│   ├── settings.json    # Claude 环境变量配置
│   └── config.json      # Claude CLI 配置（权限、引导状态）
└── tools/
    ├── codex.exe         # Codex 二进制
    ├── codex.version     # 当前版本标记
    ├── claude            # Claude 二进制
    └── claude.version    # 当前版本标记
```

### 发布包内容 (`release/`)

```
release/
├── huayu-x86_64-pc-windows-msvc.zip
│   ├── huayu.exe
│   └── tools/
│       ├── codex.exe / codex.version
│       └── claude / claude.version
├── huayu-0.2.0-x86_64-unknown-linux-gnu.tar.gz
├── codex-0.142.5-x86_64-pc-windows-msvc.zip
├── claude-1.0.3-x86_64-pc-windows-msvc.zip
├── huayu-version.txt
├── codex-version.txt
└── claude-version.txt
```

---

## 源码架构

```
src/
├── main.rs              # 入口：Windows UTF-8 初始化 → CLI 解析 → TUI 或子命令
├── config.rs            # HuayuConfig 定义、加载/保存、Codex/Claude 配置生成
├── command.rs           # slash 命令解析（AppCommand / UpdateTarget）
├── tool.rs              # ToolType / ToolEvent / ToolProcess（PTY 子进程管理）
├── error.rs             # AppError 统一错误类型（thiserror）
├── cli/
│   ├── mod.rs           # Cli / Commands 定义（clap derive）
│   └── commands/
│       ├── mod.rs
│       ├── login.rs     # 浏览器登录子命令
│       ├── status.rs    # 配置状态子命令
│       └── update.rs    # 工具更新子命令
├── tui/
│   ├── mod.rs           # TUI 事件循环（100ms tick）
│   ├── app.rs           # App 状态机（~33KB，核心逻辑）
│   ├── ui.rs            # ratatui 界面渲染（状态栏/主面板/输入框/弹窗）
│   └── theme.rs         # 主题颜色常量
└── services/
    ├── mod.rs
    ├── installer.rs     # 工具下载、版本管理、local_binary 查找
    ├── login.rs         # 浏览器登录流程、OAuth 轮询、LoginOutcome
    └── model_fetch.rs   # /v1/models 模型列表获取
```

### 模块依赖关系

```
main ──┬── cli（子命令模式）
       └── tui ──┬── app ──┬── config（配置读写）
                 │         ├── command（命令解析）
                 │         ├── tool ──┬── portable-pty（子进程）
                 │         │          └── services/installer（二进制查找）
                 │         └── services ──┬── login（OAuth）
                 │                        ├── installer（下载）
                 │                        └── model_fetch（模型列表）
                 └── ui ──── theme
```

---

## 构建与部署

### Windows

```powershell
# 完整构建 + 打包
.\package.ps1

# 跳过编译，复用现有二进制
.\package.ps1 -SkipBuild
```

### Linux

```bash
bash package-linux.sh
bash package-linux.sh --skip-build
```

### 交叉部署

```bash
# 完整编译 + 部署 (Windows + Linux)
bash huayu-deploy.sh

# 跳过某步
bash huayu-deploy.sh --skip-win     # 仅重新编译 Linux
bash huayu-deploy.sh --skip-linux   # 仅重新编译 Windows
bash huayu-deploy.sh --skip-build   # 仅打包 + 部署（复用已有二进制）
```

部署流程：
1. `cargo build --release` — Windows exe
2. `package.ps1 -SkipBuild` — 打包 Windows zip
3. robocopy 同步源码到 WSL
4. WSL `cargo build --release --target x86_64-unknown-linux-gnu`
5. WSL `package.sh` — 打包 Linux tar.gz
6. scp 产物到 baizor 服务器

产物输出至 `release/`，运行 `bash deploy.sh` 发布到 baizor 服务器。

---

## 测试

### 运行

```bash
cargo test
```

当前 **56 个测试全部通过**。

### 测试覆盖

#### 配置层 (config.rs)
- 默认值验证
- 序列化/反序列化 round-trip
- Codex `config.toml` + `auth.json` 生成正确性
- Claude `settings.json` + `config.json` 生成正确性
- Codex/Claude 专用模型覆盖逻辑
- 文件隔离（所有配置文件位于 `HUAYU_CONFIG_DIR` 内）
- API Key 脱敏输出

#### 命令解析 (command.rs)
- 全部 slash 命令：`/login` `/switch` `/model` `/update` `/install` `/status` `/help` `/?` `/clear` `/quit` `/exit` `/q`
- 未知命令返回 `Unknown`
- 非命令输入返回 `None`
- `UpdateTarget::tool_names()` 正确性

#### 事件解析 (tool.rs)
- 认证错误：401 Unauthorized / auth failed / invalid api key
- 网络错误：connection refused
- 文件写入检测
- 测试通过/失败检测
- 普通行直通

#### 二进制查找 (installer.rs)
- bundled 优先于 PATH
- tools_dir 为空/不存在返回 None
- 版本匹配/过期/缺失判断
- Windows `.exe` 后缀处理

#### TUI 状态机 (app.rs)
- 输入历史导航（空历史、循环、到底恢复草稿）
- submit 行为（空输入、未登录、/help、/clear）
- `apply_settings` 更新内存并关闭弹窗

### 测试隔离 (seam)

| seam | 用途 |
|------|------|
| `HUAYU_CONFIG_DIR` | 覆盖配置目录路径 |
| `CONFIG_LOCK` + `TempConfigGuard` | 并发测试互斥 |
| Command parser | 纯函数，无副作用 |
| Tool event parser | 纯函数，无副作用 |
| App 状态方法 | 不渲染终端即可验证行为 |

### 已知未覆盖项

- PS1 安装脚本（独立 Pester 测试，不在 Rust 测试中）
- login polling（需 mocked HTTP）
- model fetch（需 mocked API）
- 下载进度 channel（需 mocked HTTP）

---

## 错误处理

### 错误类型 (AppError)

| 变体 | 描述 | 用户操作建议 |
|------|------|-------------|
| `Io` | 文件 IO 错误 | 检查磁盘权限与路径 |
| `Config` | 配置格式错误 | 检查 `config.json` 语法 |
| `Network` | 网络请求失败 | 检查网络连接与 baizor.com 可达性 |
| `Auth` | 认证失败 | 运行 `huayu login` 重新认证 |
| `ToolNotFound` | 工具未安装或不在 PATH | 运行 `huayu update` |
| `Json` | JSON 解析错误 | 检查对应 JSON 文件格式 |
| `Message` | 通用错误 | 查看错误消息详情 |

### 连接状态 (ConnectionStatus)

TUI 状态栏实时显示：

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
- **Codex 配置注入** 通过 `CODEX_HOME` 环境变量；Claude 通过 `CLAUDE_CONFIG_DIR`
- **工具版本锁定**：Codex `0.142.5`，Claude `1.0.3`，版本号在 CI 兼容测试后更新
- **安装 channel 异步流式输出**，以 `__DONE__` 哨兵结束
- **最近命令仅记录 slash 命令**（不含自由文本 prompt）
- **PS1 安装脚本无需管理员权限**：安装到 User Profile，PATH 写入 User 级注册表
- **Windows UTF-8 初始化**：启动时 `SetConsoleCP(65001)` + `SetConsoleOutputCP(65001)` 避免乱码
- **bundled 二进制优先**：`tools/` 目录优先于 PATH；也支持 npm node_modules 兼容回退

---

## 工作范围外

以下功能明确不在当前范围内：

- macOS / Linux 一键安装脚本（仅 Windows PS1；其他平台手动下载 tar.gz）
- Web UI
- Gemini、OpenCode、Hermes 等其他 AI 工具
- 跨会话持久化聊天历史
- 多账户管理
- huayu 自身自动更新（运行安装脚本重新安装）
- ratatui 完整终端快照测试
- 管理用户系统级 Node.js 或 npm 安装
- 鼠标拖拽分栏调整大小

---

*最后更新：2026-07-07 · 版本 0.2.0*
