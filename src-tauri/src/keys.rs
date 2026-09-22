use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Keys {
    pub gemini: String,
    pub meshy: String,
    pub tripo: String,
    pub cursor: String,
    #[serde(default = "default_mesh_provider")]
    pub mesh_provider: String,
    #[serde(default)]
    pub roblox_api_key: String,
    #[serde(default)]
    pub roblox_user_id: String,
    #[serde(default)]
    pub roblox_oauth_client_id: String,
    #[serde(default)]
    pub blender_path: String,
    #[serde(default)]
    pub vibe_assets_path: String,
    #[serde(default = "default_share_bank")]
    pub share_bank: bool,
    #[serde(default)]
    pub bank_sync_token: String,
}

fn default_share_bank() -> bool {
    true
}

fn default_mesh_provider() -> String {
    "meshy".into()
}

fn keys_path() -> Result<PathBuf, String> {
    let dir = dirs::data_dir()
        .ok_or("Impossible de trouver le dossier AppData")?
        .join("Lumen");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("keys.json"))
}

pub fn load_keys() -> Result<Keys, String> {
    let path = keys_path()?;
    if !path.exists() {
        return Ok(Keys {
            mesh_provider: "meshy".into(),
            share_bank: true,
            ..Keys::default()
        });
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn save_keys(keys: Keys) -> Result<(), String> {
    let path = keys_path()?;
    let raw = serde_json::to_string_pretty(&keys).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_keys() -> Result<Keys, String> {
    load_keys()
}

#[tauri::command]
pub fn set_keys(keys: Keys) -> Result<(), String> {
    save_keys(keys)?;
    crate::catalog::clear_catalog_cache();
    Ok(())
}
