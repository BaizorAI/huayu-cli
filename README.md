# 华宇 huayu v0.2.3 — AI 编码工作站

> 华宇 (huayu) 是面向 [baizor.com](https://baizor.com) 的 AI 编码工作站，将 **Codex** 和 **Claude Code** 统一封装为 Rust TUI 终端应用，零外部依赖，一行命令安装即可使用。

---

## 目录

1. [特性](#特性)
2. [快速开始](#快速开始)
3. [安装后目录结构](#安装后目录结构)
4. [CLI 命令](#cli-命令)
5. [TUI 操作指南](#tui-操作指南)
6. [TUI 斜杠命令](#tui-斜杠命令)
7. [配置体系](#配置体系)
8. [架构概览](#架构概览)
9. [源码速览](#源码速览)
10. [构建与打包](#构建与打包)
11. [测试](#测试)
12. [错误处理与安全](#错误处理与安全)
13. [设计原则](#设计原则)
14. [路线图与边界](#路线图与边界)

---

## 特性

| 维度 | 详情 |
|------|------|
| **语言** | Rust 2021 edition |
| **二进制大小** | ~2.1 MB (Windows x64 zip) |
| **集成工具** | Codex 0.142.5 + Claude Code 1.0.3 |
| **平台** | Windows x64, Linux x64 / aarch64, macOS x64 / aarch64 |
| **配置目录** | `~/.huayu/` (可通过 `HUAYU_CONFIG_DIR` 覆盖) |
| **测试** | 56 个单元测试全部通过 |

- 统一 TUI 界面，Tab 一键切换 Codex ↔ Claude
- PTY 伪终端子进程运行 AI 工具，实时输出流式渲染
- 67% / 33% 分栏布局：左主输出 + 右帮助面板
- 斜杠命令体系（/model、/switch、/update、/clear 等）
- 浏览器 OAuth 登录，自动拉取 API Key 和服务器端配置
- 一键安装脚本，无需 Node.js / npm / 管理员权限
- 隔离配置：Codex 和 Claude 的配置文件完全由 huayu 管理
- 版本锁定：工具版本经 CI 兼容性测试后更新
- 完整 debug.log 审计日志

---

## 快速开始

### Windows

```powershell
irm https://baizor.com/install/huayu.ps1 | iex
```

### Linux / macOS

```bash
curl -fsSL https://baizor.com/install/huayu.sh | bash
```

安装完成后在终端输入：

```bash
huayu login        # 浏览器登录 baizor.com，获取 API Key
huayu status       # 查看工具与配置状态
huayu update       # 下载/更新 codex + claude
huayu              # 启动 TUI 工作站
```

---

## 安装后目录结构

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

---

## CLI 命令

| 命令 | 说明 |
|------|------|
| `huayu` | 启动 TUI 工作站（默认） |
| `huayu login` | 打开浏览器完成 baizor.com 登录，自动保存 API Key |
| `huayu login --key sk-xxx` | 直接指定 API Key 登录（跳过浏览器） |
| `huayu status` | 查看当前配置、API Key 掩码、工具版本与可用状态 |
| `huayu update` | 下载/更新全部工具（codex + claude） |
| `huayu update codex` | 仅更新 Codex |
| `huayu update claude` | 仅更新 Claude |

---

## TUI 操作指南

### 界面布局

```
+-- 华宇 huayu │ codex │ [Tab切换] │ huayu-v2 │ ● 连接中 ---+
|                                                              |
|  [主输出面板 - 70%]              |  [帮助面板 - 30%]          |
|  AI 工具的流式输出               |  最近使用命令               |
|  支持滚动、自动跟底              |  快捷键提示                 |
|                                  |                           |
+----------------------------------+---------------------------+
|  [输入框 - Shift+Enter 换行]                               |
+------------------------------------------------------------+
|  [快捷键提示栏]                                             |
+------------------------------------------------------------+
```

### 快捷键

| 按键 | 功能 |
|------|------|
| `Enter` | 发送消息 / 确认 |
| `Esc` | 取消任务 / 关闭弹窗 |
| `Tab` | 切换工具（codex ↔ claude） |
| `↑` / `↓` | 输入历史导航（语义唯一） |
| `PgUp` / `PgDn` | 滚动输出面板（↑ 暂停自动滚动，↓ 到底恢复） |
| 滚轮 | 上/下滚动输出面板 |
| `Shift+Enter` | 输入框内换行 |
| `s` | 打开设置弹窗（输入框为空时） |
| `Alt+Q` | 退出程序 |

### 设置弹窗

在 TUI 中按 `s`（输入框为空时）打开设置弹窗，可修改：

- **默认模型**: 切换使用的 AI 模型（如 `huayu-v2`）
- **API 地址**: 更改 API 基础 URL

按 `Tab` 切换焦点字段，按 `Enter` 确认保存，按 `Esc` 取消。

---

## TUI 斜杠命令

在输入框中以 `/` 开头触发：

| 命令 | 说明 |
|------|------|
| `/login` | 浏览器登录 baizor.com |
| `/model <name>` | 切换模型（如 `/model huayu-v2`） |
| `/switch codex\|claude` | 切换活动工具 |
| `/update [codex\|claude]` | 下载/更新工具（默认全部） |
| `/install [codex\|claude]` | 同 /update，保留给肌肉记忆 |
| `/status` | 显示当前配置与工具状态 |
| `/clear` | 清空主输出面板 |
| `/help` 或 `/?` | 显示帮助命令列表 |
| `/quit` 或 `/exit` 或 `/q` | 退出程序 |

> 不以 `/` 开头的输入直接作为 prompt 发送给当前活动的 AI 工具执行。

---

## 配置体系

### 主配置文件 `~/.huayu/config.json`

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `api_key` | string | `""` | baizor.com API Key |
| `base_url` | string | `https://baizor.com` | API 基础地址 |
| `default_model` | string | `huayu-v2` | 默认模型 |
| `active_tool` | string | `codex` | 当前活动工具 |
| `codex_model` | string | `""` | Codex 专用模型（覆盖 default_model） |
| `codex_full_auto` | bool | `true` | Codex 全自动模式 |
| `codex_reasoning_effort` | string | `medium` | 推理深度 (low / medium / high) |
| `claude_model` | string | `""` | Claude 专用模型 |
| `claude_max_turns` | u32 | `0` | Claude 最大轮次（0 = 不限制） |
| `claude_permission_mode` | string | `default` | Claude 权限模式 |
| `model_info` | object | `{}` | 模型元数据（服务器端下发，含 context_window 和 max_output_tokens） |

### 环境变量

| 变量 | 说明 |
|------|------|
| `HUAYU_CONFIG_DIR` | 覆盖配置根目录路径（测试 seam） |
| `DEBUG` | `true` 或 `1` 启用调试模式（默认 debug build 启用） |

### 配置自动管理

- 启动时自动重写 Codex / Claude 配置文件，确保格式变更后始终最新。
- API Key 在输出时掩码显示：`sk-xxxx***xxxx`（前 4 + 后 4 字符）。
- 配置不跨会话污染：huayu 不使用全局 Codex / Claude 配置。

---

## 架构概览

```
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
```

### 运行时数据流

```
用户输入 → command::parse()
  ├─ 斜杠命令 → AppCommand（本地处理）
  └─ 自由文本 → tool::spawn() → PTY → codex / claude
                    ↓
                ToolEvent (Line / FileWritten / AuthError / NetworkError / ...)
                    ↓
                app.rs: push_output → main_lines → ui.rs 渲染
                    ↓
                debug.log（完整未截断输出）
```

---

## 源码速览

```
huayu/
├── src/
│   ├── main.rs                  # 入口：CLI 解析、Windows UTF-8 初始化
│   ├── config.rs                # 配置中心（加载/保存/工具配置生成，503 行）
│   ├── command.rs               # 斜杠命令解析器（/help /model /switch 等）
│   ├── tool.rs                  # PTY 工具进程封装 + 事件解析（424 行）
│   ├── error.rs                 # 统一错误类型（thiserror）
│   ├── cli/
│   │   ├── mod.rs               # clap 子命令定义（login / status / update）
│   │   └── commands/
│   │       ├── login.rs         # 浏览器登录 CLI
│   │       ├── status.rs        # 状态查看 CLI
│   │       └── update.rs        # 工具下载/更新 CLI
│   ├── tui/
│   │   ├── mod.rs               # ratatui + crossterm 事件循环
│   │   ├── app.rs               # App 状态机（1000+ 行）
│   │   ├── ui.rs                # 界面渲染
│   │   └── theme.rs             # 主题色彩
│   └── services/
│       ├── installer.rs         # 工具下载与版本管理（349 行）
│       ├── login.rs             # 浏览器登录 OAuth 轮询
│       └── model_fetch.rs       # 模型列表获取
├── Cargo.toml                   # 项目依赖配置
├── Cargo.lock
├── versions.json                # 版本号唯一来源（huayu / codex / claude）
├── build.ps1 / build-linux.sh   # 智能增量构建脚本
├── package.ps1 / package-linux.sh  # 打包脚本
├── deploy.ps1 / deploy.sh       # 部署发布脚本
├── huayu.ps1 / huayu.sh         # 一键安装脚本
├── PRD20260705.md               # 产品需求文档
└── doc/
    ├── readme.md                # v4 技术参考文档
    ├── readmev2.md              # 本文档
    └── build-deploy.md          # 构建部署指南
```

### 核心模块说明

**`main.rs`** — 程序入口
- clap derive 解析 CLI 参数：无子命令进入 TUI，有子命令分发到 login / status / update。
- Windows 平台启动时设置 `SetConsoleCP(65001)` + `SetConsoleOutputCP(65001)`，确保中文、边框符号、状态图标正确显示。

**`config.rs`** — 配置中心
- `HuayuConfig` 结构体：serde 序列化，含 API Key、模型、Codex/Claude 专属设置、模型元数据。
- `write_codex_config()` / `write_claude_config()`：根据 `HuayuConfig` 生成对应工具的配置文件。
- 配置目录默认为 `~/.huayu/`，通过 `HUAYU_CONFIG_DIR` 覆盖。
- `TempConfigGuard`：RAII 守卫，测试时设置临时配置目录。

**`tool.rs`** — PTY 工具进程
- `ToolType` 枚举：Codex / Claude，含 `binary_path()`（bundled 优先）、`is_available()`。
- `ToolProcess`：基于 portable-pty 的子进程管理，通过 `mpsc::channel` 发送 ToolEvent。
- `ToolEvent` 枚举：Line / FileWritten / TestPassed / TestFailed / AuthError / NetworkError / Done / Error。
- `parse_event()`：从原始输出行解析结构化事件。

**`tui/mod.rs`** — 事件循环
- `run()`：初始化 raw mode、AlternateScreen、MouseCapture，创建 ratatui Terminal。
- 100ms tick 循环：渲染 UI → 处理键盘/鼠标事件 → 排空工具事件和更新进度 → 轮询登录状态。

**`tui/app.rs`** — App 状态机
- `App` 结构体：包含配置、工具进程、输出行、输入框、历史记录、登录浮层、设置弹窗等所有状态。
- `submit()`：处理用户提交（斜杠命令 vs 自由文本 → spawn 工具进程）。
- `drain_tool_events()`：从 PTY channel 读取事件并推入 main_lines。

**`tui/ui.rs`** — 界面渲染
- 四行布局：状态栏 → 主面板（70/30 分栏）→ 输入框 → 快捷键提示。
- 弹窗叠加：登录浮层、设置弹窗。
- 中文主题颜色定义在 `theme.rs` 中。

**`services/installer.rs`** — 工具下载管理
- 版本锁定：CODEX_VERSION = `0.142.5`，CLAUDE_VERSION = `1.0.3`。
- `local_binary()`：优先查 huayu tools 目录，回退到 npm node_modules。
- `is_current_version()`：比对 `.version` 文件内容与锁定版本。
- `install_latest()`：异步下载 → 解压（zip/tar.gz）→ 写版本文件 → 通过 channel 报告进度。

**`services/login.rs`** — OAuth 登录轮询
- 打开浏览器到 baizor.com 登录页 → 启动轮询线程，每 2 秒查询一次，最长 5 分钟。
- 成功回调返回 `LoginOutcome`：含 API Key、模型偏好、Codex/Claude 设置、model_info 元数据。

---

## 构建与打包

### 构建

```powershell
# Windows — 智能增量构建（仅编译有变更的组件）
.\build.ps1

# 强制全量构建
.\build.ps1 -Force

# 仅构建指定组件
.\build.ps1 -Component huayu
```

```bash
# Linux
bash build-linux.sh
```

构建脚本自动从 `versions.json` 读取版本号，并同步到 `Cargo.toml`。

### 打包

```powershell
# Windows — 完整编译 + 打包 zip
.\package.ps1

# 跳过编译，复用已有二进制
.\package.ps1 -SkipBuild
```

```bash
# Linux — 编译 + 打包 tar.gz
bash package-linux.sh
bash package-linux.sh --skip-build
```

产物输出到 `release/` 目录。

### 部署

```powershell
.\deploy.ps1      # Windows
bash deploy.sh    # Linux
```

将 `release/` 产物同步到 baizor.com 服务器。

---

## 测试

```bash
cargo test
```

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

---

## 错误处理与安全

### 错误类型 (`src/error.rs`)

| 变体 | 触发场景 | 用户提示 |
|------|---------|---------|
| `Io` | 文件读写失败 | IO error |
| `Config` | 配置语法错误 | Config error |
| `Network` | 网络请求失败 | Network error |
| `Auth` | 认证失败 | please run `huayu login` |
| `ToolNotFound` | 工具二进制缺失 | not installed or not in PATH |
| `Json` | JSON 解析错误 | JSON error |
| `Message` | 通用错误 | 自定义消息 |

### 安全设计

1. **API Key 掩码**: 输出时仅显示 `sk-xxxx***xxxx`（前 4 + 后 4 字符）。
2. **调试模式**: 通过 `DEBUG` 环境变量控制，默认仅 debug build 启用。
3. **配置隔离**: Codex / Claude 配置文件完全由 huayu 管理，不读取全局配置。
4. **PTY 隔离**: 每个工具运行在独立的伪终端子进程中。
5. **启动自动重写**: 每次启动自动重写工具配置，确保格式变更后始终正确。
6. **版本锁定**: 工具版本经 CI 兼容测试后才更新，不自动升级。
7. **输入历史隔离**: 仅记录 slash 命令（不含自由文本 prompt），最多 50 条。

---

## 设计原则

| 原则 | 说明 |
|------|------|
| **单根配置目录** | `~/.huayu/` 包含所有配置和工具，可整体迁移 |
| **零外部依赖** | 捆绑锁定版本二进制，不需要 Node.js / npm |
| **一键安装** | PS1/SH 脚本自动下载解压写 PATH，无需管理员权限 |
| **统一交互** | Codex 与 Claude 共用同一 TUI 界面和快捷键体系 |
| **启动自动配置** | 工具配置文件由 huayu 生成与管理，不从全局继承 |
| **bundled 优先** | `tools/` 目录二进制优先于系统 PATH |
| **Windows UTF-8** | 程序启动时设置控制台代码页为 65001 |
| **debug.log 完整审计** | 工具输出完整记录，不受 TUI 列宽限制 |
| **非破坏性退出** | `Alt+Q` 而非单独 `q` 键退出 |
| **输入历史隐私** | 仅记录 slash 命令，最多 50 条 |

---

## 路线图与边界

### 当前版本 (v0.2.3) 范围外

- macOS / Linux 一键安装脚本（当前手动下载 tar.gz）
- Web UI
- Gemini、OpenCode、Hermes 等其他 AI 编码工具
- 跨会话持久化聊天历史
- 多账户管理
- huayu 自身自动更新（重新运行安装脚本更新）
- 鼠标拖拽分栏调整大小
- 完整终端快照测试（ratatui 像素级验证）
- 管理用户系统级 Node.js 或 npm 安装
- 修改 baizor.com 服务端 API 行为

### 版本矩阵

| 组件 | 版本 | 更新策略 |
|------|------|----------|
| huayu | 0.2.3 | 发布新 Release |
| Codex | 0.142.5 | CI 兼容测试后更新 CODEX_VERSION |
| Claude Code | 1.0.3 | CI 兼容测试后更新 CLAUDE_VERSION |

---

*最后更新: 2026-07-07 · huayu v0.2.3 · 56 测试全通过 · 基于完整源码分析生成*
