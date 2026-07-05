use std::path::PathBuf;
use std::sync::mpsc;

// Pinned tool versions — verified against baizor.com API compatibility.
// Update only after running integration tests (requires BAIZOR_TEST_API_KEY).
const CODEX_VERSION: &str = "0.142.5";
// claude-code is distributed via npm; update to a native binary URL when
// Anthropic publishes platform binaries to their GitHub releases.
const CLAUDE_VERSION: &str = "1.0.3";

pub fn tools_dir() -> PathBuf {
    crate::config::config_dir().join("tools")
}

/// Resolved path to a locally managed tool binary.
/// Checks huazhen's tools directory first (native binary), then falls back
/// to the npm node_modules layout for backward compatibility with prior installs.
pub fn local_binary(name: &str) -> Option<PathBuf> {
    let dir = tools_dir();
    let direct = if cfg!(windows) {
        dir.join(format!("{}.exe", name))
    } else {
        dir.join(name)
    };
    if direct.exists() {
        return Some(direct);
    }
    // Fallback: npm node_modules layout from prior /install runs
    let npm_bin = dir.join("node_modules").join(".bin").join(name);
    #[cfg(windows)]
    {
        let cmd = npm_bin.with_extension("cmd");
        if cmd.exists() {
            return Some(cmd);
        }
    }
    if npm_bin.exists() { Some(npm_bin) } else { None }
}

pub fn pinned_version(name: &str) -> &'static str {
    match name {
        "claude" => CLAUDE_VERSION,
        _ => CODEX_VERSION,
    }
}

/// Returns true if the installed binary matches the pinned manifest version.
pub fn is_current_version(name: &str) -> bool {
    let ver_path = tools_dir().join(format!("{}.version", name));
    std::fs::read_to_string(ver_path)
        .map(|s| s.trim() == pinned_version(name))
        .unwrap_or(false)
}

fn target_triple() -> &'static str {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-musl"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-musl"
    } else {
        "unsupported"
    }
}

fn codex_archive_url() -> String {
    let triple = target_triple();
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    // GitHub release assets for openai/codex native Rust binaries.
    format!(
        "https://github.com/openai/codex/releases/download/{}/codex-{}.{}",
        CODEX_VERSION, triple, ext
    )
}

#[cfg(windows)]
const NPM: &str = "npm.cmd";
#[cfg(not(windows))]
const NPM: &str = "npm";

/// Spawn a background download/install for the given tool names and return a progress receiver.
/// Progress lines are sent on the channel; `"__DONE__"` signals completion.
pub fn download_tools(names: Vec<&'static str>) -> Result<mpsc::Receiver<String>, String> {
    let dir = tools_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建 tools 目录: {e}"))?;
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        for name in &names {
            match *name {
                "codex" => rt.block_on(download_codex(&tx, &dir)),
                "claude" => install_claude_npm(&tx),
                other => { let _ = tx.send(format!("[error] 未知工具: {}", other)); }
            }
        }
        let _ = tx.send("__DONE__".to_string());
    });
    Ok(rx)
}

async fn download_codex(tx: &mpsc::Sender<String>, dir: &PathBuf) {
    let url = codex_archive_url();
    let _ = tx.send(format!("[下载] codex {} ...", CODEX_VERSION));
    let resp = match reqwest::get(&url).await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            let _ = tx.send(format!("[错误] HTTP {} — 请检查网络或版本号", r.status()));
            return;
        }
        Err(e) => {
            let _ = tx.send(format!("[错误] 网络请求失败: {}", e));
            return;
        }
    };
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            let _ = tx.send(format!("[错误] 读取响应体失败: {}", e));
            return;
        }
    };
    let _ = tx.send(format!("[解压] {} KB ...", bytes.len() / 1024));
    let binary_name = if cfg!(windows) { "codex.exe" } else { "codex" };
    match extract_binary(&bytes, dir, binary_name) {
        Ok(path) => {
            set_executable(&path);
            let _ = std::fs::write(dir.join("codex.version"), CODEX_VERSION);
            let _ = tx.send(format!("[完成] codex {} -> {}", CODEX_VERSION, path.display()));
        }
        Err(e) => {
            let _ = tx.send(format!("[错误] 解压失败: {}", e));
        }
    }
}

fn install_claude_npm(tx: &mpsc::Sender<String>) {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let _ = tx.send(format!("[安装] claude {} (via npm) ...", CLAUDE_VERSION));
    let prefix = tools_dir();
    let prefix_str = prefix.to_string_lossy().into_owned();
    let pkg = format!("@anthropic-ai/claude-code@{}", CLAUDE_VERSION);
    let mut child = match Command::new(NPM)
        .args(["install", "--prefix", &prefix_str, &pkg])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(format!("[错误] 找不到 npm: {}", e));
            let _ = tx.send("  claude 需要 Node.js — 请从 https://nodejs.org 安装".to_string());
            return;
        }
    };
    let (stx, srx) = mpsc::channel::<String>();
    if let Some(stdout) = child.stdout.take() {
        let s = stx.clone();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines().flatten() {
                if !line.trim().is_empty() { let _ = s.send(line); }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let s = stx.clone();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines().flatten() {
                if !line.trim().is_empty() { let _ = s.send(line); }
            }
        });
    }
    drop(stx);
    for line in srx.iter() {
        let _ = tx.send(line);
    }
    match child.wait() {
        Ok(s) if s.success() => {
            let _ = std::fs::write(tools_dir().join("claude.version"), CLAUDE_VERSION);
            let _ = tx.send(format!("[完成] claude {} 已安装", CLAUDE_VERSION));
        }
        Ok(s) => {
            let _ = tx.send(format!("[错误] npm 安装失败 (exit {})", s.code().unwrap_or(-1)));
        }
        Err(e) => {
            let _ = tx.send(format!("[错误] 等待进程失败: {}", e));
        }
    }
}

// Extract the named binary from a zip archive (Windows).
#[cfg(windows)]
fn extract_binary(data: &[u8], dest: &PathBuf, binary_name: &str) -> Result<PathBuf, String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    let out_path = dest.join(binary_name);
    let mut found = false;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let base = file.name().rsplit('/').next().unwrap_or("").to_string();
        if base.eq_ignore_ascii_case(binary_name) {
            let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
            found = true;
            break;
        }
    }
    if found { Ok(out_path) } else { Err(format!("archive does not contain {}", binary_name)) }
}

// Extract the named binary from a tar.gz archive (macOS/Linux).
#[cfg(not(windows))]
fn extract_binary(data: &[u8], dest: &PathBuf, binary_name: &str) -> Result<PathBuf, String> {
    use flate2::read::GzDecoder;
    use tar::Archive;
    let gz = GzDecoder::new(data);
    let mut archive = Archive::new(gz);
    let out_path = dest.join(binary_name);
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let is_match = entry.path().ok().and_then(|p| {
            p.file_name().and_then(|n| n.to_str()).map(|n| n == binary_name)
        }).unwrap_or(false);
        if is_match {
            entry.unpack(&out_path).map_err(|e| e.to_string())?;
            return Ok(out_path);
        }
    }
    Err(format!("archive does not contain {}", binary_name))
}

#[cfg(unix)]
fn set_executable(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn set_executable(_path: &PathBuf) {}
