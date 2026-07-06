use std::path::PathBuf;
use std::sync::mpsc;

// Pinned tool versions — verified against baizor.com API compatibility.
// Update only after running integration tests (requires BAIZOR_TEST_API_KEY).
const CODEX_VERSION: &str = "0.142.5";
const CLAUDE_VERSION: &str = "1.0.3";

pub fn tools_dir() -> PathBuf {
    crate::config::config_dir().join("tools")
}

/// Resolved path to a locally managed tool binary.
/// Checks huayu's tools directory first (native binary), then falls back
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
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else {
        "unsupported"
    }
}

fn tool_archive_url(name: &str, version: &str) -> String {
    let triple = target_triple();
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    format!("https://baizor.com/install/{}-{}-{}.{}", name, version, triple, ext)
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
                "codex"  => rt.block_on(download_tool(&tx, &dir, "codex",  CODEX_VERSION)),
                "claude" => rt.block_on(download_tool(&tx, &dir, "claude", CLAUDE_VERSION)),
                other    => { let _ = tx.send(format!("[error] 未知工具: {}", other)); }
            }
        }
        let _ = tx.send("__DONE__".to_string());
    });
    Ok(rx)
}

/// Download a pkg-compiled binary from baizor.com; fall back to npm if unavailable.
async fn download_tool(tx: &mpsc::Sender<String>, dir: &PathBuf, name: &'static str, version: &'static str) {
    let url = tool_archive_url(name, version);
    let _ = tx.send(format!("[下载] {} {} ...", name, version));
    let resp = match reqwest::get(&url).await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            let _ = tx.send(format!("[提示] baizor 暂无 {} 二进制 (HTTP {})，改用 npm ...", name, r.status()));
            install_tool_npm(tx, name, npm_package(name), version);
            return;
        }
        Err(e) => {
            let _ = tx.send(format!("[提示] 下载失败 ({})，改用 npm ...", e));
            install_tool_npm(tx, name, npm_package(name), version);
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
    match extract_bundle(&bytes, dir) {
        Ok(()) => {
            let _ = std::fs::write(dir.join(format!("{}.version", name)), version);
            #[cfg(unix)]
            ensure_executable(dir, name);
            let bin_desc = local_binary(name)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "installed".to_string());
            let _ = tx.send(format!("[完成] {} {} -> {}", name, version, bin_desc));
        }
        Err(e) => {
            let _ = tx.send(format!("[错误] 解压失败: {}", e));
        }
    }
}

fn npm_package(name: &str) -> &'static str {
    match name {
        "claude" => "@anthropic-ai/claude-code",
        _        => "@openai/codex",
    }
}

fn install_tool_npm(tx: &mpsc::Sender<String>, name: &str, pkg: &str, version: &'static str) {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let _ = tx.send(format!("[安装] {}@{} (via npm) ...", pkg, version));
    let prefix = tools_dir();
    let prefix_str = prefix.to_string_lossy().into_owned();
    let pkg_ver = format!("{}@{}", pkg, version);
    let mut child = match Command::new(NPM)
        .args(["install", "--prefix", &prefix_str, &pkg_ver])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(format!("[错误] 找不到 npm: {}", e));
            let _ = tx.send("  需要 Node.js — 请从 https://nodejs.org 安装".to_string());
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
            let _ = std::fs::write(tools_dir().join(format!("{}.version", name)), version);
            let _ = tx.send(format!("[完成] {} {} 已安装", name, version));
        }
        Ok(s) => {
            let _ = tx.send(format!("[错误] npm 安装失败 (exit {})", s.code().unwrap_or(-1)));
        }
        Err(e) => {
            let _ = tx.send(format!("[错误] 等待进程失败: {}", e));
        }
    }
}

// Ensure the launcher script and node binary are executable after extraction.
#[cfg(unix)]
fn ensure_executable(dir: &PathBuf, name: &str) {
    use std::os::unix::fs::PermissionsExt;
    let targets = [
        dir.join("node"),
        dir.join("node_modules").join(".bin").join(name),
    ];
    for path in &targets {
        if path.exists() {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
        }
    }
}

// Extract all files from a zip archive, preserving directory structure (Windows).
#[cfg(windows)]
fn extract_bundle(data: &[u8], dest: &PathBuf) -> Result<(), String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let rel = file.name().replace('/', &std::path::MAIN_SEPARATOR.to_string());
        let out_path = dest.join(&rel);
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// Extract all files from a tar.gz archive (macOS/Linux).
#[cfg(not(windows))]
fn extract_bundle(data: &[u8], dest: &PathBuf) -> Result<(), String> {
    use flate2::read::GzDecoder;
    use tar::Archive;
    let gz = GzDecoder::new(data);
    let mut archive = Archive::new(gz);
    archive.unpack(dest).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TempConfigGuard;

    #[test]
    fn local_binary_returns_bundled_binary_when_present() {
        let _cfg = TempConfigGuard::new();
        let dir = tools_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let binary_name = if cfg!(windows) { "codex.exe" } else { "codex" };
        let expected = dir.join(binary_name);
        std::fs::write(&expected, b"fake binary").unwrap();
        assert_eq!(local_binary("codex"), Some(expected));
    }

    #[test]
    fn local_binary_returns_none_when_tools_dir_is_empty() {
        let _cfg = TempConfigGuard::new();
        std::fs::create_dir_all(tools_dir()).unwrap();
        assert!(local_binary("codex").is_none());
    }

    #[test]
    fn local_binary_returns_none_when_tools_dir_absent() {
        let _cfg = TempConfigGuard::new();
        // tools_dir does not exist at all
        assert!(local_binary("codex").is_none());
    }

    #[test]
    fn is_current_version_true_when_version_file_matches() {
        let _cfg = TempConfigGuard::new();
        let dir = tools_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("codex.version"), CODEX_VERSION).unwrap();
        assert!(is_current_version("codex"));
    }

    #[test]
    fn is_current_version_false_when_version_file_is_stale() {
        let _cfg = TempConfigGuard::new();
        let dir = tools_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("codex.version"), "0.0.1").unwrap();
        assert!(!is_current_version("codex"));
    }

    #[test]
    fn is_current_version_false_when_version_file_absent() {
        let _cfg = TempConfigGuard::new();
        std::fs::create_dir_all(tools_dir()).unwrap();
        assert!(!is_current_version("codex"));
    }

    #[cfg(windows)]
    #[test]
    fn local_binary_requires_exe_suffix_on_windows() {
        let _cfg = TempConfigGuard::new();
        let dir = tools_dir();
        std::fs::create_dir_all(&dir).unwrap();
        // Without .exe should not be found
        std::fs::write(dir.join("codex"), b"fake").unwrap();
        assert!(local_binary("codex").is_none());
        // With .exe should be found
        let with_ext = dir.join("codex.exe");
        std::fs::write(&with_ext, b"fake").unwrap();
        assert_eq!(local_binary("codex"), Some(with_ext));
    }
}
