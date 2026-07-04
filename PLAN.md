# huazhen — 实施计划

## 项目概述

**huazhen**（华珍）是一个 Rust TUI 命令行工具，用于：

1. 登录 [baizor.com](https://baizor.com) 获取 API Key
2. 列出并选择可用模型
3. 自动安装并配置 **Claude Code** 和 **Codex** CLI 工具，接入 baizor.com 的代理端点

参考项目：`../baizor-cli`（BaizorAi CLI，同为 Rust + ratatui）

---

## 技术栈

| 层次 | 选型 |
|------|------|
| 语言 | Rust 2021 edition |
| CLI 解析 | `clap` v4 (derive) |
| TUI 渲染 | `ratatui` + `crossterm` |
| 异步运行时 | `tokio` (current_thread) |
| HTTP 客户端 | `reqwest` (rustls-tls) |
| 序列化 | `serde` + `serde_json` + `toml` |
| 颜色输出 | `colored` |
| 配置存储 | `~/.huazhen/config.json` |

---

## 目录结构

```
huazhen/
├── Cargo.toml
├── PLAN.md                  ← 本文件
├── src/
│   ├── main.rs              ← 入口：解析 CLI，分发命令或启动 TUI
│   ├── error.rs             ← AppError 类型
│   ├── config.rs            ← 配置读写（~/.huazhen/config.json）
│   ├── cli/
│   │   ├── mod.rs           ← Clap 顶层命令定义
│   │   └── commands/
│   │       ├── login.rs     ← `huazhen login` — 浏览器登录流程
│   │       ├── models.rs    ← `huazhen models` — 列出模型
│   │       ├── install.rs   ← `huazhen install` — 安装/配置工具
│   │       └── status.rs    ← `huazhen status` — 显示当前配置
│   ├── services/
│   │   ├── login.rs         ← LoginService：token 生成 + 轮询
│   │   ├── model_fetch.rs   ← 从 baizor.com/v1/models 获取模型列表
│   │   └── installer.rs     ← 检测/安装 codex、claude CLI 并写入配置
│   └── tui/
│       ├── mod.rs           ← TUI 入口（ratatui 事件循环）
│       ├── app.rs           ← App 状态机
│       ├── ui.rs            ← 渲染逻辑
│       └── theme.rs         ← 颜色主题
```

---

## 核心流程

### 1. 登录流程（复用 baizor-cli 逻辑）

```
huazhen login
  │
  ├─ 生成随机 token（UUID v4 无连字符）
  ├─ 打印：https://baizor.com/code/token?token=<TOKEN>
  ├─ 轮询：GET https://baizor.com/api/cli/poll?token=<TOKEN>
  │     每 2 秒一次，超时 5 分钟
  │     响应：{ success, data: { status, key, default_model, ... } }
  ├─ 收到 key → 写入 ~/.huazhen/config.json
  └─ 打印成功提示（遮掩 key 中间部分）
```

### 2. 模型选择

```
huazhen models
  │
  ├─ 读取 ~/.huazhen/config.json 中的 api_key
  ├─ GET https://baizor.com/v1/models （Authorization: Bearer <key>）
  ├─ 解析并展示模型列表（id, owned_by）
  └─ [TUI 模式] 方向键选择，回车确认 → 更新默认模型到 config
```

### 3. 安装 & 配置

默认行为：安装并配置 **codex**。`--app claude` 可额外配置 Claude Code。

```
huazhen install [--app codex|claude|all]   # 默认 --app codex
  │
  ├─ [codex] 检测 codex 是否已安装（which codex）
  │   ├─ 未安装 → 自动运行 npm install -g @openai/codex（需要 node/npm）
  │   │           或打印安装命令让用户手动执行
  │   └─ 写入 Codex 配置：
  │       ~/.codex/config.toml → model_provider="huazhen", model="huazhen-fable-5",
  │                               base_url="https://baizor.com/v1", wire_api="chat"
  │       ~/.codex/auth.json   → OPENAI_API_KEY = <api_key>
  │
  └─ [claude] (可选，--app claude 或 --app all)
      ├─ 检测 claude 是否已安装
      └─ 写入 ~/.claude/settings.json →
             ANTHROPIC_AUTH_TOKEN, ANTHROPIC_BASE_URL, ANTHROPIC_MODEL
```

登录成功后自动触发 `install --app codex`（首次配置体验）。

### 4. TUI 交互模式（无参数运行）

```
huazhen
  │
  ├─ 若未登录 → 引导登录页面
  └─ 已登录 → 主界面
       ├─ Tab 1: 状态（当前 key、模型、工具安装情况）
       ├─ Tab 2: 模型选择列表
       └─ Tab 3: 安装/重配置
```

---

## 实施步骤

### 阶段一：脚手架（P0）

- [ ] 创建 `Cargo.toml`（设置 bin 名 `huazhen`）
- [ ] `src/main.rs`：clap 解析 + 命令分发
- [ ] `src/error.rs`：`AppError` 枚举
- [ ] `src/config.rs`：读写 `~/.huazhen/config.json`

### 阶段二：登录命令（P0）

- [ ] `src/services/login.rs`：`LoginService::generate_token` / `login_url` / `poll_for_key`
- [ ] `src/cli/commands/login.rs`：打印 URL、轮询、保存 key
- [ ] 单元测试：token 格式、URL 拼接

### 阶段三：模型列表（P1）

- [ ] `src/services/model_fetch.rs`：GET `/v1/models` 返回 `Vec<FetchedModel>`
- [ ] `src/cli/commands/models.rs`：展示列表，`--select` 时更新默认模型

### 阶段四：安装器（P1）

- [ ] `src/services/installer.rs`：
  - 检测 `claude` / `codex` 二进制
  - 写入 claude settings.json（仅 env 字段，不破坏其他配置）
  - 写入 codex config.toml + auth.json
- [ ] `src/cli/commands/install.rs`：调用 installer，带进度提示

### 阶段五：TUI 模式（P2）

- [ ] `src/tui/app.rs`：三个路由（Status / Models / Install）
- [ ] `src/tui/ui.rs`：ratatui 渲染
- [ ] `src/tui/mod.rs`：事件循环（200ms tick）
- [ ] `src/main.rs`：无参数时启动 TUI

### 阶段六：完善（P3）

- [ ] `huazhen status` 命令：打印当前配置摘要
- [ ] 错误处理与用户提示国际化
- [ ] `--json` 机器可读输出
- [ ] 集成测试

---

## 关键 API

| 用途 | 方法 | URL |
|------|------|-----|
| 打开登录页 | 浏览器 | `https://baizor.com/code/token?token=<TOKEN>` |
| 轮询 API Key | GET | `https://baizor.com/api/cli/poll?token=<TOKEN>` |
| 模型列表 | GET | `https://baizor.com/v1/models` |
| Claude 代理端点 | — | `https://baizor.com/v1` |

---

## 默认模型

所有工具统一使用 `huazhen-fable-5` 作为默认模型，用户可在登录后通过 `huazhen models --select` 或 TUI 模型页自定义。

| 字段 | 默认值 |
|------|--------|
| `default_model` | `huazhen-fable-5` |
| `haiku_model` | `huazhen-fable-5` |
| `sonnet_model` | `huazhen-fable-5` |
| `opus_model` | `huazhen-fable-5` |

服务端若在登录轮询响应中返回 `default_model` 等字段，优先使用服务端值；否则回退到 `huazhen-fable-5`。

---

## 配置文件格式

```json
// ~/.huazhen/config.json
{
  "api_key": "sk-...",
  "base_url": "https://baizor.com",
  "default_model": "huazhen-fable-5",
  "haiku_model": "huazhen-fable-5",
  "sonnet_model": "huazhen-fable-5",
  "opus_model": "huazhen-fable-5"
}
```

---

## 开发命令

```bash
cd huazhen
cargo run                    # TUI 模式
cargo run -- login           # 登录
cargo run -- models          # 列出模型
cargo run -- install         # 安装/配置工具
cargo run -- status          # 查看状态
cargo build --release        # 构建发布版
cargo test                   # 跑测试
cargo fmt && cargo clippy    # 格式化 & lint
```
