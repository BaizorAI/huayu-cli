# 华宇 huayu

华宇 (huayu) 是面向 baizor.com 的 AI 编程工作台，提供统一 TUI 界面，将 codex 与 claude 两个 AI 工具整合到一个交互式终端应用中。

## 功能概述

- 统一 TUI 界面，实时显示 AI 工具输出
- 支持 codex 和 claude 两种工具，Tab 键一键切换
- 会话历史记录（最近 50 条）
- 任务计时与状态展示
- 浏览器登录流程，自动保存 API Key
- Windows / Linux / macOS 全平台支持

## 安装

### Windows

```powershell
irm https://baizor.com/install/huayu.ps1 | iex
```

### Linux / macOS

```bash
curl -fsSL https://baizor.com/install/huayu.sh | bash
```

安装后 huayu 二进制位于 `~/.huayu/bin/`，自动加入 PATH。

## 使用

```bash
# 启动 TUI 工作台
huayu

# 浏览器登录（首次使用）
huayu login

# 查看配置与工具状态
huayu status

# 下载/更新 AI 工具
huayu update
huayu update codex
huayu update claude
```

## 配置

配置文件：`~/.huayu/config.json`

| 字段 | 说明 | 默认值 |
|------|------|--------|
| `api_key` | baizor.com API Key | — |
| `base_url` | API 地址 | `https://baizor.com` |
| `default_model` | 默认模型 | `huayu-v2` |
| `active_tool` | 活跃工具 (`codex`/`claude`) | `codex` |
| `codex_full_auto` | codex 全自动模式 | `true` |
| `codex_reasoning_effort` | 推理深度 (`low`/`medium`/`high`) | `medium` |
| `claude_max_turns` | claude 最大轮次（0 = 不限） | `0` |

环境变量 `HUAYU_CONFIG_DIR` 可覆盖配置目录路径。

## TUI 快捷键

| 按键 | 功能 |
|------|------|
| `Enter` | 发送消息 / 确认 |
| `Esc` | 取消任务 / 关闭弹窗 |
| `Tab` | 切换 codex ↔ claude |
| `↑ / ↓` | 浏览输入历史 |
| `PgUp / PgDn` | 滚动输出面板 |
| 滚轮 | 上下滚动 |
| `s` | 打开设置（输入框为空时） |
| `Alt+Q` | 退出 |
| `/help` | 查看命令列表 |

## TUI 命令

| 命令 | 说明 |
|------|------|
| `/login` | 浏览器登录 |
| `/model <name>` | 切换模型 |
| `/switch codex\|claude` | 切换工具 |
| `/update [codex\|claude]` | 更新工具 |
| `/status` | 查看配置状态 |
| `/clear` | 清空输出面板 |
| `/help` | 显示帮助 |
| `/quit` | 退出 |

## 工具目录结构

```
~/.huayu/
├── config.json          # 主配置
├── debug.log            # 调试日志
├── bin/
│   └── huayu.exe        # 主程序
├── codex/
│   ├── config.toml      # codex 配置（含模型与 model_info）
│   └── auth.json        # codex API Key
├── claude/
│   └── settings.json    # claude 配置（模型、权限、认证）
└── tools/
    ├── codex.exe         # codex 二进制
    ├── codex.version     # 当前版本标记
    ├── claude            # claude 二进制
    └── claude.version    # 当前版本标记
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

## 架构说明

```
src/
├── main.rs              # 入口：CLI 解析 → TUI 或子命令
├── config.rs            # 配置加载/保存，codex/claude 配置生成
├── tool.rs              # PTY 工具进程管理，事件解析
├── error.rs             # 统一错误类型
├── command.rs           # TUI 斜杠命令解析
├── cli/                 # CLI 子命令（login / status / update）
├── tui/                 # TUI 渲染与事件循环（ratatui）
└── services/
    ├── installer.rs     # 工具下载与版本管理
    └── login.rs         # 浏览器登录轮询
```
