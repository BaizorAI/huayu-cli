use clap::Args;
use colored::Colorize;

use crate::config::{self, write_codex_config};
use crate::error::AppError;
use crate::services::login::{LoginOutcome, LoginService, LOGIN_TIMEOUT_SECS};

#[derive(Args, Debug, Clone)]
pub struct LoginArgs {
    /// Baizor instance base URL
    #[arg(long, default_value = "https://baizor.com")]
    pub base_url: String,
}

pub fn execute(args: LoginArgs) -> Result<(), AppError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| AppError::Message(e.to_string()))?;

    let token = LoginService::generate_token();
    let login_url = LoginService::login_url(&args.base_url, &token);
    let timeout_mins = LOGIN_TIMEOUT_SECS / 60;

    println!();
    println!("{}", "╔══════════════════════════════════════════════════════════╗".bright_blue().bold());
    println!("{}", "║            华珍 huazhen — 浏览器登录                    ║".bright_blue().bold());
    println!("{}", "╠══════════════════════════════════════════════════════════╣".bright_blue().bold());
    println!("║  {}  ║", login_url.bright_white().bold());
    println!("{}", "╚══════════════════════════════════════════════════════════╝".bright_blue().bold());
    println!();
    println!("{} 1. 在浏览器中打开上方链接", "│".dimmed());
    println!("{} 2. 登录 Baizor 账号", "│".dimmed());
    println!("{} 3. 获取 API Key（点击后自动捕获）", "│".dimmed());
    println!();
    println!("{}", format!("等待登录（超时 {timeout_mins} 分钟）...").dimmed());

    let outcome: LoginOutcome = rt
        .block_on(LoginService::poll_for_key(&args.base_url, &token))
        .map_err(AppError::Message)?;

    let mut cfg = config::load();
    cfg.api_key = outcome.api_key.clone();
    cfg.base_url = args.base_url.clone();
    if let Some(m) = outcome.default_model {
        cfg.default_model = m;
    }
    config::save(&cfg)?;
    write_codex_config(&cfg)?;

    println!();
    println!("{}", "✓ 登录成功！配置已保存。".bright_green().bold());
    println!("  Key: {}", mask_key(&outcome.api_key));
    println!("  模型: {}", cfg.default_model);
    println!();
    println!("{}", "运行 `huazhen` 启动 TUI 工作台".dimmed());

    Ok(())
}

fn mask_key(key: &str) -> String {
    let s = key.strip_prefix("sk-").unwrap_or(key);
    if s.len() <= 8 {
        return "***".to_string();
    }
    format!("sk-{}***{}", &s[..4], &s[s.len() - 4..])
}
