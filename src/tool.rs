use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use crate::error::AppError;

// ── Shell discovery ───────────────────────────────────────────────────────

/// Find a POSIX-compatible shell (bash) on Windows.
/// Search order: huayu tools dir → well-known Git/MSYS2/Cygwin paths → PATH.
#[cfg(windows)]
pub fn find_bash() -> Option<PathBuf> {
    // 1. Bundled bash in huayu tools dir
    let tools = crate::services::installer::tools_dir();
    let tools_bash = tools.join("bash").join("bash.exe");
    if tools_bash.exists() {
        return Some(tools_bash);
    }
    // Also check tools/git/bin/bash.exe (if we bundle a minimal Git)
    let tools_git_bash = tools.join("git").join("bin").join("bash.exe");
    if tools_git_bash.exists() {
        return Some(tools_git_bash);
    }

    // 2. Well-known system install paths
    for candidate in [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files\Git\usr\bin\bash.exe",
        r"C:\msys64\usr\bin\bash.exe",
        r"C:\cygwin64\bin\bash.exe",
    ] {
        let p = std::path::Path::new(candidate);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }

    // 3. PATH fallback
    which::which("bash").ok()
}

#[cfg(not(windows))]
pub fn find_bash() -> Option<PathBuf> {
    std::env::var_os("SHELL")
        .map(PathBuf::from)
        .or_else(|| which::which("bash").ok())
        .or_else(|| Some(PathBuf::from("/bin/sh")))
}

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
            #[cfg(windows)]
            if local.extension().is_some_and(|e| e == "cmd") {
                if let Some(exe) = resolve_cmd_target(&local) {
                    return exe;
                }
            }
            return local;
        }
        // Fallback: search PATH. On Windows, prefer .exe over .cmd.
        // ConPTY + cmd.exe /c swallows output, so resolve .cmd → inner .exe.
        #[cfg(windows)]
        {
            for ext in ["exe", "cmd", "ps1"] {
                let name_ext = format!("{}.{}", self.binary(), ext);
                if let Ok(found) = which::which(&name_ext) {
                    if found.extension().is_some_and(|e| e == "cmd") {
                        if let Some(exe) = resolve_cmd_target(&found) {
                            return exe;
                        }
                    }
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

// ── .cmd wrapper resolution ───────────────────────────────────────────────

/// Parse an npm-generated `.cmd` wrapper and resolve the actual `.exe` it calls.
///
/// npm's global `.cmd` wrappers follow a standard pattern:
/// ```bat
/// @ECHO off
/// ...
/// SET dp0=%~dp0
/// ...
/// "%dp0%\node_modules\...\bin\tool.exe"   %*
/// ```
/// We extract the `%dp0%\...\tool.exe` path, resolve `%dp0%` to the `.cmd`
/// file's parent directory, and return the result if the `.exe` exists.
#[cfg(windows)]
fn resolve_cmd_target(cmd_path: &std::path::Path) -> Option<PathBuf> {
    let content = std::fs::read_to_string(cmd_path).ok()?;
    let dir = cmd_path.parent()?;

    for line in content.lines() {
        let trimmed = line.trim().trim_matches('"');
        // Look for lines containing %dp0% and .exe (the target binary call)
        if trimmed.contains("%dp0%") && trimmed.to_lowercase().contains(".exe") {
            // Extract the path part: strip quotes and %* args
            let path_part = line
                .trim()
                .trim_start_matches('"')
                .split("%*")
                .next()?
                .trim()
                .trim_end_matches('"')
                .trim();

            // Replace %dp0% with the actual directory
            let resolved = path_part.replace("%dp0%\\", "").replace("%dp0%/", "");
            let target = dir.join(&resolved);
            if target.exists() {
                return Some(target);
            }
        }
    }
    None
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

/// Try to parse a line as a Claude Code stream-json event (NDJSON).
/// Returns `Some(events)` if the line is valid stream-json, `None` otherwise.
fn try_parse_stream_json(line: &str) -> Option<Vec<ToolEvent>> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let event_type = v.get("type")?.as_str()?;

    match event_type {
        "assistant" => {
            let content = match v.get("content") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
                None => return Some(vec![]),
            };
            if content.trim().is_empty() {
                return Some(vec![]);
            }
            let events = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| ToolEvent::Line(l.to_string()))
                .collect();
            Some(events)
        }
        "tool_use" => {
            let tool_name = v
                .get("name")
                .and_then(|t| t.as_str())
                .unwrap_or("tool");
            let file_info = v
                .get("input")
                .and_then(|i| i.get("file_path"))
                .and_then(|f| f.as_str())
                .map(|f| format!(": {}", f))
                .unwrap_or_default();
            Some(vec![ToolEvent::Line(format!("🔧 {}{}", tool_name, file_info))])
        }
        "result" => {
            let subtype = v.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
            match subtype {
                "error" | "error_max_turns" => {
                    let msg = v
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("unknown error");
                    Some(vec![ToolEvent::Error(msg.to_string())])
                }
                _ => Some(vec![]), // success — content already streamed
            }
        }
        _ => Some(vec![]), // skip system/init events
    }
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

impl Drop for ToolProcess {
    fn drop(&mut self) {
        self.kill();
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
            c.arg("--output-format");
            c.arg("stream-json");
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
                if let Some(bash) = find_bash() {
                    c.env("SHELL", bash.as_os_str());
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
    let config_dir_str = match tool {
        ToolType::Claude => claude_config_dir.display().to_string(),
        ToolType::Codex => codex_home.display().to_string(),
    };
    // Read the SHELL we actually set on the child command.
    let child_shell_str = find_bash()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(none found!)".to_string());
    let prompt_preview = if full_prompt.len() > 80 {
        let end = full_prompt
            .char_indices()
            .take_while(|&(i, _)| i < 80)
            .map(|(i, c)| i + c.len_utf8())
            .last()
            .unwrap_or(80);
        format!("{}...", &full_prompt[..end])
    } else {
        full_prompt.clone()
    };
    let api_key_masked = if api_key.len() > 12 {
        format!("{}...{}", &api_key[..6], &api_key[api_key.len() - 4..])
    } else {
        "***".to_string()
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
            let _ = writeln!(f, "=== spawn {} @ {} ===",
                tool_label,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
            let _ = writeln!(f, "  binary:     {}", binary_path_str);
            let _ = writeln!(f, "  model:      {}", model_str);
            let _ = writeln!(f, "  base_url:   {}", base_url_str);
            let _ = writeln!(f, "  config_dir: {}", config_dir_str);
            let _ = writeln!(f, "  shell:      {}", child_shell_str);
            let _ = writeln!(f, "  api_key:    {}", api_key_masked);
            let _ = writeln!(f, "  prompt:     {}", prompt_preview);
            let _ = writeln!(f, "  pid:        {:?}", process_id);
            let _ = writeln!(f, "  cwd:        {}",
                std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "(unknown)".to_string()));
        }

        let _master_keep = master;
        let mut line_count: u32 = 0;
        let mut err_count: u32 = 0;
        let read_start = std::time::Instant::now();
        if let Some(f) = &mut log_file {
            let _ = writeln!(f, "  reader: starting...");
        }
        let buf_reader = std::io::BufReader::new(reader);
        for result in buf_reader.lines() {
            match result {
                Ok(line) => {
                    let clean = strip_ansi(&line);
                    let trimmed = clean.trim();
                    if !trimmed.is_empty() {
                        line_count += 1;
                        let elapsed = read_start.elapsed();
                        if let Some(f) = &mut log_file {
                            let _ = writeln!(f, "  [{:>3}] +{:.1}s  {}", line_count, elapsed.as_secs_f64(), trimmed);
                        }
                        // Try stream-json (NDJSON) parsing first; fall back to heuristic.
                        if let Some(events) = try_parse_stream_json(trimmed) {
                            for ev in events {
                                let _ = tx.send(ev);
                            }
                        } else {
                            let _ = tx.send(parse_event(trimmed));
                        }
                    }
                }
                Err(e) => {
                    err_count += 1;
                    if err_count <= 5 {
                        if let Some(f) = &mut log_file {
                            let _ = writeln!(f, "  [read-err #{}] {:?}", err_count, e);
                        }
                    }
                    // Non-UTF-8 line or I/O error — skip and continue
                }
            }
        }
        if let Some(f) = &mut log_file {
            let _ = writeln!(f, "  reader: EOF (read_errors: {})", err_count);
        }
        let exit_status = child.wait();
        let total_elapsed = read_start.elapsed();
        if let Some(f) = &mut log_file {
            let _ = writeln!(f, "  exit: {:?}  (lines: {}, elapsed: {:.1}s)", exit_status, line_count, total_elapsed.as_secs_f64());
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

    // ── stream-json parsing ──────────────────────────────────────────────

    #[test]
    fn stream_json_assistant_text() {
        let line = r#"{"type":"assistant","subtype":"text","content":"Hello world"}"#;
        let events = try_parse_stream_json(line).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ToolEvent::Line(s) if s == "Hello world"));
    }

    #[test]
    fn stream_json_assistant_multiline() {
        let line = r#"{"type":"assistant","subtype":"text","content":"Line 1\nLine 2\nLine 3"}"#;
        let events = try_parse_stream_json(line).unwrap();
        assert_eq!(events.len(), 3);
        assert!(matches!(&events[0], ToolEvent::Line(s) if s == "Line 1"));
        assert!(matches!(&events[2], ToolEvent::Line(s) if s == "Line 3"));
    }

    #[test]
    fn stream_json_tool_use() {
        let line = r#"{"type":"tool_use","name":"Read","input":{"file_path":"src/main.rs"}}"#;
        let events = try_parse_stream_json(line).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ToolEvent::Line(s) if s.contains("Read") && s.contains("src/main.rs")));
    }

    #[test]
    fn stream_json_result_error() {
        let line = r#"{"type":"result","subtype":"error","content":"max tokens exceeded"}"#;
        let events = try_parse_stream_json(line).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ToolEvent::Error(s) if s.contains("max tokens")));
    }

    #[test]
    fn stream_json_result_success_is_empty() {
        let line = r#"{"type":"result","subtype":"success","cost_usd":0.05}"#;
        let events = try_parse_stream_json(line).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn stream_json_system_event_is_skipped() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc"}"#;
        let events = try_parse_stream_json(line).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn non_json_returns_none() {
        assert!(try_parse_stream_json("just a regular line").is_none());
    }
}
