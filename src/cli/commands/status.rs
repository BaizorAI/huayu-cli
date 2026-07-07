use colored::Colorize;

use crate::config;
use crate::tool::ToolType;

pub fn execute() {
    let cfg = config::load();

    println!();
    println!(
        "{}",
        "── 华宇 huayu status ──────────────────────────"
            .bright_blue()
            .bold()
    );
    println!();

    // API Key
    if cfg.api_key.is_empty() {
        println!("  API Key   {}", "未配置 — 请运行 `huayu login`".red());
    } else {
        println!("  API Key   {}", mask_key(&cfg.api_key).bright_white());
    }
    println!("  Base URL  {}", cfg.base_url.bright_white());
    println!("  模型      {}", cfg.default_model.bright_white());
    println!("  工具      {}", cfg.active_tool.bright_white());
    println!();

    // Tool availability
    let tools = [ToolType::Codex, ToolType::Claude];
    println!(
        "{}",
        "── 工具检测 ──────────────────────────────────────".dimmed()
    );
    for tool in &tools {
        let avail = tool.is_available();
        let status = if avail {
            "✓ 可用".bright_green().to_string()
        } else {
            "✗ 未找到".red().to_string()
        };
        println!("  {:8} {}", tool.as_str(), status);
    }
    println!();
}

fn mask_key(key: &str) -> String {
    let s = key.strip_prefix("sk-").unwrap_or(key);
    if s.len() <= 8 {
        return "***".to_string();
    }
    format!("sk-{}***{}", &s[..4], &s[s.len() - 4..])
}
