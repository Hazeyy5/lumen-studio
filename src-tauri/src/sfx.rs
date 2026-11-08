//! Bruitages générés (`assets/sfx/<Nom>.sfx.json` + `<Nom>.wav`) : onglet Sons de Lumen et `tools/lumen-sfx.mjs`.

use base64::Engine;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Outil des agents (même synthèse que l'onglet Sons).
pub fn write_sfx_kit(dir: &Path) -> Result<(), String> {
    let tools = dir.join("tools");
    fs::create_dir_all(&tools).map_err(|e| e.to_string())?;
    fs::write(tools.join("lumen-sfx-core.mjs"), include_str!("../resources/lumen-sfx-core.mjs"))
        .map_err(|e| e.to_string())?;
    fs::write(tools.join("lumen-sfx.mjs"), include_str!("../resources/lumen-sfx.mjs")).map_err(|e| e.to_string())?;
    fs::create_dir_all(sfx_dir(dir)).map_err(|e| e.to_string())?;
    Ok(())
}

fn sfx_dir(project: &Path) -> PathBuf {
    project.join("assets").join("sfx")
}

fn valid_name(name: &str) -> Result<&str, String> {
    let ok = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(name)
    } else {
        Err(format!("Nom invalide « {name} » : lettres, chiffres et _, en commençant par une lettre."))
    }
}

fn short_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn read_ids(project: &Path) -> serde_json::Map<String, serde_json::Value> {
    fs::read_to_string(sfx_dir(project).join("ids.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SfxFile {
    pub name: String,
    pub text: String,
    pub modified: u64,
    pub wav_path: Option<String>,
    /// rbxassetid://… si publié.
    pub asset_id: Option<String>,
    /// Le WAV a changé depuis la publication.
    pub stale: bool,
}

#[tauri::command(async)]
pub fn list_sfx(project_path: String) -> Result<Vec<SfxFile>, String> {
    let project = Path::new(&project_path);
    let dir = sfx_dir(project);
    let ids = read_ids(project);
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(out);
    };
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        let Some(name) = file.strip_suffix(".sfx.json") else {
            continue;
        };
        let text = fs::read_to_string(entry.path()).unwrap_or_default();
        let wav = dir.join(format!("{name}.wav"));
        let mut modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let wav_bytes = fs::read(&wav).ok();
        if let Some(t) = fs::metadata(&wav)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        {
            modified = modified.max(t.as_millis() as u64);
        }
        let entry_ids = ids.get(name);
        let asset_id = entry_ids
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let stale = match (&asset_id, &wav_bytes, entry_ids.and_then(|v| v.get("hash")).and_then(|v| v.as_str())) {
            (Some(_), Some(bytes), Some(h)) => short_hash(bytes) != h,
            _ => false,
        };
        out.push(SfxFile {
            name: name.to_string(),
            text,
            modified,
            wav_path: wav_bytes.map(|_| wav.to_string_lossy().into()),
            asset_id,
            stale,
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Enregistre un son généré dans l'onglet Sons (JSON + WAV rendu par l'app).
#[tauri::command(async)]
pub fn save_sfx(project_path: String, name: String, spec: String, wav_b64: String) -> Result<(), String> {
    let name = valid_name(name.trim())?;
    serde_json::from_str::<serde_json::Value>(&spec).map_err(|e| format!("JSON invalide : {e}"))?;
    let wav = base64::engine::general_purpose::STANDARD
        .decode(wav_b64.as_bytes())
        .map_err(|e| e.to_string())?;
    let dir = sfx_dir(Path::new(&project_path));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(format!("{name}.sfx.json")), spec).map_err(|e| e.to_string())?;
    fs::write(dir.join(format!("{name}.wav")), wav).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command(async)]
pub fn delete_sfx(project_path: String, name: String) -> Result<(), String> {
    let name = valid_name(name.trim())?;
    let dir = sfx_dir(Path::new(&project_path));
    let _ = fs::remove_file(dir.join(format!("{name}.sfx.json")));
    let _ = fs::remove_file(dir.join(format!("{name}.wav")));
    Ok(())
}

/// Publie le WAV sur Roblox (banque Lumen + Open Cloud) et note l'id dans assets/sfx/ids.json.
#[tauri::command(async)]
pub fn publish_sfx(project_path: String, name: String) -> Result<String, String> {
    let name = valid_name(name.trim())?;
    let project = Path::new(&project_path);
    let wav = sfx_dir(project).join(format!("{name}.wav"));
    let bytes = fs::read(&wav).map_err(|_| format!("Pas de WAV pour {name}"))?;
    let saved = crate::assets::upload_project_file(
        &project_path,
        &format!("assets/sfx/{name}.wav"),
        &format!("SFX {name}"),
    )?;
    let id = saved
        .roblox_asset_id
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| saved.publish_error.unwrap_or_else(|| "Roblox n'a pas renvoyé d'id".into()))?;
    let id = if id.starts_with("rbxassetid://") { id } else { format!("rbxassetid://{id}") };
    let mut ids = read_ids(project);
    ids.insert(name.to_string(), serde_json::json!({ "id": id, "hash": short_hash(&bytes) }));
    fs::write(
        sfx_dir(project).join("ids.json"),
        serde_json::to_string_pretty(&serde_json::Value::Object(ids)).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}
