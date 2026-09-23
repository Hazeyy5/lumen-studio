use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub fn extra_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".cargo").join("bin"));
        dirs.push(home.join(".rokit").join("bin"));
        dirs.push(home.join(".aftman").join("bin"));
        dirs.push(home.join("AppData").join("Roaming").join("npm"));
        dirs.push(home.join("AppData").join("Local").join("Programs"));
        dirs.push(
            home.join("AppData")
                .join("Local")
                .join("Programs")
                .join("OpenAI")
                .join("Codex")
                .join("bin"),
        );
        dirs.push(home.join("AppData").join("Local").join("Lumen").join("bin"));
        dirs.push(home.join("AppData").join("Local").join("cursor-agent"));
        dirs.push(home.join(".cursor").join("bin"));
        dirs.push(home.join("AppData").join("Local").join("agy").join("bin"));
        dirs.push(home.join(".local").join("agy").join("bin"));
    }
    if let Some(data) = dirs::data_local_dir() {
        dirs.push(data.join("Lumen").join("bin"));
        dirs.push(data.join("cursor-agent"));
        dirs.push(data.join("agy").join("bin"));
    }
    dirs
}

pub fn find_binary(candidates: &[&str]) -> Option<PathBuf> {
    let key = candidates.join("\0");
    let cache = binary_cache();
    if let Ok(guard) = cache.lock() {
        if let Some((at, path)) = guard.get(&key) {
            if at.elapsed() < Duration::from_secs(30) {
                return path.clone();
            }
        }
    }
    let found = find_binary_uncached(candidates);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(key, (Instant::now(), found.clone()));
    }
    found
}

fn binary_cache() -> &'static Mutex<HashMap<String, (Instant, Option<PathBuf>)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Option<PathBuf>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn find_binary_uncached(candidates: &[&str]) -> Option<PathBuf> {
    for name in candidates {
        if let Ok(path) = which::which(name) {
            return Some(path);
        }
        for dir in extra_bin_dirs() {
            let exe = dir.join(format!("{name}.exe"));
            if exe.exists() {
                return Some(exe);
            }
            let cmd = dir.join(format!("{name}.cmd"));
            if cmd.exists() {
                return Some(cmd);
            }
            let plain = dir.join(name);
            if plain.exists() {
                return Some(plain);
            }
        }
    }
    None
}

pub fn lumen_bin_dir() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or("AppData Local introuvable")?
        .join("Lumen")
        .join("bin");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
