use std::sync::mpsc;
use std::time::Instant;

use crate::command::{self, AppCommand, UpdateTarget};
use crate::config::HuazhenConfig;
use crate::services::login::{LoginOutcome, LoginService};
use crate::tool::{Message, ToolEvent, ToolProcess, ToolType};

// ── Connection status ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    NotConfigured,
    AuthError,
    NetworkError,
    ToolNotFound(String),
}

impl ConnectionStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Connected => "●连接中",
            Self::NotConfigured => "○未配置",
            Self::AuthError => "✗认证失败",
            Self::NetworkError => "✗服务不可用",
            Self::ToolNotFound(_) => "✗工具未找到",
        }
    }
}

// ── Login overlay ──────────────────────────────────────────────────────────

pub enum LoginState {
    Waiting,
    Error(String),
}

pub struct LoginOverlay {
    pub url: String,
    pub state: LoginState,
    pub result_rx: mpsc::Receiver<Result<LoginOutcome, String>>,
}

// ── App ────────────────────────────────────────────────────────────────────

pub struct App {
    pub config: HuazhenConfig,

    // Active tool
    pub tool_type: ToolType,
    pub tool_process: Option<ToolProcess>,
    pub connection_status: ConnectionStatus,

    // Chat history (conversation context passed to tool)
    pub messages: Vec<Message>,

    // Main unified output panel
    pub main_lines: Vec<String>,
    /// Distance from the bottom (0 = bottom-anchored, tail-f mode).
    pub scroll_offset: usize,
    pub auto_scroll: bool,

    // Bottom input
    pub input: String,
    pub cursor_pos: usize,

    // Input history (session-only, max 50 entries)
    pub input_history: Vec<String>,
    /// None = editing draft; Some(i) = browsing history[i]
    pub history_cursor: Option<usize>,
    /// Saved draft while browsing history
    pub input_draft: String,

    // Task timing
    pub task_start: Option<Instant>,

    // Background download/install progress
    pub update_rx: Option<mpsc::Receiver<String>>,

    // Login overlay
    pub login_overlay: Option<LoginOverlay>,

    // Settings overlay
    pub show_settings: bool,
    pub settings_model_input: String,
    pub settings_url_input: String,
    pub settings_focus_field: usize,

    // Recent slash commands (max 5, shown in right panel)
    pub recent_commands: Vec<String>,

    pub should_quit: bool,
}

impl App {
    pub fn new(config: HuazhenConfig) -> Self {
        let tool_type = ToolType::from_str(&config.active_tool);
        let connection_status = if config.api_key.is_empty() {
            ConnectionStatus::NotConfigured
        } else {
            ConnectionStatus::Connected
        };
        let settings_model_input = config.default_model.clone();
        let settings_url_input = config.base_url.clone();

        // Startup tool availability check
        let mut startup_lines: Vec<String> = Vec::new();
        for tool in [ToolType::Codex, ToolType::Claude] {
            let name = tool.binary();
            if !tool.is_available() {
                startup_lines.push(format!(
                    "[提示] {} 未找到 — 运行 /update 下载工具",
                    name
                ));
            } else if crate::services::installer::local_binary(name).is_some()
                && !crate::services::installer::is_current_version(name)
            {
                startup_lines.push(format!(
                    "[更新] {} 可更新至 {} — 运行 /update",
                    name,
                    crate::services::installer::pinned_version(name)
                ));
            }
        }

        Self {
            config,
            tool_type,
            tool_process: None,
            connection_status,
            messages: Vec::new(),
            main_lines: startup_lines,
            scroll_offset: 0,
            auto_scroll: true,
            input: String::new(),
            cursor_pos: 0,
            input_history: Vec::new(),
            history_cursor: None,
            input_draft: String::new(),
            task_start: None,
            update_rx: None,
            login_overlay: None,
            show_settings: false,
            settings_model_input,
            settings_url_input,
            settings_focus_field: 0,
            recent_commands: Vec::new(),
            should_quit: false,
        }
    }

    // ── Input editing ──────────────────────────────────────────────────────

    pub fn input_push(&mut self, ch: char) {
        // Any edit while browsing history exits history mode
        self.exit_history();
        self.input.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
    }

    pub fn input_backspace(&mut self) {
        self.exit_history();
        if self.cursor_pos == 0 {
            return;
        }
        let ch_len = self.input[..self.cursor_pos]
            .chars()
            .last()
            .map_or(0, |c| c.len_utf8());
        self.cursor_pos -= ch_len;
        self.input.remove(self.cursor_pos);
    }

    pub fn input_newline(&mut self) {
        self.exit_history();
        self.input.insert(self.cursor_pos, '\n');
        self.cursor_pos += 1;
    }

    // ── Input history navigation ───────────────────────────────────────────

    pub fn history_up(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        match self.history_cursor {
            None => {
                // Save current draft and jump to most recent entry
                self.input_draft = self.input.clone();
                let idx = self.input_history.len() - 1;
                self.history_cursor = Some(idx);
                self.input = self.input_history[idx].clone();
                self.cursor_pos = self.input.len();
            }
            Some(0) => {} // Already at oldest entry
            Some(i) => {
                let idx = i - 1;
                self.history_cursor = Some(idx);
                self.input = self.input_history[idx].clone();
                self.cursor_pos = self.input.len();
            }
        }
    }

    pub fn history_down(&mut self) {
        match self.history_cursor {
            None => {} // Not in history mode
            Some(i) if i + 1 < self.input_history.len() => {
                let idx = i + 1;
                self.history_cursor = Some(idx);
                self.input = self.input_history[idx].clone();
                self.cursor_pos = self.input.len();
            }
            Some(_) => {
                // At newest entry; restore draft
                self.history_cursor = None;
                self.input = self.input_draft.clone();
                self.input_draft = String::new();
                self.cursor_pos = self.input.len();
            }
        }
    }

    fn exit_history(&mut self) {
        if self.history_cursor.is_some() {
            self.history_cursor = None;
            self.input_draft = String::new();
        }
    }

    // ── Panel output ───────────────────────────────────────────────────────

    fn push_main(&mut self, line: impl Into<String>) {
        self.main_lines.push(line.into());
        const MAX_LINES: usize = 10_000;
        if self.main_lines.len() > MAX_LINES {
            self.main_lines.remove(0);
        }
        if !self.auto_scroll {
            // Increment offset so the view stays anchored to the same content
            self.scroll_offset = self.scroll_offset.saturating_add(1);
        }
        // When auto_scroll=true, scroll_offset stays 0 and the view follows the latest line
    }

    pub fn push_output(&mut self, line: impl Into<String>) {
        self.push_main(line);
    }

    pub fn push_progress(&mut self, line: impl Into<String>) {
        self.push_main(line);
    }

    // ── Drain tool events ──────────────────────────────────────────────────

    pub fn drain_tool_events(&mut self) {
        let events: Vec<ToolEvent> = if let Some(proc) = &self.tool_process {
            proc.drain()
        } else {
            return;
        };

        for ev in events {
            match &ev {
                ToolEvent::Line(s) => self.push_main(s.clone()),
                ToolEvent::FileWritten(s) => {
                    self.push_main(format!("[文件] {}", s));
                }
                ToolEvent::TestPassed => {
                    self.push_main("✓ 测试通过");
                }
                ToolEvent::TestFailed(s) => {
                    self.push_main(format!("✗ 测试失败: {}", s));
                }
                ToolEvent::AuthError => {
                    self.connection_status = ConnectionStatus::AuthError;
                    self.push_main("✗ API认证失败 (401) — 请使用 /login 重新登录");
                }
                ToolEvent::NetworkError => {
                    self.connection_status = ConnectionStatus::NetworkError;
                    self.push_main("✗ 网络错误 — 请检查 baizor.com 连接");
                }
                ToolEvent::Error(s) => {
                    self.push_main(format!("✗ 错误: {}", s));
                }
                ToolEvent::Done => {
                    let elapsed = self.task_start
                        .take()
                        .map(|t| format!(" ({:.1}s)", t.elapsed().as_secs_f32()))
                        .unwrap_or_default();
                    self.push_main(format!("─── 完成{} ───", elapsed));
                    self.tool_process = None;
                }
            }
        }
    }

    // ── Drain update/install events ────────────────────────────────────────

    pub fn drain_update(&mut self) {
        let lines: Vec<String> = if let Some(rx) = &self.update_rx {
            let mut out = Vec::new();
            while let Ok(line) = rx.try_recv() {
                out.push(line);
            }
            out
        } else {
            return;
        };

        for line in lines {
            if line == "__DONE__" {
                self.update_rx = None;
                self.push_main("─── 更新结束 ───");
                // Refresh tool configs so any format changes (e.g. new model_info entries)
                // are applied immediately without requiring a restart or re-login.
                if !self.config.api_key.is_empty() {
                    let _ = crate::config::write_codex_config(&self.config);
                    let _ = crate::config::write_claude_config(&self.config);
                }
                break;
            }
            self.push_main(line);
        }
    }

    // ── Login overlay ──────────────────────────────────────────────────────

    pub fn poll_login(&mut self) {
        let result = if let Some(ov) = &self.login_overlay {
            ov.result_rx.try_recv().ok()
        } else {
            return;
        };

        if let Some(result) = result {
            match result {
                Ok(outcome) => {
                    self.config.api_key = outcome.api_key;
                    if let Some(m) = outcome.default_model {
                        self.config.default_model = m.clone();
                        self.settings_model_input = m;
                    }
                    // Apply codex settings
                    if let Some(m) = outcome.codex.model {
                        self.config.codex_model = m;
                    }
                    if let Some(fa) = outcome.codex.full_auto {
                        self.config.codex_full_auto = fa;
                    }
                    if let Some(e) = outcome.codex.reasoning_effort {
                        self.config.codex_reasoning_effort = e;
                    }
                    // Apply claude settings
                    if let Some(m) = outcome.claude.model {
                        self.config.claude_model = m;
                    }
                    if let Some(t) = outcome.claude.max_turns {
                        self.config.claude_max_turns = t;
                    }
                    if let Some(p) = outcome.claude.permission_mode {
                        self.config.claude_permission_mode = p;
                    }
                    // Apply model metadata from server
                    if !outcome.model_info.is_empty() {
                        self.config.model_info = outcome.model_info;
                    }
                    let _ = crate::config::save(&self.config);
                    let _ = crate::config::write_codex_config(&self.config);
                    let _ = crate::config::write_claude_config(&self.config);
                    self.connection_status = ConnectionStatus::Connected;
                    self.login_overlay = None;
                    self.push_main("✓ 登录成功！直接输入需求，或运行 /update 下载工具".to_string());
                }
                Err(e) => {
                    if let Some(ov) = &mut self.login_overlay {
                        ov.state = LoginState::Error(e);
                    }
                }
            }
        }
    }

    pub fn open_login_overlay(&mut self) {
        let token = LoginService::generate_token();
        let url = LoginService::login_url(&self.config.base_url, &token);
        let (tx, rx) = mpsc::channel::<Result<LoginOutcome, String>>();
        let base_url = self.config.base_url.clone();
        let tok = token.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            let result = rt.block_on(LoginService::poll_for_key(&base_url, &tok));
            let _ = tx.send(result);
        });

        self.login_overlay = Some(LoginOverlay {
            url,
            state: LoginState::Waiting,
            result_rx: rx,
        });
    }

    // ── Settings overlay ───────────────────────────────────────────────────

    pub fn apply_settings(&mut self) {
        let model = self.settings_model_input.trim().to_string();
        let url = self.settings_url_input.trim().to_string();
        if !model.is_empty() {
            self.config.default_model = model;
        }
        if !url.is_empty() {
            self.config.base_url = url;
            self.settings_url_input = self.config.base_url.clone();
        }
        self.config.active_tool = self.tool_type.as_str().to_string();
        let _ = crate::config::save(&self.config);
        self.show_settings = false;
        self.push_main(format!(
            "✓ 设置已保存 — 模型: {}  URL: {}",
            self.config.default_model, self.config.base_url
        ));
    }

    // ── Command dispatch ───────────────────────────────────────────────────

    pub fn handle_command(&mut self, cmd: AppCommand) {
        match cmd {
            AppCommand::Login => {
                self.open_login_overlay();
            }

            AppCommand::Switch(tool) => {
                let next = ToolType::from_str(tool.trim());
                self.switch_tool(next.clone());
                self.push_main(format!("✓ 已切换到 {}", next.as_str()));
            }

            AppCommand::Model(name) => {
                if name.is_empty() {
                    self.push_main(format!("当前模型: {}", self.config.default_model));
                } else {
                    self.config.default_model = name.clone();
                    self.settings_model_input = name.clone();
                    let _ = crate::config::save(&self.config);
                    self.push_main(format!("✓ 模型已切换到: {}", name));
                }
            }

            AppCommand::Update(target) => {
                if self.update_rx.is_some() {
                    self.push_main("⚠ 更新正在进行中，请稍候...");
                    return;
                }
                self.start_update(target);
            }

            AppCommand::Status => {
                self.show_status();
            }

            AppCommand::Help => {
                for line in command::help_lines() {
                    self.push_main(line);
                }
            }

            AppCommand::Clear => {
                self.main_lines.clear();
                self.scroll_offset = 0;
                self.auto_scroll = true;
            }

            AppCommand::Quit => {
                self.should_quit = true;
            }

            AppCommand::Unknown(name) => {
                self.push_main(format!("未知命令: /{} — 输入 /help 查看可用命令", name));
            }
        }
    }

    fn start_update(&mut self, target: UpdateTarget) {
        let names = target.tool_names();
        match crate::services::installer::download_tools(names) {
            Err(e) => {
                self.push_main(format!("✗ 无法启动更新: {}", e));
            }
            Ok(rx) => {
                self.update_rx = Some(rx);
                self.push_main("─── 开始下载/更新工具 ───");
            }
        }
    }

    fn show_status(&mut self) {
        self.push_main("── 状态 ──────────────────────────────────────────");
        if self.config.api_key.is_empty() {
            self.push_main("  API Key   未配置 — 使用 /login");
        } else {
            let k = &self.config.api_key;
            let masked = if k.len() > 8 {
                format!("sk-{}***{}", &k[..4.min(k.len())], &k[k.len().saturating_sub(4)..])
            } else {
                "***".to_string()
            };
            self.push_main(format!("  API Key   {}", masked));
        }
        self.push_main(format!("  Base URL  {}", self.config.base_url));
        self.push_main(format!("  模型      {}", self.config.default_model));
        self.push_main(format!("  工具      {}", self.tool_type.as_str()));
        self.push_main("── 工具检测 ───────────────────────────────────────");
        for tool in [ToolType::Codex, ToolType::Claude] {
            let name = tool.binary();
            let avail = tool.is_available();
            let loc = crate::services::installer::local_binary(name)
                .map(|p| format!(" (捆绑: {})", p.display()))
                .unwrap_or_else(|| {
                    if avail { " (PATH)".to_string() } else { String::new() }
                });
            let ver = if crate::services::installer::is_current_version(name) {
                format!(" v{}", crate::services::installer::pinned_version(name))
            } else if crate::services::installer::local_binary(name).is_some() {
                format!(
                    " (可更新→{})",
                    crate::services::installer::pinned_version(name)
                )
            } else {
                String::new()
            };
            self.push_main(format!(
                "  {:8} {}{}{}",
                name,
                if avail { "✓ 可用" } else { "✗ 未找到 — 运行 /update" },
                loc,
                ver,
            ));
        }
        self.push_main("──────────────────────────────────────────────────");
    }

    // ── Switch tool ────────────────────────────────────────────────────────

    pub fn switch_tool(&mut self, next: ToolType) {
        if self.tool_type == next {
            return;
        }
        if let Some(proc) = &mut self.tool_process {
            proc.kill();
            self.push_main(format!("[任务已取消] 切换到 {}", next.as_str()));
            self.tool_process = None;
            self.task_start = None;
        }
        self.tool_type = next;
        self.config.active_tool = self.tool_type.as_str().to_string();
        let _ = crate::config::save(&self.config);
    }

    // ── Submit (Enter pressed) ─────────────────────────────────────────────

    pub fn submit(&mut self) {
        let input = self.input.trim().to_string();
        // Exit history mode on submit regardless
        self.history_cursor = None;
        self.input_draft = String::new();
        if input.is_empty() {
            return;
        }
        self.input.clear();
        self.cursor_pos = 0;

        // Slash command takes priority regardless of running state.
        if let Some(cmd) = command::parse(&input) {
            // Track recent slash commands (exclude meta/destructive ones)
            if !matches!(cmd, AppCommand::Help | AppCommand::Clear | AppCommand::Quit) {
                if self.recent_commands.last().map(|s: &String| s != &input).unwrap_or(true) {
                    self.recent_commands.push(input.clone());
                    if self.recent_commands.len() > 5 {
                        self.recent_commands.remove(0);
                    }
                }
            }
            self.handle_command(cmd);
            return;
        }

        // Append non-empty free-text to input history
        if !input.is_empty() {
            if self.input_history.last().map(|s: &String| s != &input).unwrap_or(true) {
                self.input_history.push(input.clone());
                if self.input_history.len() > 50 {
                    self.input_history.remove(0);
                }
            }
        }

        // If a tool task is already running, forward input to its PTY stdin.
        if let Some(proc) = &mut self.tool_process {
            let line = format!("{}\n", input);
            proc.write_input(&line);
            self.push_main(format!("▷ {}", input));
            return;
        }

        // Plain prompt → spawn tool
        if self.config.api_key.is_empty() {
            self.push_main("⚠ 尚未登录，请先使用 /login");
            return;
        }

        let history = self.messages.clone();
        self.messages.push(Message::user(&input));
        let preview_end = input.char_indices().nth(60).map(|(i, _)| i).unwrap_or(input.len());
        let preview = &input[..preview_end];
        self.push_main(format!("─── {} ▶ {} ───", self.tool_type.as_str(), preview));

        {
            let k = &self.config.api_key;
            let masked = if k.len() > 8 {
                format!("{}...{}", &k[..4], &k[k.len() - 4..])
            } else {
                "***".to_string()
            };
            self.push_main(format!(
                "  endpoint: {}/v1  key: {}",
                self.config.base_url.trim_end_matches('/'),
                masked
            ));
        }

        let codex_model = crate::config::effective_codex_model(&self.config).to_string();
        let claude_model = crate::config::effective_claude_model(&self.config).to_string();
        let spawn_model = match self.tool_type {
            ToolType::Codex => codex_model,
            ToolType::Claude => claude_model,
        };

        match crate::tool::spawn(
            &self.tool_type,
            &history,
            &input,
            &self.config.api_key,
            &self.config.base_url,
            &spawn_model,
            &crate::config::codex_home(),
            &crate::config::claude_config_dir(),
            self.config.codex_full_auto,
            &self.config.codex_reasoning_effort,
            self.config.claude_max_turns,
        ) {
            Ok(proc) => {
                self.task_start = Some(Instant::now());
                self.tool_process = Some(proc);
            }
            Err(e) => {
                self.push_main(format!("✗ 启动失败: {} — 运行 /update 安装工具", e));
                if let crate::error::AppError::ToolNotFound(name) = &e {
                    self.connection_status = ConnectionStatus::ToolNotFound(name.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HuazhenConfig, TempConfigGuard};

    fn make_app() -> App {
        App::new(HuazhenConfig::default())
    }

    fn make_logged_in_app() -> App {
        App::new(HuazhenConfig {
            api_key: "sk-test-key".to_string(),
            ..Default::default()
        })
    }

    // ── History navigation ─────────────────────────────────────────────────

    #[test]
    fn history_up_from_empty_does_nothing() {
        let mut app = make_app();
        app.input = "draft".to_string();
        app.cursor_pos = 5;
        app.history_up();
        assert!(app.history_cursor.is_none());
        assert_eq!(app.input, "draft");
    }

    #[test]
    fn history_up_saves_draft_and_shows_most_recent_entry() {
        let mut app = make_app();
        app.input_history = vec!["first".to_string(), "second".to_string()];
        app.input = "draft".to_string();
        app.cursor_pos = 5;
        app.history_up();
        assert_eq!(app.history_cursor, Some(1));
        assert_eq!(app.input, "second");
        assert_eq!(app.input_draft, "draft");
    }

    #[test]
    fn history_up_twice_reaches_older_entry() {
        let mut app = make_app();
        app.input_history = vec!["first".to_string(), "second".to_string()];
        app.history_up();
        app.history_up();
        assert_eq!(app.history_cursor, Some(0));
        assert_eq!(app.input, "first");
    }

    #[test]
    fn history_up_stops_at_oldest_entry() {
        let mut app = make_app();
        app.input_history = vec!["only".to_string()];
        app.history_up();
        app.history_up(); // should not go past index 0
        assert_eq!(app.history_cursor, Some(0));
        assert_eq!(app.input, "only");
    }

    #[test]
    fn history_down_past_newest_restores_draft() {
        let mut app = make_app();
        app.input_history = vec!["a".to_string(), "b".to_string()];
        app.input = "draft".to_string();
        app.history_up(); // → "b", saves "draft"
        app.history_down(); // → past newest → restore draft
        assert!(app.history_cursor.is_none());
        assert_eq!(app.input, "draft");
        assert!(app.input_draft.is_empty());
    }

    #[test]
    fn history_down_when_not_browsing_does_nothing() {
        let mut app = make_app();
        app.input_history = vec!["a".to_string()];
        app.input = "draft".to_string();
        app.history_down();
        assert!(app.history_cursor.is_none());
        assert_eq!(app.input, "draft");
    }

    // ── Submit behavior ────────────────────────────────────────────────────

    #[test]
    fn submit_empty_input_does_nothing() {
        let mut app = make_app();
        let lines_before = app.main_lines.len();
        app.submit();
        assert_eq!(app.main_lines.len(), lines_before);
    }

    #[test]
    fn submit_not_logged_in_shows_login_prompt() {
        let mut app = make_app();
        app.input = "analyze project".to_string();
        app.cursor_pos = app.input.len();
        app.submit();
        assert!(
            app.main_lines.iter().any(|l| l.contains("/login") || l.contains("未登录")),
            "expected login prompt in output, got: {:?}",
            app.main_lines
        );
        assert!(app.tool_process.is_none());
    }

    #[test]
    fn submit_not_logged_in_still_records_input_history() {
        let mut app = make_app();
        app.input = "analyze project".to_string();
        app.cursor_pos = app.input.len();
        app.submit();
        assert!(app.input_history.contains(&"analyze project".to_string()));
        assert!(app.input.is_empty());
    }

    #[test]
    fn submit_slash_help_outputs_help_lines() {
        let mut app = make_logged_in_app();
        app.input = "/help".to_string();
        app.cursor_pos = 5;
        app.submit();
        assert!(
            app.main_lines.iter().any(|l| l.contains("可用命令")),
            "expected help output"
        );
    }

    #[test]
    fn submit_slash_clear_empties_main_lines() {
        let mut app = make_logged_in_app();
        app.push_output("existing line");
        app.input = "/clear".to_string();
        app.cursor_pos = 6;
        app.submit();
        assert!(app.main_lines.is_empty());
        assert_eq!(app.scroll_offset, 0);
        assert!(app.auto_scroll);
    }

    // ── Settings ──────────────────────────────────────────────────────────

    #[test]
    fn apply_settings_updates_config_and_closes_overlay() {
        let _cfg = TempConfigGuard::new();
        let mut app = make_app();
        app.settings_model_input = "new-model".to_string();
        app.settings_url_input = "https://new.example.com".to_string();
        app.show_settings = true;
        app.apply_settings();
        assert_eq!(app.config.default_model, "new-model");
        assert_eq!(app.config.base_url, "https://new.example.com");
        assert!(!app.show_settings);
        assert!(app.main_lines.iter().any(|l| l.contains("设置已保存")));
    }
}
