use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::assets::{assert_lumen_project, SavedAsset};
use crate::keys::load_keys;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlenderStatus {
    pub found: bool,
    pub path: Option<String>,
}

fn slug_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    let take: String = trimmed.chars().take(40).collect();
    if take.is_empty() {
        "mesh".into()
    } else {
        take
    }
}

fn now_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn scan_blender_dir(root: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return None;
    };
    for entry in entries.flatten() {
        let exe = entry.path().join("blender.exe");
        if exe.is_file() {
            found.push(exe);
        }
        let bin = entry.path().join("blender");
        if bin.is_file() {
            found.push(bin);
        }
    }
    found.sort();
    found.pop()
}

pub fn find_blender() -> Option<PathBuf> {
    let keys = load_keys().ok();
    if let Some(explicit) = keys
        .as_ref()
        .map(|k| k.blender_path.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        let path = PathBuf::from(&explicit);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(path) = crate::bins::find_binary(&["blender"]) {
        return Some(path);
    }
    let program_files = [
        PathBuf::from(r"C:\Program Files\Blender Foundation"),
        PathBuf::from(r"C:\Program Files (x86)\Blender Foundation"),
    ];
    for root in program_files {
        if let Some(exe) = scan_blender_dir(&root) {
            return Some(exe);
        }
    }
    if let Some(home) = dirs::home_dir() {
        if let Some(exe) = scan_blender_dir(&home.join("AppData").join("Local").join("Programs")) {
            return Some(exe);
        }
    }
    None
}

#[tauri::command]
pub fn detect_blender() -> BlenderStatus {
    match find_blender() {
        Some(path) => BlenderStatus {
            found: true,
            path: Some(path.to_string_lossy().into()),
        },
        None => BlenderStatus {
            found: false,
            path: None,
        },
    }
}

fn runner_path() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or("AppData Local introuvable")?
        .join("Lumen")
        .join("blender");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join("lumen-blender-run.py");
    fs::write(&dest, include_str!("../resources/lumen-blender-run.py")).map_err(|e| e.to_string())?;
    Ok(dest)
}

fn work_dir() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or("AppData Local introuvable")?
        .join("Lumen")
        .join("blender")
        .join("jobs");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn under_project(project: &Path, candidate: &Path) -> bool {
    let Ok(root) = project.canonicalize() else {
        return false;
    };
    let Ok(path) = candidate.canonicalize() else {
        return false;
    };
    path.starts_with(&root)
}

pub fn run_blender_mesh(
    project_path: &str,
    script: &str,
    script_path: Option<&str>,
    title: &str,
) -> Result<SavedAsset, String> {
    let project = assert_lumen_project(project_path)?;
    let blender = find_blender().ok_or(
        "Blender introuvable. Installe Blender 4.x (blender.org) ou indique le chemin dans Réglages.",
    )?;
    let stamp = now_stamp();
    let name = slug_name(if title.trim().is_empty() {
        "mesh"
    } else {
        title
    });
    let job = work_dir()?.join(format!("{name}-{stamp}"));
    fs::create_dir_all(&job).map_err(|e| e.to_string())?;

    let user_script = if let Some(path) = script_path.map(str::trim).filter(|s| !s.is_empty()) {
        let p = PathBuf::from(path);
        let full = if p.is_absolute() {
            p
        } else {
            project.join(p)
        };
        if !under_project(&project, &full) {
            return Err("Le script Blender doit être dans le projet Lumen".into());
        }
        if !full.is_file() {
            return Err("Script Blender introuvable".into());
        }
        full
    } else {
        let raw = script.trim();
        if raw.is_empty() {
            return Err("Script Blender vide".into());
        }
        if raw.len() > 200_000 {
            return Err("Script Blender trop long".into());
        }
        let dir = project.join("assets").join("blender");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dest = dir.join(format!("{name}-{stamp}.py"));
        fs::write(&dest, raw).map_err(|e| e.to_string())?;
        dest
    };

    let out_glb = job.join("model.glb");
    let runner = runner_path()?;
    let mut cmd = Command::new(&blender);
    cmd.arg("--background")
        .arg("--factory-startup")
        .arg("--python")
        .arg(&runner)
        .arg("--")
        .arg("--script")
        .arg(&user_script)
        .arg("--out")
        .arg(&out_glb)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Impossible de lancer Blender : {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() || !stdout.contains("LUMEN_BLENDER_OK") {
        let detail = format!("{stdout}\n{stderr}");
        let clipped: String = detail.chars().rev().take(1800).collect::<String>().chars().rev().collect();
        return Err(format!(
            "Blender a échoué.\n{}",
            clipped.trim()
        ));
    }
    let bytes = fs::read(&out_glb).map_err(|e| format!("GLB introuvable après Blender : {e}"))?;
    if bytes.len() < 64 {
        return Err("Blender n’a pas produit de modèle".into());
    }

    let filename = format!("{name}.glb");
    let item = crate::bank::add_bytes_to_bank(&bytes, &filename, "mesh", "blender")?;
    let (mut item, _) = crate::publish::auto_publish(item);
    let dir = project.join("assets").join("meshes");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&filename);
    fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    item.path = dest.to_string_lossy().into();

    Ok(SavedAsset {
        kind: "mesh".into(),
        path: item.path,
        bank_id: Some(item.id),
        code: Some(item.code),
        roblox_asset_id: item.roblox_asset_id,
        publish_error: None,
        preview_path: item.preview_path,
        prompt: if title.trim().is_empty() {
            name
        } else {
            title.trim().into()
        },
        can_retry: false,
    })
}

#[tauri::command]
pub fn run_blender_mesh_cmd(
    project_path: String,
    script: Option<String>,
    script_path: Option<String>,
    title: Option<String>,
) -> Result<SavedAsset, String> {
    run_blender_mesh(
        &project_path,
        script.as_deref().unwrap_or(""),
        script_path.as_deref(),
        title.as_deref().unwrap_or("mesh"),
    )
}
