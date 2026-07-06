/// Parsed slash command from the input box.
#[derive(Debug, Clone)]
pub enum AppCommand {
    Login,
    Switch(String),
    Model(String),
    Update(UpdateTarget),
    Status,
    Help,
    Clear,
    Quit,
    Unknown(String),
}

#[derive(Debug, Clone)]
pub enum UpdateTarget {
    Codex,
    Claude,
    All,
}

impl UpdateTarget {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "claude" => Self::Claude,
            "codex" => Self::Codex,
            _ => Self::All,
        }
    }

    pub fn tool_names(&self) -> Vec<&'static str> {
        match self {
            Self::Codex => vec!["codex"],
            Self::Claude => vec!["claude"],
            Self::All => vec!["codex", "claude"],
        }
    }
}

pub fn parse(input: &str) -> Option<AppCommand> {
    if !input.starts_with('/') {
        return None;
    }
    let rest = input[1..].trim();
    let (cmd, args) = rest
        .split_once(' ')
        .map(|(c, a)| (c, a.trim()))
        .unwrap_or((rest, ""));

    Some(match cmd.to_lowercase().as_str() {
        "login" => AppCommand::Login,
        "switch" => AppCommand::Switch(args.to_string()),
        "model" => AppCommand::Model(args.to_string()),
        // /update and /install are synonyms; install kept for muscle memory
        "update" | "install" => AppCommand::Update(UpdateTarget::from_str(args)),
        "status" => AppCommand::Status,
        "help" | "?" => AppCommand::Help,
        "clear" => AppCommand::Clear,
        "quit" | "exit" | "q" => AppCommand::Quit,
        other => AppCommand::Unknown(other.to_string()),
    })
}

pub fn help_lines() -> Vec<&'static str> {
    vec![
        "── 可用命令 ─────────────────────────────────────",
        "/login                   登录 baizor.com",
        "/switch codex|claude     切换当前工具",
        "/model <name>            更改默认模型",
        "/update [codex|claude]   下载/更新工具（默认全部）",
        "/status                  显示配置与工具状态",
        "/clear                   清空面板",
        "/help                    显示本帮助",
        "/quit                    退出",
        "─────────────────────────────────────────────────",
        "直接输入文字并回车 → 发送给当前工具执行",
        "PgUp/PgDn 或滚轮 → 滚动输出面板",
        "↑/↓ → 输入历史导航",
        "Alt+Q → 退出程序",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_login() {
        assert!(matches!(parse("/login"), Some(AppCommand::Login)));
    }

    #[test]
    fn parse_switch_with_tool_name() {
        let cmd = parse("/switch codex").unwrap();
        assert!(matches!(cmd, AppCommand::Switch(s) if s == "codex"));
    }

    #[test]
    fn parse_model_with_name() {
        let cmd = parse("/model gpt-5.5").unwrap();
        assert!(matches!(cmd, AppCommand::Model(s) if s == "gpt-5.5"));
    }

    #[test]
    fn parse_model_without_name_gives_empty_args() {
        let cmd = parse("/model").unwrap();
        assert!(matches!(cmd, AppCommand::Model(s) if s.is_empty()));
    }

    #[test]
    fn parse_update_no_args_targets_all() {
        assert!(matches!(parse("/update"), Some(AppCommand::Update(UpdateTarget::All))));
    }

    #[test]
    fn parse_install_is_synonym_for_update_all() {
        assert!(matches!(parse("/install"), Some(AppCommand::Update(UpdateTarget::All))));
    }

    #[test]
    fn parse_update_codex() {
        assert!(matches!(
            parse("/update codex"),
            Some(AppCommand::Update(UpdateTarget::Codex))
        ));
    }

    #[test]
    fn parse_update_claude() {
        assert!(matches!(
            parse("/update claude"),
            Some(AppCommand::Update(UpdateTarget::Claude))
        ));
    }

    #[test]
    fn parse_status() {
        assert!(matches!(parse("/status"), Some(AppCommand::Status)));
    }

    #[test]
    fn parse_help_and_question_mark() {
        assert!(matches!(parse("/help"), Some(AppCommand::Help)));
        assert!(matches!(parse("/?"), Some(AppCommand::Help)));
    }

    #[test]
    fn parse_clear() {
        assert!(matches!(parse("/clear"), Some(AppCommand::Clear)));
    }

    #[test]
    fn parse_quit_synonyms() {
        assert!(matches!(parse("/quit"), Some(AppCommand::Quit)));
        assert!(matches!(parse("/exit"), Some(AppCommand::Quit)));
        assert!(matches!(parse("/q"), Some(AppCommand::Quit)));
    }

    #[test]
    fn parse_unknown_command() {
        let cmd = parse("/foobar").unwrap();
        assert!(matches!(cmd, AppCommand::Unknown(s) if s == "foobar"));
    }

    #[test]
    fn non_command_returns_none() {
        assert!(parse("hello world").is_none());
        assert!(parse("").is_none());
        assert!(parse("analyze my project").is_none());
    }

    #[test]
    fn update_target_tool_names() {
        assert_eq!(UpdateTarget::Codex.tool_names(), vec!["codex"]);
        assert_eq!(UpdateTarget::Claude.tool_names(), vec!["claude"]);
        let all = UpdateTarget::All.tool_names();
        assert!(all.contains(&"codex"));
        assert!(all.contains(&"claude"));
    }
}
