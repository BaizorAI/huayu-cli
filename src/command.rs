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
