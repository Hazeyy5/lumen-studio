use crate::bank::BankItem;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CatalogMeta {
    #[serde(default)]
    items: HashMap<String, CatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CatalogEntry {
    code: String,
    #[serde(default)]
    roblox_asset_id: Option<String>,
    #[serde(default)]
    preview_path: Option<String>,
}

pub fn vibestarter_root() -> Option<PathBuf> {
    if let Ok(keys) = crate::keys::load_keys() {
        let custom = keys.vibe_assets_path.trim();
        if !custom.is_empty() {
            let path = PathBuf::from(custom);
            if path.is_dir() {
                return Some(path);
            }
        }
    }
    let mut candidates = Vec::new();
    if let Some(docs) = dirs::document_dir() {
        candidates.push(docs.join("AssetsDownloader").join("vibestarter_assets"));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(
            home.join("OneDrive")
                .join("Documents")
                .join("AssetsDownloader")
                .join("vibestarter_assets"),
        );
    }
    candidates.into_iter().find(|p| p.exists())
}

fn meta_path() -> Result<PathBuf, String> {
    let dir = dirs::document_dir()
        .ok_or("Documents introuvable")?
        .join("Lumen");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("catalog-vibestarter.json"))
}

fn load_meta() -> Result<CatalogMeta, String> {
    let path = meta_path()?;
    if !path.exists() {
        return Ok(CatalogMeta::default());
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_meta(meta: &CatalogMeta) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    fs::write(meta_path()?, raw).map_err(|e| e.to_string())
}

fn next_vs_code(meta: &CatalogMeta) -> String {
    let mut max = 0u32;
    for entry in meta.items.values() {
        if let Some(n) = entry
            .code
            .trim()
            .strip_prefix("VS-")
            .or_else(|| entry.code.trim().strip_prefix("vs-"))
            .and_then(|s| s.parse().ok())
        {
            max = max.max(n);
        }
    }
    format!("VS-{:04}", max + 1)
}

fn display_name(stem: &str) -> String {
    let cleaned = stem.replace("___", " ").replace('_', " ");
    let name = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        "asset".into()
    } else {
        name
    }
}

fn file_kind(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" => Some("image"),
        "glb" | "gltf" | "fbx" | "obj" => Some("mesh"),
        _ => None,
    }
}

fn collect_files(root: &Path) -> Vec<(PathBuf, &'static str)> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(true) {
                continue;
            }
            if let Some(kind) = file_kind(&path) {
                files.push((path, kind));
            }
        }
    }
    let icons = root.join("vibestarter_icons");
    if let Ok(entries) = fs::read_dir(icons) {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(true) {
                continue;
            }
            if file_kind(&path) == Some("image") {
                files.push((path, "image"));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn file_stamp(path: &Path) -> String {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|| "0".into())
}

fn catalog_key(kind: &str, path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("asset");
    format!("{kind}:{name}")
}

fn cache() -> &'static Mutex<Option<Vec<BankItem>>> {
    static CACHE: OnceLock<Mutex<Option<Vec<BankItem>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn lock_cache() -> std::sync::MutexGuard<'static, Option<Vec<BankItem>>> {
    cache().lock().unwrap_or_else(|e| e.into_inner())
}

fn snapshot_path() -> Result<PathBuf, String> {
    let dir = dirs::document_dir()
        .ok_or("Documents introuvable")?
        .join("Lumen");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("catalog-vibestarter.snapshot.json"))
}

#[derive(Serialize, Deserialize)]
struct CatalogSnapshot {
    items: Vec<BankItem>,
}

fn load_snapshot() -> Option<Vec<BankItem>> {
    let path = snapshot_path().ok()?;
    let raw = fs::read_to_string(&path).ok()?;
    let snap: CatalogSnapshot = serde_json::from_str(&raw).ok()?;
    if snap
        .items
        .first()
        .is_some_and(|item| item.path.starts_with("https://"))
    {
        let fresh = fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|time| time.elapsed().ok())
            .is_some_and(|age| age.as_secs() < 600);
        if !fresh {
            return None;
        }
    }
    if snap.items.is_empty() {
        return None;
    }
    Some(snap.items)
}

fn save_snapshot(items: &[BankItem]) {
    if let Ok(path) = snapshot_path() {
        if let Ok(raw) = serde_json::to_string(&CatalogSnapshot {
            items: items.to_vec(),
        }) {
            let _ = fs::write(path, raw);
        }
    }
}

fn overlay_meta(items: &mut [BankItem], meta: &CatalogMeta) {
    for item in items {
        let key = item.id.strip_prefix("vs:").unwrap_or(&item.id);
        if let Some(entry) = meta.items.get(key) {
            if !entry.code.is_empty() {
                item.code = entry.code.clone();
            }
            item.roblox_asset_id = entry.roblox_asset_id.clone();
            item.preview_path = entry
                .preview_path
                .clone()
                .filter(|p| Path::new(p).is_file());
        }
    }
}

fn patch_cache(id_or_code: &str, patch: impl FnOnce(&mut BankItem)) {
    let needle = id_or_code.trim();
    let mut guard = lock_cache();
    if let Some(items) = guard.as_mut() {
        if let Some(item) = items.iter_mut().find(|item| {
            item.id.eq_ignore_ascii_case(needle) || item.code.eq_ignore_ascii_case(needle)
        }) {
            patch(item);
        }
    }
}

pub fn clear_catalog_cache() {
    *lock_cache() = None;
    if let Ok(path) = snapshot_path() {
        let _ = fs::remove_file(path);
    }
}

fn scan_vibestarter() -> Result<Vec<BankItem>, String> {
    let Some(root) = vibestarter_root() else {
        return Ok(Vec::new());
    };
    let mut meta = load_meta()?;
    let mut changed = false;
    let mut items = Vec::new();

    for (path, kind) in collect_files(&root) {
        let key = catalog_key(kind, &path);
        if !meta.items.contains_key(&key) {
            let code = next_vs_code(&meta);
            meta.items.insert(
                key.clone(),
                CatalogEntry {
                    code,
                    roblox_asset_id: None,
                    preview_path: None,
                },
            );
            changed = true;
        }
        let entry = meta.items.get(&key).cloned().unwrap_or_default();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("asset");
        items.push(BankItem {
            id: format!("vs:{key}"),
            name: display_name(name),
            kind: kind.into(),
            path: path.to_string_lossy().into(),
            source: "vibestarter".into(),
            created_at: file_stamp(&path),
            roblox_asset_id: entry.roblox_asset_id,
            preview_path: entry
                .preview_path
                .filter(|p| Path::new(p).is_file()),
            code: entry.code,
            scale_type: None,
            tile_size: None,
            hash: String::new(),
            shared: false,
        });
    }

    if changed {
        save_meta(&meta)?;
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| a.name.cmp(&b.name)));
    Ok(items)
}

pub fn list_vibestarter() -> Result<Vec<BankItem>, String> {
    list_vibestarter_inner(false)
}

fn ensure_vibestarter(force: bool) -> Result<(), String> {
    if force {
        clear_catalog_cache();
    } else if lock_cache().is_some() {
        return Ok(());
    } else if let Some(mut items) = load_snapshot() {
        if let Ok(meta) = load_meta() {
            overlay_meta(&mut items, &meta);
        }
        *lock_cache() = Some(items);
        return Ok(());
    }
    let mut items = scan_vibestarter()?;
    if items.is_empty() {
        if let Ok(remote) = fetch_remote_vibestarter() {
            if !remote.is_empty() {
                items = remote;
            }
        }
    }
    if items.is_empty() {
        return Ok(());
    }
    save_snapshot(&items);
    *lock_cache() = Some(items);
    Ok(())
}

const REMOTE_VIBE_BASE: &str = "https://lumen-vibestarter.contact-delaplacetheo.workers.dev";

#[derive(Deserialize)]
struct RemoteVibeEntry {
    path: String,
    kind: String,
    name: String,
}

fn fetch_remote_vibestarter() -> Result<Vec<BankItem>, String> {
    if REMOTE_VIBE_BASE.contains("REPLACE") {
        return Ok(Vec::new());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent("Lumen/0.1")
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(format!("{REMOTE_VIBE_BASE}/manifest.json"))
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Catalogue distant: HTTP {}", response.status()));
    }
    let entries: Vec<RemoteVibeEntry> = response.json().map_err(|e| e.to_string())?;
    let mut items = Vec::with_capacity(entries.len());
    for entry in entries {
        let rel = entry.path.replace('\\', "/");
        if rel.contains("..") || rel.is_empty() {
            continue;
        }
        let encoded = rel
            .split('/')
            .map(|part| urlencoding::encode(part).into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let url = format!("{REMOTE_VIBE_BASE}/vibe/{encoded}");
        let kind = if entry.kind == "mesh" { "mesh" } else { "image" };
        items.push(BankItem {
            id: format!("vs:{kind}:{rel}"),
            name: if entry.name.trim().is_empty() {
                display_name(rel.rsplit('/').next().unwrap_or("asset"))
            } else {
                entry.name
            },
            kind: kind.into(),
            path: url.clone(),
            source: "vibestarter".into(),
            created_at: "0".into(),
            roblox_asset_id: None,
            preview_path: if kind == "image" { Some(url.clone()) } else { None },
            code: String::new(),
            scale_type: None,
            tile_size: None,
            hash: String::new(),
            shared: true,
        });
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

pub fn with_vibestarter<R>(force: bool, f: impl FnOnce(&[BankItem]) -> R) -> Result<R, String> {
    ensure_vibestarter(force)?;
    let guard = lock_cache();
    Ok(f(guard.as_deref().unwrap_or(&[])))
}

fn list_vibestarter_inner(force: bool) -> Result<Vec<BankItem>, String> {
    with_vibestarter(force, |items| items.to_vec())
}

#[tauri::command]
pub fn list_vibestarter_bank(force: Option<bool>) -> Result<Vec<BankItem>, String> {
    let force = force.unwrap_or(false);
    if force {
        clear_catalog_cache();
    }
    list_vibestarter_inner(force)
}

pub fn get_item(id_or_code: &str) -> Result<BankItem, String> {
    let needle = id_or_code.trim();
    list_vibestarter()?
        .into_iter()
        .find(|item| item.id.eq_ignore_ascii_case(needle) || item.code.eq_ignore_ascii_case(needle))
        .ok_or_else(|| format!("Asset {needle} introuvable dans VibeStarter"))
}

pub fn mark_published(id_or_code: &str, asset_id: &str) -> Result<(), String> {
    let item = get_item(id_or_code)?;
    let key = item
        .id
        .strip_prefix("vs:")
        .unwrap_or(&item.id)
        .to_string();
    let mut meta = load_meta()?;
    meta.items
        .entry(key)
        .or_insert_with(|| CatalogEntry {
            code: item.code.clone(),
            roblox_asset_id: None,
            preview_path: None,
        })
        .roblox_asset_id = Some(asset_id.into());
    save_meta(&meta)?;
    patch_cache(id_or_code, |item| {
        item.roblox_asset_id = Some(asset_id.into());
    });
    Ok(())
}

pub fn attach_preview(id_or_code: &str, preview_path: String) -> Result<(), String> {
    let item = get_item(id_or_code)?;
    let key = item
        .id
        .strip_prefix("vs:")
        .unwrap_or(&item.id)
        .to_string();
    let mut meta = load_meta()?;
    let entry = meta.items.entry(key).or_insert_with(|| CatalogEntry {
        code: item.code.clone(),
        roblox_asset_id: item.roblox_asset_id.clone(),
        preview_path: None,
    });
    entry.preview_path = Some(preview_path.clone());
    save_meta(&meta)?;
    patch_cache(id_or_code, |item| {
        item.preview_path = Some(preview_path);
    });
    Ok(())
}

pub fn is_under_vibestarter(canon: &Path) -> bool {
    match vibestarter_root().and_then(|p| p.canonicalize().ok()) {
        Some(root) => canon.starts_with(root),
        None => false,
    }
}
