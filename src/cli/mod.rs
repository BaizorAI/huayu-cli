pub mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "huazhen", about = "华珍 — AI 编程工作台", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 登录 baizor.com 并保存 API Key
    Login(commands::login::LoginArgs),
    /// 查看当前配置与工具状态
    Status,
    /// 下载/更新 AI 工具（codex、claude）
    Update {
        /// 指定工具：codex 或 claude（默认全部）
        #[arg(default_value = "all")]
        tool: String,
    },
}
