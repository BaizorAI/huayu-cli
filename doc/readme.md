# 华宇 huayu

华宇 (huayu) 是面向 [baizor.com](https://baizor.com) 的 AI 编码工作站，以 Rust TUI 终端应用形式将 **Codex** 与 **Claude Code** 整合为统一交互界面。

## 项目概述

- **技术栈**: Rust 2021 / clap / ratatui + crossterm / tokio + reqwest / portable-pty
- **平台支持**: Windows / Linux / macOS
- **安装方式**: 一行命令一键安装，无需 Node.js、npm 或其他外部依赖
- **配置目录**: `~/.huayu/`（可通过环境变量 `HUAYU_CONFIG_DIR` 覆盖）
- **测试覆盖**: 56 个单元测试全部通过（配置序列化、命令解析、事件解析、二进制查找、输入历史导航、设置更新等）

## 核心功能

- **统一 TUI 界面** — 实时滚动显示 Codex / Claude 工具输出，分栏布局，状态栏与设置弹窗
- **双工具切换** — Tab 键在 Codex 与 Claude 之间一键切换，底层通过 PTY 子进程管理
- **浏览器登录** — `/login` 启动本地 HTTP 回调服务器完成 baizor.com OAuth 流程，自动写入 API Key
- **Slash 命令体系** — `/login` `/switch` `/model` `/update` `/status` `/help` `/clear` `/quit`
- **输入历史导航** — 上下箭头浏览最近 50 条命令历史（仅记录 slash 命令，不含自由文本 prompt）
- **任务计时** — 状态栏实时显示当前任务执行时长
- **配置热更新** — 设置弹窗修改即时生效（模型、推理深度、自动模式等）
- **离线工具版本锁定** — release 包内捆绑 `codex.exe` 和 `claude` 二进制，独立于系统 PATH

## 快速安装

### Windows
```powershell
irm https://baizor.com/install/huayu.ps1 | iex
```

### Linux / macOS
```bash
curl -fsSL https://baizor.com/install/huayu.sh | bash
```

安装完成后直接运行 `huayu` 进入 TUI 工作站。

## 使用指南

```bash
# 启动 TUI 工作站
huayu

# 浏览器登录 baizor.com（首次使用）
huayu login

# 查看配置与工具状态
huayu status

# 下载/更新 AI 工具
huayu update
huayu update codex
huayu update claude
```

## TUI 快捷键

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

## TUI 命令

| 命令 | 说明 |
|------|------|
| `/login` | 浏览器登录 |
| `/switch codex\|claude` | 切换当前工具 |
| `/model <name>` | 切换默认模型 |
| `/update [codex\|claude]` | 下载/更新工具（默认全部） |
| `/status` | 显示配置与工具状态 |
| `/clear` | 清空面板 |
| `/help` | 显示帮助 |
| `/quit` | 退出 |

## 配置说明

配置文件：`~/.huayu/config.json`

| 字段 | 说明 | 默认值 |
|------|------|--------|
| `api_key` | baizor.com API Key | — |
| `base_url` | API 地址 | `https://baizor.com` |
| `default_model` | 默认模型 | `huayu-v2` |
| `active_tool` | 活跃工具 (`codex` / `claude`) | `codex` |
| `codex_full_auto` | Codex 全自动模式 | `true` |
| `codex_reasoning_effort` | 推理深度 (`low` / `medium` / `high`) | `medium` |
| `claude_max_turns` | Claude 最大轮次（0 = 不限） | `0` |
| `claude_permission_mode` | Claude 权限模式 | `bypassPermissions` |
| `model_info` | 模型元数据（从服务端同步） | — |

环境变量 `HUAYU_CONFIG_DIR` 可覆盖配置目录路径（测试隔离 seam）。

## 目录结构（安装后）

```
~/.huayu/
├── config.json          # 主配置
├── debug.log            # 调试日志
├── bin/
│   └── huayu.exe        # 主程序
├── codex/
│   ├── config.toml      # Codex 配置（含 [model_info]）
│   └── auth.json        # Codex API Key
├── claude/
│   └── settings.json    # Claude 配置（模型、权限、认证）
└── tools/
    ├── codex.exe         # Codex 二进制
    ├── codex.version     # 当前版本标记
    ├── claude            # Claude 二进制
    └── claude.version    # 当前版本标记
```

## 源码架构

```
src/
├── main.rs              # 入口：CLI 解析 → TUI 或子命令
├── config.rs            # 配置加载/保存，Codex/Claude 配置生成
├── command.rs           # TUI 斜杠命令解析
├── tool.rs              # PTY 工具进程管理，事件解析
├── error.rs             # 统一错误类型
├── cli/
│   ├── mod.rs           # CLI 子命令定义（clap derive）
│   └── commands/
│       ├── login.rs     # 浏览器登录轮询
│       ├── status.rs    # 配置状态输出
│       └── update.rs    # 工具下载/更新
├── tui/
│   ├── mod.rs           # TUI 事件循环（ratatui + crossterm）
│   ├── app.rs           # App 状态机
│   ├── ui.rs            # 界面渲染
│   └── theme.rs         # 主题颜色定义
└── services/
    ├── mod.rs
    ├── installer.rs     # 工具下载与版本管理
    ├── login.rs         # 浏览器登录流程
    └── model_fetch.rs   # 模型列表获取
```

## 构建与打包

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

产物输出至 `release/`，运行 `bash deploy.sh` 发布到 baizor 服务器。

### 交叉部署
```bash
# 完整编译 + 部署 (Windows + Linux)
bash huayu-deploy.sh

# 跳过某步
bash huayu-deploy.sh --skip-win    # 只重新编译 Linux
bash huayu-deploy.sh --skip-linux  # 只重新编译 Windows
```

## 运行测试

```bash
cargo test
```

当前 56 个测试全部通过，覆盖：
- 配置默认值、读写 round-trip、文件隔离
- API Key 脱敏
- 全部 slash 命令解析
- PTY 事件解析（认证错误、网络错误、文件写入、测试通过/失败）
- 本地二进制查找与版本校验
- App 输入历史导航、提交行为、设置更新

## 设计约定

- **单根配置目录** `~/.huayu/`，通过 `HUAYU_CONFIG_DIR` 可覆盖
- **Codex 配置注入** 通过 `CODEX_HOME` 环境变量，Claude 通过 `CLAUDE_CONFIG_DIR`
- **工具版本锁定**：Codex 锁定 `0.142.5`（Responses API + bypass approvals），Claude 锁定最小支持版本
- **安装通过 channel 异步流式输出进度**，以 `__DONE__` 哨兵结束
- **最近命令仅记录 slash 命令**（不含自由文本 prompt），避免泄露用户业务内容
- **PS1 安装脚本无需管理员权限**：安装到 User Profile，PATH 写入 User 级注册表

## 工作范围之外

以下功能明确不在当前范围内：
- macOS / Linux 一键安装脚本（当前仅 Windows PS1；其他平台手动下载 tar.gz）
- Web UI
- Gemini、OpenCode、Hermes 等其他 AI 工具
- 跨会话持久化聊天历史
- 多账户管理
- huayu 自身自动更新（用户重新运行 PS1 安装脚本更新）
- ratatui 完整终端快照测试
- 管理用户系统级 Node.js 或 npm 安装
- 鼠标拖拽分栏调整大小
