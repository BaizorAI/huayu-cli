use std::sync::mpsc;
use colored::Colorize;
use crate::services::installer;

pub fn execute(tools: Vec<&'static str>) {
    let names_display = tools.join(", ");
    println!();
    println!("{}", format!("── 安装工具: {} ──────────────────────────────────", names_display)
        .bright_blue().bold());
    println!();

    let rx: mpsc::Receiver<String> = match installer::download_tools(tools) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("  {} {}", "[错误]".red(), e);
            std::process::exit(1);
        }
    };

    let mut ok = true;
    for line in rx {
        if line == "__DONE__" {
            break;
        }
        if line.contains("[错误]") || line.contains("[error]") {
            println!("  {}", line.red());
            ok = false;
        } else if line.contains("[完成]") {
            println!("  {}", line.bright_green());
        } else {
            println!("  {}", line);
        }
    }

    println!();
    if ok {
        println!("{}", "  全部完成！运行 `huazhen` 启动。".bright_green());
        // Refresh tool configs (e.g. codex model_info) after a successful update
        // so that the fix takes effect without requiring a re-login.
        let cfg = crate::config::load();
        if !cfg.api_key.is_empty() {
            let _ = crate::config::write_codex_config(&cfg);
            let _ = crate::config::write_claude_config(&cfg);
        }
    } else {
        println!("{}", "  部分工具安装失败，请检查上方错误信息。".red());
        std::process::exit(1);
    }
    println!();
}
