use serde::Serialize;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use crate::bins::{find_binary, lumen_bin_dir};
use crate::studio::{clear_offer, set_offer_from_project, OfferState};

const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;

const ROJO_PORT: u16 = 34873;
const ROJO_PORT_FALLBACK: [u16; 6] = [34873, 34874, 34875, 34876, 34877, 34878];

pub struct RojoState {
    pub child: Option<Child>,
    pub project_path: Option<String>,
    pub port: u16,
}

impl Default for RojoState {
    fn default() -> Self {
        Self {
            child: None,
            project_path: None,
            port: ROJO_PORT,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RojoStatus {
    pub found: bool,
    pub path: Option<String>,
    pub serving: bool,
    pub project_path: Option<String>,
    pub port: u16,
    pub reachable: bool,
}

fn host_pe_machine() -> u16 {
    if cfg!(target_arch = "aarch64") {
        IMAGE_FILE_MACHINE_ARM64
    } else {
        IMAGE_FILE_MACHINE_AMD64
    }
}

fn host_asset_needle() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "windows-aarch64"
    } else {
        "windows-x86_64"
    }
}

fn pe_machine(path: &Path) -> Option<u16> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 64 || bytes[0] != b'M' || bytes[1] != b'Z' {
        return None;
    }
    let e_lfanew = u32::from_le_bytes(bytes[0x3C..0x40].try_into().ok()?) as usize;
    if e_lfanew + 6 > bytes.len() || &bytes[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return None;
    }
    Some(u16::from_le_bytes(
        bytes[e_lfanew + 4..e_lfanew + 6].try_into().ok()?,
    ))
}

fn exe_matches_host(path: &Path) -> bool {
    path.exists() && pe_machine(path) == Some(host_pe_machine())
}

fn rojo_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(dir) = lumen_bin_dir() {
        candidates.push(dir.join("rojo.exe"));
        candidates.push(dir.join("rojo"));
    }
    if let Some(found) = find_binary(&["rojo"]) {
        candidates.push(found);
    }
    candidates.into_iter().find(|p| exe_matches_host(p))
}

#[tauri::command(async)]
pub fn rojo_status(state: tauri::State<Mutex<RojoState>>) -> RojoStatus {
    let serving = {
        let mut guard = state.lock().unwrap();
        if let Some(child) = guard.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    guard.child = None;
                    guard.project_path = None;
                    false
                }
                _ => true,
            }
        } else {
            false
        }
    };
    let path = rojo_path();
    let reachable = serving && {
        let port = state.lock().unwrap().port;
        port_open(port)
    };
    let project_path = state.lock().unwrap().project_path.clone();
    let port = state.lock().unwrap().port;
    RojoStatus {
        found: path.is_some(),
        path: path.map(|p| p.to_string_lossy().into()),
        serving,
        project_path,
        port,
        reachable,
    }
}

fn port_open(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_millis(40),
    )
    .is_ok()
}

#[tauri::command(async)]
pub fn start_rojo(
    state: tauri::State<Mutex<RojoState>>,
    offer: tauri::State<OfferState>,
    project_path: String,
) -> Result<RojoStatus, String> {
    stop_rojo_inner(&state)?;
    let bin = match rojo_path() {
        Some(path) => path,
        None => PathBuf::from(install_rojo()?),
    };
    let project_file = PathBuf::from(&project_path).join("default.project.json");
    if !project_file.exists() {
        return Err("default.project.json introuvable dans le projet".into());
    }
    let port = pick_free_port()?;
    let log_path = lumen_bin_dir()?.join("rojo.log");
    let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&bin);
    cmd.arg("serve")
        .arg(&project_file)
        .arg("--port")
        .arg(port.to_string())
        .current_dir(&project_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Impossible de lancer Rojo: {e}"))?;
    std::thread::sleep(Duration::from_millis(600));
    if let Ok(Some(status)) = child.try_wait() {
        let tail = fs::read_to_string(&log_path).unwrap_or_default();
        return Err(format!(
            "Rojo s’est arrêté ({status}). Port {port}. {tail}"
        ));
    }
    if !port_open(port) {
        let _ = child.kill();
        let tail = fs::read_to_string(&log_path).unwrap_or_default();
        return Err(format!(
            "Rojo n’écoute pas le port {port}. {tail}"
        ));
    }
    {
        let mut guard = state.lock().unwrap();
        guard.child = Some(child);
        guard.project_path = Some(project_path.clone());
        guard.port = port;
    }
    set_offer_from_project(&offer, &project_path, true, port);
    let _ = install_rojo_studio_plugin();
    Ok(rojo_status(state))
}

fn pick_free_port() -> Result<u16, String> {
    for port in ROJO_PORT_FALLBACK {
        if !port_open(port) {
            return Ok(port);
        }
    }
    Err("Tous les ports Lumen (34873–34878) sont pris. Ferme l’autre Rojo (VibeStarter) et réessaie.".into())
}

#[tauri::command(async)]
pub fn stop_rojo(
    state: tauri::State<Mutex<RojoState>>,
    offer: tauri::State<OfferState>,
) -> Result<RojoStatus, String> {
    stop_rojo_inner(&state)?;
    clear_offer(&offer);
    Ok(rojo_status(state))
}

fn stop_rojo_inner(state: &tauri::State<Mutex<RojoState>>) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if let Some(mut child) = guard.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    guard.project_path = None;
    Ok(())
}

#[tauri::command(async)]
pub fn install_rojo() -> Result<String, String> {
    if let Some(existing) = rojo_path() {
        return Ok(existing.to_string_lossy().into());
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("Lumen/0.1")
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let release: serde_json::Value = client
        .get("https://api.github.com/repos/rojo-rbx/rojo/releases/latest")
        .send()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    let needle = host_asset_needle();
    let asset = release["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find(|a| {
                a["name"]
                    .as_str()
                    .map(|n| {
                        n.to_ascii_lowercase().contains(needle) && n.ends_with(".zip")
                    })
                    .unwrap_or(false)
            })
        })
        .ok_or_else(|| format!("Pas de binaire Rojo ({needle}) dans la dernière release"))?;
    let url = asset["browser_download_url"]
        .as_str()
        .ok_or("URL Rojo manquante")?;
    let bytes = client
        .get(url)
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    let dest_dir = lumen_bin_dir()?;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();
        if !name.ends_with("rojo.exe") && !name.ends_with("/rojo") && name != "rojo" {
            continue;
        }
        let out = dest_dir.join("rojo.exe");
        let mut dest = fs::File::create(&out).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        dest.write_all(&buf).map_err(|e| e.to_string())?;
        drop(dest);
        if !exe_matches_host(&out) {
            let _ = fs::remove_file(&out);
            return Err(format!(
                "Le binaire Rojo téléchargé n'est pas {needle}. Réessaie « Installer Rojo »."
            ));
        }
        return Ok(out.to_string_lossy().into());
    }
    Err("rojo.exe absent de l'archive".into())
}

pub fn install_rojo_studio_plugin() -> Result<String, String> {
    let bin = rojo_path().ok_or("Rojo n'est pas installé")?;
    let mut cmd = Command::new(bin);
    cmd.args(["plugin", "install"]).stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let status = cmd.status().map_err(|e| format!("rojo plugin install: {e}"))?;
    if !status.success() {
        return Err("Impossible d'installer le plugin Rojo dans Studio".into());
    }
    Ok("plugin Rojo installé".into())
}
