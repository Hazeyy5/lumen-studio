use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

fn tools_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn tools_dir() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or("AppData Local introuvable")?
        .join("Lumen")
        .join("tools");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn hide(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("Lumen/0.1")
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())
}

fn which_exe(name: &str) -> Option<PathBuf> {
    which::which(name).ok().filter(|path| path.is_file())
}

fn find_named(root: &Path, file_name: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    let mut fallback = None;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.file_name().and_then(|n| n.to_str()) != Some(file_name) {
                continue;
            }
            let text = path.to_string_lossy().replace('\\', "/");
            if file_name == "git.exe" && text.ends_with("/cmd/git.exe") {
                return Some(path);
            }
            fallback = Some(path);
        }
    }
    fallback
}

fn latest_zip(repo: &str, accept: impl Fn(&str) -> bool) -> Result<Vec<u8>, String> {
    let http = client()?;
    let release: serde_json::Value = http
        .get(format!("https://api.github.com/repos/{repo}/releases/latest"))
        .send()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    let url = release["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find_map(|asset| {
                let name = asset["name"].as_str()?;
                if accept(name) {
                    asset["browser_download_url"].as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| format!("Binaire introuvable dans {repo}"))?;
    let bytes = http
        .get(url)
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}

fn extract_zip(bytes: &[u8], dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = file.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        let rel = Path::new(&name);
        if rel
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            continue;
        }
        let out = dest.join(rel);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut reader = Vec::new();
        file.read_to_end(&mut reader).map_err(|e| e.to_string())?;
        fs::write(out, reader).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn install_zip(repo: &str, dest: &Path, accept: impl Fn(&str) -> bool) -> Result<(), String> {
    let bytes = latest_zip(repo, accept)?;
    if dest.exists() {
        let _ = fs::remove_dir_all(dest);
    }
    extract_zip(&bytes, dest)
}

pub fn git_exe() -> Result<PathBuf, String> {
    let _guard = tools_lock();
    if let Some(path) = which_exe("git") {
        return Ok(path);
    }
    let dest = tools_dir()?.join("mingit");
    if let Some(path) = find_named(&dest, "git.exe") {
        return Ok(path);
    }
    install_zip("git-for-windows/git", &dest, |name| {
        let lower = name.to_ascii_lowercase();
        lower.starts_with("mingit-")
            && lower.contains("64-bit")
            && lower.ends_with(".zip")
            && !lower.contains("busybox")
            && !lower.contains("arm64")
    })?;
    find_named(&dest, "git.exe").ok_or_else(|| "git.exe introuvable après l’installation".into())
}

pub fn gh_exe() -> Result<PathBuf, String> {
    let _guard = tools_lock();
    if let Some(path) = which_exe("gh") {
        return Ok(path);
    }
    let dest = tools_dir()?.join("gh");
    if let Some(path) = find_named(&dest, "gh.exe") {
        return Ok(path);
    }
    install_zip("cli/cli", &dest, |name| {
        let lower = name.to_ascii_lowercase();
        lower.starts_with("gh_") && lower.contains("windows_amd64") && lower.ends_with(".zip")
    })?;
    find_named(&dest, "gh.exe").ok_or_else(|| "gh.exe introuvable après l’installation".into())
}

pub fn ensure_binaries() {
    let _ = git_exe();
    let _ = gh_exe();
}

fn tool_path(git: &Path, gh: &Path) -> String {
    let mut parts = Vec::new();
    if let Some(dir) = git.parent() {
        parts.push(dir.display().to_string());
    }
    if let Some(dir) = gh.parent() {
        parts.push(dir.display().to_string());
    }
    if let Ok(current) = std::env::var("PATH") {
        parts.push(current);
    }
    parts.join(";")
}

pub fn ensure_github_login() -> Result<(), String> {
    let git = git_exe()?;
    let gh = gh_exe()?;
    let path = tool_path(&git, &gh);
    let mut status = Command::new(&gh);
    status.args(["auth", "status"]).env("PATH", &path).stdin(Stdio::null());
    hide(&mut status);
    if status.output().map(|out| out.status.success()).unwrap_or(false) {
        return Ok(());
    }
    let mut login = Command::new(&gh);
    login
        .args([
            "auth",
            "login",
            "--hostname",
            "github.com",
            "--git-protocol",
            "https",
            "--web",
        ])
        .env("PATH", &path)
        .stdin(Stdio::null());
    let result = login.status().map_err(|e| e.to_string())?;
    if result.success() {
        Ok(())
    } else {
        Err("Connexion GitHub interrompue. Réessaie : une page du navigateur doit s’ouvrir.".into())
    }
}
