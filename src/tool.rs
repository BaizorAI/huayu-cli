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

    /// Preferred binary: local huayu install first, then system PATH.
    pub fn binary_path(&self) -> PathBuf {
        if let Some(local) = crate::services::installer::local_binary(self.binary()) {
            return local;
        }
        // Fallback: search PATH. On Windows, prefer .cmd/.exe wrappers
        // over bare script files (CreateProcessW cannot execute them).
        #[cfg(windows)]
        {
            for ext in ["cmd", "exe", "ps1"] {
                let name_ext = format!("{}.{}", self.binary(), ext);
                if let Ok(found) = which::which(&name_ext) {
                    return found;
                }
            }
        }
        if let Ok(found) = which::which(self.binary()) {
            return found;
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
    NetworkError(String),
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
        || (lower.contains("unauthorized")
            && (lower.contains("error") || lower.contains("status")));
    if is_auth_error {
        return ToolEvent::AuthError;
    }
    if lower.contains("connection refused")
        || lower.contains("connection error")
        || (lower.contains("network") && lower.contains("error"))
    {
        return ToolEvent::NetworkError(line.to_string());
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
    let _api_endpoint = format!("{}/v1", base_url.trim_end_matches('/'));

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 50,
            cols: 220,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AppError::Message(e.to_string()))?;

    let master = pair.master;
    let slave = pair.slave;

    let cwd = std::env::current_dir().ok();

    let cmd = match tool {
        ToolType::Codex => {
            let bin = tool.binary_path();
            let mut c = if cfg!(windows) && bin.extension().is_some_and(|e| e == "cmd") {
                let mut c = CommandBuilder::new("cmd.exe");
                c.arg("/c");
                c.arg(&bin);
                c
            } else {
                CommandBuilder::new(bin)
            };
            c.arg("exec");
            // Codex appends /responses to openai_base_url, so the URL must include /v1.
            let domain = base_url.trim_end_matches('/');
            c.arg("-c");
            c.arg(format!("openai_base_url=\"{}/v1\"", domain));
            // Disable interactive user-input requests so codex never blocks waiting
            // for a reply in huayu's non-interactive PTY environment.
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
            if let Some(ref d) = cwd {
                c.cwd(d);
            }
            c
        }
        ToolType::Claude => {
            let bin = tool.binary_path();
            let mut c = if cfg!(windows) && bin.extension().is_some_and(|e| e == "cmd") {
                let mut c = CommandBuilder::new("cmd.exe");
                c.arg("/c");
                c.arg(&bin);
                c
            } else {
                CommandBuilder::new(bin)
            };
            c.arg("--print");
            c.arg("--dangerously-skip-permissions");
            c.arg("--model");
            c.arg(model);
            if claude_max_turns > 0 {
                c.arg("--max-turns");
                c.arg(claude_max_turns.to_string());
            }
            c.arg(&full_prompt);
            c.env("CLAUDE_CONFIG_DIR", claude_config_dir.as_os_str());
            c.env("ANTHROPIC_AUTH_TOKEN", api_key);
            // Claude Code appends /v1 internally — pass the bare base URL.
            c.env("ANTHROPIC_BASE_URL", base_url.trim_end_matches('/'));
            c.env("ANTHROPIC_MODEL", model);
            // Claude Code requires a POSIX shell. On Windows, always set SHELL
            // to a discovered bash — even if the parent already has one — because
            // portable_pty may not inherit the parent environment on all code paths.
            #[cfg(windows)]
            {
                let mut shell_set = false;
                for candidate in [
                    r"C:\Program Files\Git\bin\bash.exe",
                    r"C:\Program Files\Git\usr\bin\bash.exe",
                    r"C:\msys64\usr\bin\bash.exe",
                    r"C:\cygwin64\bin\bash.exe",
                ] {
                    let shell = std::path::Path::new(candidate);
                    if shell.exists() {
                        c.env("SHELL", shell.as_os_str());
                        shell_set = true;
                        break;
                    }
                }
                if !shell_set {
                    // Last resort: look for bash on PATH
                    if let Ok(bash) = which::which("bash") {
                        c.env("SHELL", bash.as_os_str());
                    }
                }
            }
            if let Some(ref d) = cwd {
                c.cwd(d);
            }
            c
        }
    };

    // Capture spawn metadata for debug logging before moving `cmd`.
    let tool_label = tool.as_str().to_string();
    let binary_path_str = tool.binary_path().display().to_string();
    let base_url_str = base_url.to_string();
    let model_str = model.to_string();
    let shell_str = std::env::var("SHELL").unwrap_or_else(|_| "(not set)".to_string());

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
            .join(".huayu")
            .join("debug.log");
        let mut log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .ok();

        // Log spawn details for debugging.
        if let Some(f) = &mut log_file {
            let _ = writeln!(f, "");
            let _ = writeln!(f, "=== spawn {} ===", tool_label);
            let _ = writeln!(f, "  binary: {}", binary_path_str);
            let _ = writeln!(f, "  model:  {}", model_str);
            let _ = writeln!(f, "  base:   {}", base_url_str);
            let _ = writeln!(f, "  shell:  {}", shell_str);
            let _ = writeln!(f, "  pid:    {:?}", process_id);
        }

        let _master_keep = master;
        let mut line_count: u32 = 0;
        for line in std::io::BufReader::new(reader).lines().flatten() {
            let clean = strip_ansi(&line);
            let trimmed = clean.trim();
            if !trimmed.is_empty() {
                line_count += 1;
                if let Some(f) = &mut log_file {
                    let _ = writeln!(f, "  [{}] {}", line_count, trimmed);
                }
                let _ = tx.send(parse_event(trimmed));
            }
        }
        let exit_status = child.wait();
        if let Some(f) = &mut log_file {
            let _ = writeln!(f, "  exit: {:?}  (lines: {})", exit_status, line_count);
        }
        let _ = tx.send(ToolEvent::Done);
    });

    Ok(ToolProcess {
        process_id,
        writer,
        rx,
    })
}

// ── Conversation message ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Message {
    pub role: &'static str,
    pub text: String,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user",
            text: text.into(),
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: "assistant",
            text: text.into(),
        }
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
        assert!(matches!(
            parse_event("error: 401 Unauthorized"),
            ToolEvent::AuthError
        ));
    }

    #[test]
    fn auth_error_from_invalid_api_key() {
        assert!(matches!(
            parse_event("invalid api key provided"),
            ToolEvent::AuthError
        ));
    }

    #[test]
    fn auth_error_from_authentication_failed() {
        assert!(matches!(
            parse_event("authentication failed"),
            ToolEvent::AuthError
        ));
    }

    #[test]
    fn network_error_from_connection_refused() {
        assert!(matches!(
            parse_event("Connection refused"),
            ToolEvent::NetworkError(_)
        ));
    }

    #[test]
    fn network_error_from_network_error_phrase() {
        assert!(matches!(
            parse_event("network error occurred"),
            ToolEvent::NetworkError(_)
        ));
    }

    #[test]
    fn file_written_from_wrote_prefix() {
        assert!(matches!(
            parse_event("wrote src/main.rs"),
            ToolEvent::FileWritten(_)
        ));
    }

    #[test]
    fn file_written_from_created_prefix() {
        assert!(matches!(
            parse_event("created new file foo.txt"),
            ToolEvent::FileWritten(_)
        ));
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
