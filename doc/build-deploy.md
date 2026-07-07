# 智能构建与部署系统

## 概述

huayu、codex、claude、skills 四个组件的构建与部署系统，实现：

- **有变化才构建** — 基于源码指纹检测变化，跳过无变化的组件
- **版本自动增长** — 检测到变化时自动 bump patch 版本号
- **按需部署** — 只将版本有变化的组件拷贝到服务器

## 架构

```
versions.json          ← 版本唯一来源（手动编辑 codex/claude/skills 版本）
       │
   build.ps1           ← 智能构建（指纹对比 → 版本bump → 同步 → 构建）
       │
       ├─→ Cargo.toml, installer.rs, package-tools.ps1, package-linux.sh  （版本同步）
       ├─→ cargo build --release → package.ps1                             （huayu构建）
       └─→ package-tools.ps1                                               （codex/claude构建）
       │
  .build-state.json    ← 构建状态记录（指纹+版本，本地不提交）
       │
   deploy.ps1          ← 智能部署（本地版本 vs 远端版本 → 按需scp）
       │
       └─→ baizor:/lucky/NewApi/data/install/  （服务器文件）
```

## 文件说明

| 文件 | 说明 |
|------|------|
| `versions.json` | 四个组件的版本号，唯一来源 |
| `build.ps1` | 智能构建脚本（指纹检测 → 版本同步 → cargo build → 调 package.ps1 打包） |
| `package.ps1` | Windows 打包脚本，将 huayu.exe + tools + skills + bash 打成 zip |
| `package-tools.ps1` | codex/claude 的 npm 打包脚本（build.ps1 在构建工具时调用） |
| `deploy.ps1` | 智能部署脚本（本地版本 vs 远端版本 → 按需 scp） |
| `.build-state.json` | 构建状态（自动生成，不提交） |

### 脚本调用关系

```
build.ps1
  ├─ cargo build --release          (huayu 源码编译)
  ├─ package.ps1 -SkipBuild         (huayu 打包：exe + tools + skills + bash → zip)
  ├─ package-tools.ps1              (codex/claude npm 打包，仅当版本变化时)
  └─ build-linux-all.sh (via WSL)   (Linux musl 交叉编译)

deploy.ps1                          (独立使用，读 .build-state.json → scp 到 baizor)
```

## versions.json

```json
{
  "huayu": "0.2.0",
  "codex": "0.142.5",
  "claude": "1.0.3",
  "skills": "0.1.0"
}
```

- **huayu** 版本由 build.ps1 自动管理（检测到源码变化自动 +1）
- **codex/claude/skills** 版本需手动编辑（codex/claude 是外部 npm 包，skills 是独立插件包，升级时改这里）

## build.ps1 使用

```powershell
.\build.ps1                     # 构建有变化的组件
.\build.ps1 -Force              # 强制构建全部
.\build.ps1 -Component huayu    # 只处理 huayu
.\build.ps1 -NoBump             # 不自动增加版本号
```

### 变化检测机制

| 组件 | 指纹来源 | 变化含义 |
|------|---------|---------|
| huayu | `src/**/*.rs` + `Cargo.toml` + `Cargo.lock` 的 SHA256 | 源码修改 |
| codex | `versions.json` 中的版本字符串 | 手动升级版本 |
| claude | `versions.json` 中的版本字符串 | 手动升级版本 |
| skills | `versions.json` 中的版本字符串 | 手动升级版本 |

skills 组件仅依赖版本号变化，无额外源码指纹。构建时 `package.ps1` 会将 `skills/` 目录打包进 huayu 安装包（随 huayu 一起分发），同时 `deploy.ps1` 可单独部署 skills zip 以供运行中的实例通过 `/skills update` 热更新。

### 构建流程

1. 读 `versions.json` 获取当前版本
2. 读 `.build-state.json` 获取上次构建状态
3. 计算各组件指纹，与上次对比
4. 对于有变化的组件：
   - 自动 bump patch 版本（如 `0.2.0` → `0.2.1`）
   - 将版本同步到所有相关文件
   - 执行构建（cargo build / npm bundle）
   - 更新 `.build-state.json`
5. 无变化的组件跳过

### 版本同步目标

build.ps1 会将 `versions.json` 中的版本写入以下文件：

| 文件 | 同步内容 |
|------|---------|
| `Cargo.toml` | huayu version |
| `src/services/installer.rs` | CODEX_VERSION, CLAUDE_VERSION, SKILLS_VERSION |
| `package-tools.ps1` | $CodexVersion, $ClaudeVersion |
| `package-linux.sh` | CODEX_VERSION, CLAUDE_VERSION |

## deploy.ps1 使用

```powershell
.\deploy.ps1                    # 部署有版本变化的组件
.\deploy.ps1 -DryRun            # 只显示差异，不实际部署
.\deploy.ps1 -Force             # 强制部署全部
```

### 部署流程

1. 读 `.build-state.json` 获取本地已构建版本
2. SSH 到 baizor 读取远端版本文件
3. 对比本地 vs 远端版本
4. 只 scp 有版本差异的组件到 `/lucky/NewApi/data/install/`

## Skills 分发

Skills 是 Claude Code 和 Codex 的插件/规则文件，huayu 通过以下机制分发热更新：

### 分发路径

| 路径 | 说明 |
|------|------|
| **内置嵌入** | `skills/claude/*.md` 和 `skills/codex/rules.md` 通过 `include_str!` 编译进 `huayu.exe`，首次启动自动安装 |
| **安装包** | `package.ps1` 打包时将 `skills/` 目录打入 zip，安装脚本解压到对应位置 |
| **热更新** | 用户执行 `/skills update` 从 `baizor.com/install/skills-{ver}.zip` 下载更新 |

### 安装目标

```
~/.huayu/
├── claude/
│   └── skills/              ← Claude Code 自动加载（CLAUDE_CONFIG_DIR）
│       ├── code-review.md
│       └── refactor.md
├── codex/
│   └── rules.md             ← Codex 规则文件
└── skills/
    └── .skills-version      ← 版本标记（"builtin" 或具体版本号）
```

### 版本标记

- 首次启动时检测 `~/.huayu/skills/.skills-version` 是否存在
- 不存在 → 写入内置 skills（不覆盖用户已手动创建的文件）
- `/skills update` → 从服务器下载，写入远程版本号

### 升级 skills

```powershell
# 1. 编辑 skills/ 目录下的文件
# 2. 编辑 versions.json，把 skills 版本号 +1
# 3. 构建 + 部署
.\build.ps1 -Component skills
.\deploy.ps1
```
用户端执行 `/skills update` 即可获取新版本。

## 典型工作流

### 日常开发（改了 huayu 代码）

```
.\build.ps1
  [skip] codex 0.142.5 — no changes
  [skip] claude 1.0.3 — no changes
  [skip] skills 0.1.0 — no changes
  [build] huayu 0.2.0 → 0.2.1 (source changed)
  cargo build --release ... ok

.\deploy.ps1
  huayu  local=0.2.1  remote=0.2.0  → deploy
  codex  local=0.142.5 remote=0.142.5 → skip
  claude local=1.0.3  remote=1.0.3  → skip
  skills  local=0.1.0  remote=0.1.0 → skip
  Done (1 component deployed)
```

### 升级外部工具（如 codex）

```
# 1. 编辑 versions.json，把 codex 改为 "0.143.0"
# 2. 构建 + 部署
.\build.ps1
  [build] codex 0.142.5 → 0.143.0 (version changed)
  [skip] claude 1.0.3 — no changes
  [skip] huayu 0.2.1 — no changes
  [skip] skills 0.1.0 — no changes

.\deploy.ps1
  codex  local=0.143.0 remote=0.142.5 → deploy
  huayu  local=0.2.1  remote=0.2.1  → skip
  claude local=1.0.3  remote=1.0.3  → skip
  skills  local=0.1.0  remote=0.1.0 → skip
  Done (1 component deployed)
```

### 无变化时

```
.\build.ps1
  [skip] all components — no changes

.\deploy.ps1
  All versions match remote — nothing to deploy
```
