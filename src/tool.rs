use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use crate::error::AppError;

// ── Tool type ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolType {
    Codex,
    Claude,
}

impl ToolType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolType::Codex => "codex",
            ToolType::Claude => "claude",
        }
    }

    pub fn binary(&self) -> &'static str {
        match self {
            ToolType::Codex => "codex",
            ToolType::Claude => "claude",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "claude" => ToolType::Claude,
            _ => ToolType::Codex,
        }
    }

    /// Preferred binary: local huazhen install first, then system PATH.
    pub fn binary_path(&self) -> PathBuf {
        if let Some(local) = crate::services::installer::local_binary(self.binary()) {
            return local;
        }
        PathBuf::from(self.binary())
    }

    pub fn is_available(&self) -> bool {
        crate::services::installer::local_binary(self.binary()).is_some()
            || which::which(self.binary()).is_ok()
    }
}

// ── Tool event ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ToolEvent {
    Line(String),
    FileWritten(String),
    TestPassed,
    TestFailed(String),
    AuthError,
    NetworkError,
    Done,
    Error(String),
}

/// Parse a raw log line into a structured ToolEvent.
pub fn parse_event(line: &str) -> ToolEvent {
    let lower = line.to_lowercase();
    // Use strict patterns to avoid false positives on normal log output.
    let is_auth_error = lower.contains("401 unauthorized")
        || lower.contains("authentication failed")
        || lower.contains("invalid api key")
        || lower.contains("incorrect api key")
        || (lower.contains("unauthorized") && (lower.contains("error") || lower.contains("status")));
    if is_auth_error {
        return ToolEvent::AuthError;
    }
    if lower.contains("connection refused") || (lower.contains("network") && lower.contains("error")) {
        return ToolEvent::NetworkError;
    }
    if lower.contains("wrote ") || lower.contains("written ") || lower.contains("created ") {
        return ToolEvent::FileWritten(line.to_string());
    }
    if lower.contains("test") && (lower.contains("pass") || lower.contains(" ok")) {
        return ToolEvent::TestPassed;
    }
    if lower.contains("test") && lower.contains("fail") {
        return ToolEvent::TestFailed(line.to_string());
    }
    ToolEvent::Line(line.to_string())
}

// ── Running process ────────────────────────────────────────────────────────

pub struct ToolProcess {
    process_id: Option<u32>,
    /// Write handle to the PTY master — forwards keystrokes to the running tool.
    writer: Box<dyn Write + Send>,
    pub rx: mpsc::Receiver<ToolEvent>,
}

impl ToolProcess {
    pub fn kill(&mut self) {
        if let Some(pid) = self.process_id.take() {
            kill_process(pid);
        }
    }

    /// Send a line of text to the tool's PTY stdin (e.g. "y\n").
    pub fn write_input(&mut self, text: &str) {
        let _ = self.writer.write_all(text.as_bytes());
        let _ = self.writer.flush();
    }

    pub fn drain(&self) -> Vec<ToolEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            out.push(ev);
        }
        out
    }
}

fn kill_process(pid: u32) {
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .spawn();
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .spawn();
    }
}

// ── Spawn helpers ──────────────────────────────────────────────────────────

fn build_prompt(history: &[Message], current: &str) -> String {
    if history.is_empty() {
        return current.to_string();
    }
    let mut parts: Vec<String> = history
        .iter()
        .map(|m| format!("[{}]: {}", m.role, m.text))
        .collect();
    parts.push(format!("[user]: {}", current));
    parts.join("\n")
}

/// Spawn the tool inside a PTY so it behaves as if connected to a real terminal.
pub fn spawn(
    tool: &ToolType,
    history: &[Message],
    prompt: &str,
    api_key: &str,
    base_url: &str,
    model: &str,
    codex_home: &PathBuf,
    claude_config_dir: &PathBuf,
    codex_full_auto: bool,
    codex_reasoning_effort: &str,
    claude_max_turns: u32,
) -> Result<ToolProcess, AppError> {
    let full_prompt = build_prompt(history, prompt);
    let api_endpoint = format!("{}/v1", base_url.trim_end_matches('/'));

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize { rows: 50, cols: 220, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| AppError::Message(e.to_string()))?;

    let master = pair.master;
    let slave  = pair.slave;

    let cwd = std::env::current_dir().ok();

    let cmd = match tool {
        ToolType::Codex => {
            let mut c = CommandBuilder::new(tool.binary_path());
            c.arg("exec");
            // --ignore-user-config prevents config.toml from being loaded so codex
            // never sees a [model_providers.openai] block (reserved name → validation error).
            // Auth still reads from CODEX_HOME/auth.json.
            c.arg("--ignore-user-config");
            // Codex appends /responses to openai_base_url, so the URL must include /v1.
            let domain = base_url.trim_end_matches('/');
            c.arg("-c");
            c.arg(format!("openai_base_url=\"{}/v1\"", domain));
            // Disable interactive user-input requests so codex never blocks waiting
            // for a reply in huazhen's non-interactive PTY environment.
            c.arg("-c");
            c.arg("disable_response_storage=true");
            // Apply reasoning effort if set
            if !codex_reasoning_effort.is_empty() {
                c.arg("-c");
                c.arg(format!("reasoning_effort=\"{}\"", codex_reasoning_effort));
            }
            c.arg("--model");
            c.arg(model);
            if codex_full_auto {
                c.arg("--dangerously-bypass-approvals-and-sandbox");
            }
            c.arg(&full_prompt);
            c.env("CODEX_HOME", codex_home.as_os_str());
            c.env("OPENAI_API_KEY", api_key);
            if let Some(ref d) = cwd { c.cwd(d); }
            c
        }
        ToolType::Claude => {
            let mut c = CommandBuilder::new(tool.binary_path());
            c.arg("--print");
            c.arg("--dangerously-skip-permissions");
            if claude_max_turns > 0 {
                c.arg("--max-turns");
                c.arg(claude_max_turns.to_string());
            }
            c.arg(&full_prompt);
            c.env("CLAUDE_CONFIG_DIR", claude_config_dir.as_os_str());
            c.env("ANTHROPIC_AUTH_TOKEN", api_key);
            c.env("ANTHROPIC_BASE_URL", api_endpoint.as_str());
            if let Some(ref d) = cwd { c.cwd(d); }
            c
        }
    };

    let mut child = slave.spawn_command(cmd).map_err(|e| {
        let s = e.to_string().to_lowercase();
        if s.contains("no such file") || s.contains("not found") || s.contains("cannot find") {
            AppError::ToolNotFound(tool.binary().to_string())
        } else {
            AppError::Message(e.to_string())
        }
    })?;

    let process_id = child.process_id();
    let (tx, rx) = mpsc::channel::<ToolEvent>();

    // Drop slave *before* reading so the master sees EOF when the child exits.
    drop(slave);

    // take_writer() consumes the write side; try_clone_reader() gives the read side.
    let writer = master
        .take_writer()
        .map_err(|e| AppError::Message(e.to_string()))?;

    let reader = master
        .try_clone_reader()
        .map_err(|e| AppError::Message(e.to_string()))?;

    std::thread::spawn(move || {
        // Write full untruncated output to a debug log so errors aren't cut off by TUI column width.
        let log_path = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".huazhen")
            .join("debug.log");
        let mut log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .ok();

        let _master_keep = master;
        for line in std::io::BufReader::new(reader).lines().flatten() {
            let clean = strip_ansi(&line);
            let trimmed = clean.trim();
            if !trimmed.is_empty() {
                if let Some(f) = &mut log_file {
                    let _ = writeln!(f, "{}", trimmed);
                }
                let _ = tx.send(parse_event(trimmed));
            }
        }
        let _ = child.wait();
        let _ = tx.send(ToolEvent::Done);
    });

    Ok(ToolProcess { process_id, writer, rx })
}

// ── Conversation message ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Message {
    pub role: &'static str,
    pub text: String,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: "user", text: text.into() }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self { role: "assistant", text: text.into() }
    }
}

// ── ANSI strip ─────────────────────────────────────────────────────────────

fn strip_ansi(s: &str) -> String {
    strip_ansi_escapes::strip_str(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_error_from_401_unauthorized() {
        assert!(matches!(parse_event("error: 401 Unauthorized"), ToolEvent::AuthError));
    }

    #[test]
    fn auth_error_from_invalid_api_key() {
        assert!(matches!(parse_event("invalid api key provided"), ToolEvent::AuthError));
    }

    #[test]
    fn auth_error_from_authentication_failed() {
        assert!(matches!(parse_event("authentication failed"), ToolEvent::AuthError));
    }

    #[test]
    fn network_error_from_connection_refused() {
        assert!(matches!(parse_event("Connection refused"), ToolEvent::NetworkError));
    }

    #[test]
    fn network_error_from_network_error_phrase() {
        assert!(matches!(parse_event("network error occurred"), ToolEvent::NetworkError));
    }

    #[test]
    fn file_written_from_wrote_prefix() {
        assert!(matches!(parse_event("wrote src/main.rs"), ToolEvent::FileWritten(_)));
    }

    #[test]
    fn file_written_from_created_prefix() {
        assert!(matches!(parse_event("created new file foo.txt"), ToolEvent::FileWritten(_)));
    }

    #[test]
    fn test_passed_from_pass_keyword() {
        assert!(matches!(parse_event("test passed"), ToolEvent::TestPassed));
    }

    #[test]
    fn test_passed_from_ok_keyword() {
        assert!(matches!(parse_event("test ok"), ToolEvent::TestPassed));
    }

    #[test]
    fn test_failed_event() {
        assert!(matches!(
            parse_event("test failed: assertion error"),
            ToolEvent::TestFailed(_)
        ));
    }

    #[test]
    fn normal_line_passes_through() {
        let line = "compiling main.rs";
        assert!(matches!(parse_event(line), ToolEvent::Line(s) if s == line));
    }
}
