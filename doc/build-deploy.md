# 智能构建与部署系统

## 概述

huayu、codex、claude 三个组件的构建与部署系统，实现：

- **有变化才构建** — 基于源码指纹检测变化，跳过无变化的组件
- **版本自动增长** — 检测到变化时自动 bump patch 版本号
- **按需部署** — 只将版本有变化的组件拷贝到服务器

## 架构

```
versions.json          ← 版本唯一来源（手动编辑 codex/claude 版本）
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
| `versions.json` | 三个组件的版本号，唯一来源 |
| `build.ps1` | 智能构建脚本 |
| `deploy.ps1` | 智能部署脚本 |
| `.build-state.json` | 构建状态（自动生成，不提交） |

## versions.json

```json
{
  "huayu": "0.2.0",
  "codex": "0.142.5",
  "claude": "1.0.3"
}
```

- **huayu** 版本由 build.ps1 自动管理（检测到源码变化自动 +1）
- **codex/claude** 版本需手动编辑（它们是外部 npm 包，升级时改这里）

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
| `src/services/installer.rs` | CODEX_VERSION, CLAUDE_VERSION |
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

## 典型工作流

### 日常开发（改了 huayu 代码）

```
.\build.ps1
  [skip] codex 0.142.5 — no changes
  [skip] claude 1.0.3 — no changes
  [build] huayu 0.2.0 → 0.2.1 (source changed)
  cargo build --release ... ok

.\deploy.ps1
  huayu  local=0.2.1  remote=0.2.0  → deploy
  codex  local=0.142.5 remote=0.142.5 → skip
  claude local=1.0.3  remote=1.0.3  → skip
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

.\deploy.ps1
  codex  local=0.143.0 remote=0.142.5 → deploy
  Done (1 component deployed)
```

### 无变化时

```
.\build.ps1
  [skip] all components — no changes

.\deploy.ps1
  All versions match remote — nothing to deploy
```
