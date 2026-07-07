# 华宇 huayu v2 — AI 编码工作站

> 华宇 (huayu) 是面向 [baizor.com](https://baizor.com) 的 AI 编码工作站，以 Rust TUI 终端应用形式将 **Codex** 与 **Claude Code** 整合为统一交互界面。

---

## 目录

1. [核心价值](#核心价值)
2. [快速安装](#快速安装)
3. [功能特性](#功能特性)
4. [架构设计](#架构设计)
5. [模块详解](#模块详解)
6. [数据流](#数据流)
7. [配置体系](#配置体系)
8. [Slash 命令体系](#slash-命令体系)
9. [快捷键](#快捷键)
10. [构建与部署](#构建与部署)
11. [测试策略](#测试策略)
12. [错误处理](#错误处理)
13. [设计约定](#设计约定)
14. [工作范围外](#工作范围外)

---

## 核心价值

| 痛点 | 解决方案 |
|------|---------|
| Codex / Claude 需分别通过 npm 安装，依赖 Node.js | 内嵌锁定版本二进制，零外部依赖 |
| 两个工具交互方式不同，学习成本高 | 统一 TUI 界面 + 一致快捷键体系 |
| API Key 配置繁琐 | 浏览器 OAuth 一键登录，自动生成所有配置文件 |
| Windows 终端中文乱码 | 启动时 `SetConsoleCP(65001)` + `SetConsoleOutputCP(65001)` |
| 用户安装门槛高 | `irm ... \| iex` 一行命令完成安装 |

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
6. 无需管理员权限

### macOS / Linux

从 [GitHub Releases](https://github.com/BaizorAI/huayu/releases) 下载对应平台的 `tar.gz`，解压并放入 PATH。

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
├── history.json               # 输入历史
└── debug.log                  # 完整工具输出日志
```

---

## 功能特性

### 统一 TUI 界面

```
+-- 华宇 huayu | codex [Tab切换] | huayu-v2 | ●连接中 -------------------+
|                                                                          |
+------------------------------------------+-------------------------------+
|  主工作区 (左 ~70%)                        |  帮助与参考 (右 ~30%)           |
|                                          |                                |
|  执行日志、AI 输出、文件事件               |  快捷键速查、最近命令            |
|                                          |  工具版本状态                   |
|                                          |                                |
+------------------------------------------+-------------------------------+
|  输入框: /help                                                            |
|  [Enter]发送  [Shift+Enter]换行  [Tab]切换工具  [Alt+Q]退出  /help 查看命令 |
+--------------------------------------------------------------------------+
```

### 双工具管理

- **Tab** 键在 Codex ↔ Claude 之间一键切换
- 底层通过 `portable-pty` 管理子进程，支持 PTY 交互（y/n 确认等）
- 工具版本锁定：Codex `0.142.5` / Claude `1.0.3`
- 本地 bundled 二进制优先于系统 PATH；兼容 npm node_modules 回退
- 独立配置目录注入：`CODEX_HOME` / `CLAUDE_CONFIG_DIR` 环境变量隔离

### 浏览器登录流程

```
用户运行 /login
  → 生成 UUID token
  → 启动本地 HTTP 回调服务器
  → 打开浏览器 → baizor.com/code/token?token=xxx
  → 后台 2s 间隔轮询 /api/cli/poll
  → 用户授权完成 → 服务端返回 API Key + 模型元数据
  → 自动写入 config.json / codex config.toml / claude settings.json
  → 同步模型信息（context_window / max_output_tokens）
```

### 设置弹窗

- 输入框为空时按 `s` 打开
- 修改默认模型 + Base URL，**Enter 保存即生效**
- Tab 切换字段，Esc 关闭

### 其他

- **任务计时**：状态栏实时显示当前任务执行时长
- **滚动**：PgUp/PgDn 或鼠标滚轮；上滚暂停自动跟底，下滚到底恢复
- **进度流式输出**：更新/下载通过 `__DONE__` 哨兵结束
- **启动检查**：自动检测工具可用性与版本，提示用户更新

---

## 架构设计

### 项目概览

| 属性 | 值 |
|------|-----|
| 语言与版本 | Rust 2021 edition |
| 界面类型 | TUI（ratatui 0.29 + crossterm 0.28） |
| 运行时 | tokio 1.x (multi-thread) |
| HTTP 客户端 | reqwest 0.12 (rustls-tls) |
| 序列化 | serde / serde_json 1.0 |
| PTY 管理 | portable-pty 0.8 |
| 版本 | 0.2.0 |
| 测试 | 56 个单元测试全部通过 |

### 完整依赖树

| crate | 版本 | 用途 |
|-------|------|------|
| `clap` | 4.5 (derive) | CLI 参数解析 |
| `ratatui` | 0.29 | TUI 渲染框架 |
| `crossterm` | 0.28 (event-stream) | 终端控制与键盘/鼠标事件 |
| `tokio` | 1 (multi-thread) | 异步运行时 |
| `reqwest` | 0.12 (rustls-tls) | HTTPS 请求 |
| `serde` / `serde_json` | 1.0 | JSON 序列化/反序列化 |
| `portable-pty` | 0.8 | 跨平台伪终端 |
| `colored` | 2.1 | CLI 模式彩色输出 |
| `dirs` | 5.0 | 用户目录获取 |
| `uuid` | 1 (v4) | 登录 token 生成 |
| `thiserror` | 1.0 | 错误类型派生 |
| `strip-ansi-escapes` | 0.2 | ANSI 序列剥离 |
| `which` | 6.0 | PATH 二进制查找 |
| `zip` / `flate2` / `tar` | 2 / 1 / 0.4 | 压缩包处理 |
| `tempfile` (dev) | 3 | 测试隔离临时目录 |

### 分层架构

```
src/
├── main.rs           # 入口：Windows UTF-8 初始化 → CLI 解析 → 路由
├── error.rs          # 统一错误类型 (AppError)
├── config.rs         # 配置加载/保存/Codex&Claude 配置文件生成
├── command.rs        # Slash 命令解析器 (AppCommand)
├── tool.rs           # 工具抽象层 (ToolType / ToolEvent / ToolProcess)
├── cli/
│   ├── mod.rs        # CLI 入口 (clap Parser/Subcommand)
│   └── commands/
│       ├── login.rs  # huayu login 子命令
│       ├── status.rs # huayu status 子命令
│       └── update.rs # huayu update 子命令
├── services/
│   ├── mod.rs
│   ├── login.rs      # 浏览器登录流程 (token 生成 / URL / 轮询)
│   ├── installer.rs  # 工具下载/安装/版本管理
│   └── model_fetch.rs# /v1/models API 获取模型列表
└── tui/
    ├── mod.rs        # TUI 事件循环 (run / run_loop / handle_key / handle_mouse)
    ├── app.rs        # App 状态机 (输入 / 命令 / 工具事件 / 登录 / 设置)
    ├── ui.rs         # ratatui 渲染 (状态栏 / 主面板 / 帮助面板 / 输入框 / 弹窗)
    └── theme.rs      # 颜色常量
```

### 模块职责

| 模块 | 职责 | 输入 | 输出 |
|------|------|------|------|
| `main.rs` | 入口、UTF-8 初始化、命令路由 | 命令行参数 | 退出码 |
| `config.rs` | 配置 CRUD + 工具配置文件生成 | `config.json` | `HuayuConfig` / `config.toml` / `settings.json` |
| `command.rs` | 斜杠命令解析（纯函数） | 用户输入字符串 | `Option<AppCommand>` |
| `tool.rs` | 工具进程管理 + 输出事件分类 | 用户 prompt | `ToolProcess` / `ToolEvent` 流 |
| `services/installer.rs` | 下载/解压/版本检测 | 工具名 | 进度 channel |
| `services/login.rs` | OAuth 轮询 + 数据解析 | base_url + token | `LoginOutcome` |
| `services/model_fetch.rs` | 模型列表获取 | base_url + api_key | `Vec<FetchedModel>` |
| `tui/mod.rs` | 事件循环 | crossterm 事件 | 渲染帧 |
| `tui/app.rs` | 状态机 | 键盘输入 / ToolEvent / login 结果 | 状态变更 |
| `tui/ui.rs` | 渲染 | App 状态 | ratatui 组件树 |
| `cli/` | 独立 CLI 子命令模式 | CLI 参数 | 控制台输出 |

---

## 数据流

### TUI 模式主循环

```
crossterm event (键盘/鼠标/tick)
        │
        ▼
  handle_key() ─── 更新 App 状态
        │
        ▼
  app.drain_tool_events()  ←── ToolProcess.rx (mpsc channel)
  app.drain_update()       ←── update_rx (mpsc channel, 下载进度)
  app.poll_login()         ←── login result_rx (mpsc channel)
        │
        ▼
  ui::render()             ←── ratatui draw
        │
        ▼
  循环 (100ms tick)
```

### 用户输入 → 工具执行

```
输入框 Enter
  → command::parse() 判别是否为 slash 命令
  → 是命令：App 状态机处理（/switch /model /login /update ...）
  → 否 prompt：tool::spawn() 启动 PTY 子进程
       → codex exec "prompt" 或 claude -p "prompt"
       → PTY stdout 逐行读取 → strip ANSI → parse_event()
       → ToolEvent 通过 mpsc channel 发回主线程
       → 主循环 drain → push_output() → 渲染
```

### 登录流程

```
/login 命令
  → app.open_login_overlay()
  → LoginService::generate_token()
  → spawn 异步任务：
       1. 打开浏览器 → login_url
       2. LoginService::poll_for_key() 轮询后端
       3. 返回 Ok(LoginOutcome) 或 Err
  → 结果通过 result_rx 回到主线程
  → app.finalize_login()：
       - 写入 HuayuConfig
       - 保存 config.json
       - 生成 codex config.toml + auth.json
       - 生成 claude settings.json + config.json
       - 更新 TUI 状态
```

---

## 配置体系

### 主配置 (`config.json`)

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
    "huayu-v2": { "context_window": 128000, "max_output_tokens": 16384 }
  }
}
```

### 工具配置自动生成

**Codex** (`config.toml`)：
- `model_provider = "custom"`
- `base_url = "{base_url}/v1"`
- `wire_api = "responses"`
- 根据 `model_info` 生成 `[model_info.<name>]` 段

**Claude** (`settings.json`)：
- `env.ANTHROPIC_AUTH_TOKEN`
- `env.ANTHROPIC_BASE_URL`
- `env.ANTHROPIC_MODEL`
- `bypassPermissionsModeAccepted = true`

### 配置隔离

| 工具 | 环境变量 | 默认路径 |
|------|---------|---------|
| Codex | `CODEX_HOME` | `~/.huayu/codex/` |
| Claude | `CLAUDE_CONFIG_DIR` | `~/.huayu/claude/` |
| 华宇 | `HUAYU_CONFIG_DIR` | `~/.huayu/` |

可通过环境变量覆盖实现测试/多实例隔离。

---

## Slash 命令体系

| 命令 | 说明 | 示例 |
|------|------|------|
| `/login` | 浏览器登录 baizor.com | `/login` |
| `/switch codex\|claude` | 切换当前工具 | `/switch claude` |
| `/model <name>` | 更改默认模型 | `/model gpt-5.5` |
| `/update [codex\|claude]` | 下载/更新工具（默认全部） | `/update codex` |
| `/install [codex\|claude]` | 同 `/update`，别名 | `/install` |
| `/status` | 显示配置与工具状态 | `/status` |
| `/clear` | 清空输出面板 | `/clear` |
| `/help` 或 `/?` | 显示帮助 | `/?` |
| `/quit` / `/exit` / `/q` | 退出程序 | `/q` |

---

## 快捷键

### 全局

| 快捷键 | 功能 |
|--------|------|
| `Alt+Q` | 退出程序 |
| `Tab` | 切换工具（Codex ↔ Claude） |
| `s` (输入为空) | 打开设置弹窗 |

### 输入

| 快捷键 | 功能 |
|--------|------|
| `Enter` | 发送输入 |
| `Shift+Enter` | 输入框换行 |
| `↑ / ↓` | 输入历史导航（最近 50 条） |
| `Backspace` | 删除字符 |

### 主面板

| 快捷键 | 功能 |
|--------|------|
| `Page Up` | 向上翻页（暂停自动滚动） |
| `Page Down` | 向下翻页（到底恢复自动滚动） |
| 鼠标滚轮 | 上下滚动 |
| `Space` (输入为空) | 切换自动滚动 |
| `Esc` | 取消当前任务 |

### 弹窗

| 快捷键 | 功能 |
|--------|------|
| `Esc` | 关闭弹窗 |
| `r` (登录弹窗) | 重试登录 |
| `Enter` (设置弹窗) | 保存设置 |
| `Tab` (设置弹窗) | 切换字段 |

---

## 构建与部署

### 本地构建

```bash
# Windows
cargo build --release

# Linux (交叉编译)
cargo build --release --target x86_64-unknown-linux-gnu
```

### 打包

```powershell
# Windows — 完整编译 + 打包 zip
.\package.ps1

# 跳过编译，复用现有二进制
.\package.ps1 -SkipBuild
```

```bash
# Linux — 打包 tar.gz
bash package-linux.sh
bash package-linux.sh --skip-build
```

### 一键部署

```bash
bash huayu-deploy.sh                # 完整：编译 Win+Linux → 打包 → scp 到服务器
bash huayu-deploy.sh --skip-win     # 仅 Linux
bash huayu-deploy.sh --skip-linux   # 仅 Windows
bash huayu-deploy.sh --skip-build   # 复用已有二进制，仅打包+部署
```

部署流程：
1. `cargo build --release` → Windows exe
2. `package.ps1 -SkipBuild` → Windows zip
3. robocopy 同步源码到 WSL
4. WSL `cargo build --release --target x86_64-unknown-linux-gnu`
5. WSL `package.sh` → Linux tar.gz
6. scp 产物到 baizor 服务器 `release/` 目录

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
- 文件写入 / 测试通过失败 / 普通行直通

#### 二进制管理 (`installer.rs`)
- bundled 优先于 PATH / npm node_modules 回退
- tools_dir 为空/不存在返回 `None`
- 版本匹配/过期/缺失 / Windows `.exe` 后缀处理

#### TUI 状态机 (`app.rs`)
- 输入历史导航（空历史 / 循环 / 到底恢复草稿）
- submit 行为（空输入 / 未登录 / /help / /clear）
- `apply_settings` 更新内存并关闭弹窗

#### 登录服务 (`services/login.rs`)
- token 格式（32 位 hex）
- token 唯一性
- URL 格式正确性

### 测试隔离 Seam

| seam | 用途 |
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

### 错误类型 (`AppError`)

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
- **debug.log** 记录完整工具输出，不受 TUI 列宽截断

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

---

*最后更新：2026-07-07 · 版本 0.2.0 · 由项目源码分析自动生成*
