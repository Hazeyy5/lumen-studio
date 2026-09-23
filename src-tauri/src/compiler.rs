use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::bins::find_binary;
use crate::projects::write_rbxts_layout;
use crate::rojo::{start_rojo, stop_rojo, RojoState, RojoStatus};
use crate::studio::{install_studio_plugin, OfferState};

pub struct CompilerState {
    pub child: Option<Child>,
    pub project_path: Option<String>,
    pub last_error: Option<String>,
}

impl Default for CompilerState {
    fn default() -> Self {
        Self {
            child: None,
            project_path: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerStatus {
    pub watching: bool,
    pub ready: bool,
    pub project_path: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub compiler: CompilerStatus,
    pub rojo: RojoStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainStatus {
    pub node: bool,
    pub npm: bool,
    pub node_path: Option<String>,
}

#[tauri::command(async)]
pub fn toolchain_status() -> ToolchainStatus {
    let node = find_binary(&["node.exe", "node"]);
    let npm = find_binary(&["npm.cmd", "npm"]);
    ToolchainStatus {
        node: node.is_some(),
        npm: npm.is_some(),
        node_path: node.map(|p| p.to_string_lossy().into()),
    }
}

fn npm() -> Result<PathBuf, String> {
    find_binary(&["npm.cmd", "npm"]).ok_or("npm introuvable. Installe Node.js.".into())
}

fn hide(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
}

fn rbxtsc_bin(project: &Path) -> Result<PathBuf, String> {
    let bin_dir = project.join("node_modules").join(".bin");
    let names = if cfg!(windows) {
        ["rbxtsc.cmd", "rbxtsc.exe", "rbxtsc"]
    } else {
        ["rbxtsc", "rbxtsc.cmd", "rbxtsc.exe"]
    };
    for name in names {
        let path = bin_dir.join(name);
        if path.exists() {
            return Ok(path);
        }
    }
    find_binary(&["rbxtsc.cmd", "rbxtsc"])
        .ok_or("rbxtsc introuvable. npm install a-t-il réussi ?".into())
}

fn stop_compiler_inner(state: &tauri::State<Mutex<CompilerState>>) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if let Some(mut child) = guard.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    guard.project_path = None;
    Ok(())
}

fn out_ready(project: &Path) -> bool {
    project.join("out").join("server").exists() && project.join("include").exists()
}

#[tauri::command(async)]
pub fn compiler_status(state: tauri::State<Mutex<CompilerState>>) -> CompilerStatus {
    let mut guard = state.lock().unwrap();
    let watching = if let Some(child) = guard.child.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => {
                guard.child = None;
                false
            }
            _ => true,
        }
    } else {
        false
    };
    let project_path = guard.project_path.clone();
    let ready = project_path
        .as_ref()
        .map(|p| out_ready(Path::new(p)))
        .unwrap_or(false);
    CompilerStatus {
        watching,
        ready,
        project_path,
        last_error: guard.last_error.clone(),
    }
}

fn ensure_deps(project: &Path) -> Result<(), String> {
    write_rbxts_layout(project)?;
    if project.join("node_modules").join("roblox-ts").exists() {
        return Ok(());
    }
    let mut cmd = Command::new(npm()?);
    cmd.args(["install", "--no-fund", "--no-audit"])
        .current_dir(project)
        .stdin(Stdio::null());
    hide(&mut cmd);
    let output = cmd.output().map_err(|e| format!("npm install: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        return Err(format!("npm install a échoué: {err}{out}"));
    }
    Ok(())
}

fn compile_once(project: &Path) -> Result<(), String> {
    let mut cmd = Command::new(rbxtsc_bin(project)?);
    cmd.current_dir(project).stdin(Stdio::null());
    hide(&mut cmd);
    let output = cmd.output().map_err(|e| format!("rbxtsc: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        return Err(format!("Compilation TypeScript → Luau: {err}{out}"));
    }
    Ok(())
}

fn start_watch(
    state: &tauri::State<Mutex<CompilerState>>,
    project: &Path,
) -> Result<(), String> {
    stop_compiler_inner(state)?;
    let mut cmd = Command::new(rbxtsc_bin(project)?);
    cmd.arg("-w")
        .current_dir(project)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide(&mut cmd);
    let child = cmd.spawn().map_err(|e| format!("rbxtsc -w: {e}"))?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.child = Some(child);
    guard.project_path = Some(project.to_string_lossy().into());
    guard.last_error = None;
    Ok(())
}

#[tauri::command(async)]
pub fn start_sync(
    compiler: tauri::State<Mutex<CompilerState>>,
    rojo: tauri::State<Mutex<RojoState>>,
    offer: tauri::State<OfferState>,
    project_path: String,
) -> Result<SyncStatus, String> {
    let project = PathBuf::from(&project_path);
    {
        let mut guard = compiler.lock().map_err(|e| e.to_string())?;
        guard.last_error = None;
    }
    if let Err(err) = (|| {
        ensure_deps(&project)?;
        compile_once(&project)?;
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if out_ready(&project) {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if !out_ready(&project) {
            return Err("rbxtsc n'a pas produit out/ + include/".into());
        }
        start_watch(&compiler, &project)?;
        Ok::<(), String>(())
    })() {
        let mut guard = compiler.lock().map_err(|e| e.to_string())?;
        guard.last_error = Some(err.clone());
        return Err(err);
    }
    let _ = crate::projects::ensure_agent_bridge(&project_path);
    let rojo_status = start_rojo(rojo, offer, project_path)?;
    let _ = install_studio_plugin();
    Ok(SyncStatus {
        compiler: compiler_status(compiler),
        rojo: rojo_status,
    })
}

#[tauri::command(async)]
pub fn stop_sync(
    compiler: tauri::State<Mutex<CompilerState>>,
    rojo: tauri::State<Mutex<RojoState>>,
    offer: tauri::State<OfferState>,
) -> Result<SyncStatus, String> {
    stop_compiler_inner(&compiler)?;
    let rojo_status = stop_rojo(rojo, offer)?;
    Ok(SyncStatus {
        compiler: compiler_status(compiler),
        rojo: rojo_status,
    })
}
