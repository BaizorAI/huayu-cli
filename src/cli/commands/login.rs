use clap::Args;
use colored::Colorize;

use crate::config::{self, write_claude_config, write_codex_config};
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
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════╗"
            .bright_blue()
            .bold()
    );
    println!(
        "{}",
        "║            华宇 huayu — 浏览器登录                      ║"
            .bright_blue()
            .bold()
    );
    println!(
        "{}",
        "╠══════════════════════════════════════════════════════════╣"
            .bright_blue()
            .bold()
    );
    println!("║  {}  ║", login_url.bright_white().bold());
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════╝"
            .bright_blue()
            .bold()
    );
    println!();
    println!("{} 1. 在浏览器中打开上方链接", "│".dimmed());
    println!("{} 2. 登录 Baizor 账号", "│".dimmed());
    println!("{} 3. 获取 API Key（点击后自动捕获）", "│".dimmed());
    println!();
    println!(
        "{}",
        format!("等待登录（超时 {timeout_mins} 分钟）...").dimmed()
    );

    let outcome: LoginOutcome = rt
        .block_on(LoginService::poll_for_key(&args.base_url, &token))
        .map_err(AppError::Message)?;

    let mut cfg = config::load();
    cfg.api_key = outcome.api_key.clone();
    cfg.base_url = args.base_url.clone();
    if let Some(m) = outcome.default_model {
        cfg.default_model = m;
    }

    // Apply codex settings from server (only when server explicitly provides a value)
    if let Some(m) = outcome.codex.model {
        cfg.codex_model = m;
    }
    if let Some(fa) = outcome.codex.full_auto {
        cfg.codex_full_auto = fa;
    }
    if let Some(e) = outcome.codex.reasoning_effort {
        cfg.codex_reasoning_effort = e;
    }

    // Apply claude settings from server (only when server explicitly provides a value)
    if let Some(m) = outcome.claude.model {
        cfg.claude_model = m;
    }
    if let Some(t) = outcome.claude.max_turns {
        cfg.claude_max_turns = t;
    }
    if let Some(p) = outcome.claude.permission_mode {
        cfg.claude_permission_mode = p;
    }

    // Apply model metadata from server (merge — server values override built-ins)
    if !outcome.model_info.is_empty() {
        cfg.model_info = outcome.model_info;
    }

    config::save(&cfg)?;
    write_codex_config(&cfg)?;
    write_claude_config(&cfg)?;

    println!();
    println!("{}", "✓ 登录成功！配置已保存。".bright_green().bold());
    println!("  Key: {}", mask_key(&outcome.api_key));
    let codex_model = config::effective_codex_model(&cfg).to_string();
    let claude_model = config::effective_claude_model(&cfg).to_string();
    println!("  Codex 模型: {}", codex_model);
    println!("  Claude 模型: {}", claude_model);
    println!();
    println!("{}", "运行 `huayu` 启动 TUI 客户端".dimmed());

    Ok(())
}

fn mask_key(key: &str) -> String {
    let s = key.strip_prefix("sk-").unwrap_or(key);
    if s.len() <= 8 {
        return "***".to_string();
    }
    format!("sk-{}***{}", &s[..4], &s[s.len() - 4..])
}
