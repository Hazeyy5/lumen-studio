use crate::bank::{BankItem, TileSize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, UNIX_EPOCH};

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
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scale_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tile_size: Option<TileSize>,
}

pub fn shared_textures_dir() -> Result<PathBuf, String> {
    let dir = dirs::document_dir()
        .ok_or("Documents introuvable")?
        .join("Lumen")
        .join("shared-textures");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn texture_dirs() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(pics) = dirs::picture_dir() {
        candidates.push(pics.join("textures"));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("OneDrive").join("Images").join("textures"));
        candidates.push(home.join("Pictures").join("textures"));
    }
    if let Some(docs) = dirs::document_dir() {
        candidates.push(docs.join("Lumen").join("shared-textures"));
    }
    candidates.into_iter().filter(|path| path.is_dir()).collect()
}

pub fn textures_root() -> Option<PathBuf> {
    texture_dirs().into_iter().next()
}

fn meta_path() -> Result<PathBuf, String> {
    let dir = dirs::document_dir()
        .ok_or("Documents introuvable")?
        .join("Lumen");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("catalog-textures.json"))
}

fn snapshot_path() -> Result<PathBuf, String> {
    let dir = dirs::document_dir()
        .ok_or("Documents introuvable")?
        .join("Lumen");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("catalog-textures.snapshot.json"))
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

fn cache() -> &'static Mutex<Option<Vec<BankItem>>> {
    static CACHE: OnceLock<Mutex<Option<Vec<BankItem>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn lock_cache() -> std::sync::MutexGuard<'static, Option<Vec<BankItem>>> {
    cache().lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Serialize, Deserialize)]
struct CatalogSnapshot {
    items: Vec<BankItem>,
}

fn load_snapshot() -> Option<Vec<BankItem>> {
    let raw = fs::read_to_string(snapshot_path().ok()?).ok()?;
    let snap: CatalogSnapshot = serde_json::from_str(&raw).ok()?;
    if snap.items.is_empty() {
        return None;
    }
    Some(snap.items)
}

fn save_snapshot(items: &[BankItem]) {
    if items.is_empty() {
        return;
    }
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
        let key = item.id.strip_prefix("tex:").unwrap_or(&item.id);
        if let Some(entry) = meta.items.get(key) {
            if !entry.code.is_empty() {
                item.code = entry.code.clone();
            }
            item.roblox_asset_id = entry.roblox_asset_id.clone();
            if !entry.name.trim().is_empty() {
                item.name = entry.name.clone();
            }
            item.scale_type = entry.scale_type.clone();
            item.tile_size = entry.tile_size.clone();
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

pub(crate) fn clear_cache() {
    *lock_cache() = None;
    if let Ok(path) = snapshot_path() {
        let _ = fs::remove_file(path);
    }
}

fn is_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
    )
}

fn collect_images(root: &Path, files: &mut Vec<PathBuf>, depth: u8) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            collect_images(&path, files, depth + 1);
        } else if is_image(&path) {
            files.push(path);
        }
    }
}

fn display_name(stem: &str) -> String {
    let cleaned = stem.replace("___", " ").replace('_', " ");
    let name = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        "texture".into()
    } else {
        name
    }
}

fn file_stamp(path: &Path) -> String {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|| "0".into())
}

fn next_tex_code(meta: &CatalogMeta) -> String {
    let mut max = 0u32;
    for entry in meta.items.values() {
        if let Some(n) = entry
            .code
            .trim()
            .strip_prefix("TEX-")
            .or_else(|| entry.code.trim().strip_prefix("tex-"))
            .and_then(|s| s.parse().ok())
        {
            max = max.max(n);
        }
    }
    format!("TEX-{:04}", max + 1)
}

fn thumbs_dir() -> Result<PathBuf, String> {
    let dir = dirs::document_dir()
        .ok_or("Documents introuvable")?
        .join("Lumen")
        .join("bank")
        .join("studio-thumbs");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn studio_item_from_entry(key: &str, entry: &CatalogEntry) -> BankItem {
    let id_part = key.strip_prefix("studio:").unwrap_or(key);
    let safe: String = id_part
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else {
                '_'
            }
        })
        .collect();
    let thumb = thumbs_dir()
        .ok()
        .map(|dir| dir.join(format!("{safe}.png")))
        .filter(|p| p.is_file());
    let preview = thumb.map(|p| p.to_string_lossy().into_owned());
    let name = if entry.name.trim().is_empty() {
        display_name(id_part)
    } else {
        entry.name.clone()
    };
    BankItem {
        id: format!("tex:{key}"),
        name,
        kind: "image".into(),
        path: preview.clone().unwrap_or_default(),
        source: "texture".into(),
        created_at: "0".into(),
        roblox_asset_id: entry.roblox_asset_id.clone(),
        preview_path: preview,
        code: entry.code.clone(),
        scale_type: entry.scale_type.clone(),
        tile_size: entry.tile_size.clone(),
        hash: String::new(),
        shared: false,
    }
}

fn scan_textures() -> Result<Vec<BankItem>, String> {
    let mut meta = load_meta()?;
    let mut changed = false;
    let mut items = Vec::new();

    let dirs = texture_dirs();
    let mut files = Vec::new();
    for root in &dirs {
        collect_images(root, &mut files, 0);
    }
    files.sort();
    files.dedup();
    for path in files {
        let key = dirs
            .iter()
            .find_map(|root| path.strip_prefix(root).ok())
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("texture")
                    .to_string()
            });
            if !meta.items.contains_key(&key) {
                let code = next_tex_code(&meta);
                meta.items.insert(
                    key.clone(),
                    CatalogEntry {
                        code,
                        roblox_asset_id: None,
                        name: String::new(),
                        ..Default::default()
                    },
                );
                changed = true;
            }
            let entry = meta.items.get(&key).cloned().unwrap_or_default();
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("texture");
            items.push(BankItem {
                id: format!("tex:{key}"),
                name: if entry.name.trim().is_empty() {
                    display_name(name)
                } else {
                    entry.name
                },
                kind: "image".into(),
                path: path.to_string_lossy().into(),
                source: "texture".into(),
                created_at: file_stamp(&path),
                roblox_asset_id: entry.roblox_asset_id,
                preview_path: Some(path.to_string_lossy().into()),
                code: entry.code,
                scale_type: None,
                tile_size: None,
                hash: String::new(),
                shared: false,
            });
    }

    for (key, entry) in &meta.items {
        if !key.starts_with("studio:") {
            continue;
        }
        let id = format!("tex:{key}");
        if items.iter().any(|item| item.id == id) {
            continue;
        }
        items.push(studio_item_from_entry(key, entry));
    }

    if changed {
        save_meta(&meta)?;
    }
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(items)
}

pub fn list_textures() -> Result<Vec<BankItem>, String> {
    list_textures_inner(false)
}

fn list_textures_inner(force: bool) -> Result<Vec<BankItem>, String> {
    if !force {
        if let Some(items) = lock_cache().as_ref() {
            if !items.is_empty() {
                return Ok(items.clone());
            }
        }
        if let Some(mut items) = load_snapshot() {
            if let Ok(meta) = load_meta() {
                overlay_meta(&mut items, &meta);
            }
            *lock_cache() = Some(items.clone());
            return Ok(items);
        }
    }
    let items = scan_textures()?;
    *lock_cache() = Some(items.clone());
    save_snapshot(&items);
    Ok(items)
}

#[tauri::command(async)]
pub fn list_textures_bank(force: Option<bool>) -> Result<Vec<BankItem>, String> {
    let force = force.unwrap_or(false);
    if force {
        clear_cache();
    }
    list_textures_inner(force)
}

pub fn merge_catalog_json(bytes: &[u8]) -> Result<(), String> {
    let incoming: CatalogMeta = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let mut meta = load_meta()?;
    for (key, entry) in incoming.items {
        meta.items.entry(key).or_insert(entry);
    }
    save_meta(&meta)?;
    clear_cache();
    Ok(())
}

pub fn get_item(id_or_code: &str) -> Result<BankItem, String> {
    let needle = id_or_code.trim();
    list_textures()?
        .into_iter()
        .find(|item| item.id.eq_ignore_ascii_case(needle) || item.code.eq_ignore_ascii_case(needle))
        .ok_or_else(|| format!("Texture {needle} introuvable"))
}

pub fn mark_published(id_or_code: &str, asset_id: &str) -> Result<(), String> {
    let item = get_item(id_or_code)?;
    let key = item
        .id
        .strip_prefix("tex:")
        .unwrap_or(&item.id)
        .to_string();
    let mut meta = load_meta()?;
    meta.items
        .entry(key)
        .or_insert_with(|| CatalogEntry {
            code: item.code.clone(),
            roblox_asset_id: None,
            name: item.name.clone(),
            ..Default::default()
        })
        .roblox_asset_id = Some(asset_id.into());
    save_meta(&meta)?;
    patch_cache(id_or_code, |row| {
        row.roblox_asset_id = Some(asset_id.into());
    });
    Ok(())
}

pub fn is_under_textures(canon: &Path) -> bool {
    match textures_root().and_then(|p| p.canonicalize().ok()) {
        Some(root) => canon.starts_with(root),
        None => false,
    }
}

fn want_scan() -> &'static AtomicBool {
    static FLAG: OnceLock<AtomicBool> = OnceLock::new();
    FLAG.get_or_init(|| AtomicBool::new(false))
}

fn last_scan_count() -> &'static Mutex<Option<u32>> {
    static LAST: OnceLock<Mutex<Option<u32>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(None))
}

pub fn want_studio_scan() -> bool {
    want_scan().load(Ordering::SeqCst)
}

pub fn begin_studio_scan() {
    if let Ok(mut last) = last_scan_count().lock() {
        *last = None;
    }
    want_scan().store(true, Ordering::SeqCst);
}

pub fn cancel_studio_scan() {
    want_scan().store(false, Ordering::SeqCst);
}

pub fn take_studio_scan_result() -> Option<u32> {
    last_scan_count().lock().ok()?.take()
}

pub fn normalize_texture_content(raw: &str) -> Option<(String, String)> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if s.chars().all(|c| c.is_ascii_digit()) && s.len() >= 4 {
        return Some((format!("studio:{s}"), s.to_string()));
    }
    let lower = s.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("rbxassetid://") {
        let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if id.len() >= 4 {
            return Some((format!("studio:{id}"), id));
        }
    }
    if let Some(idx) = lower.find("id=") {
        let id: String = lower[idx + 3..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if id.len() >= 4 {
            return Some((format!("studio:{id}"), id));
        }
    }
    if lower.starts_with("rbxasset://") {
        let path = lower.replace('\\', "/");
        return Some((format!("studio:{path}"), s.to_string()));
    }
    None
}

fn fetch_thumb(asset_id: &str) -> Option<PathBuf> {
    if !asset_id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let dest = thumbs_dir().ok()?.join(format!("{asset_id}.png"));
    if dest.is_file() {
        return Some(dest);
    }
    let url = format!(
        "https://thumbnails.roblox.com/v1/assets?assetIds={asset_id}&returnPolicy=PlaceHolder&size=420x420&format=Png"
    );
    let json: serde_json::Value = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .ok()?
        .get(url)
        .send()
        .ok()?
        .json()
        .ok()?;
    let image = json.pointer("/data/0/imageUrl")?.as_str()?;
    let bytes = reqwest::blocking::get(image).ok()?.bytes().ok()?;
    if bytes.len() < 32 {
        return None;
    }
    fs::write(&dest, bytes).ok()?;
    Some(dest)
}

fn json_num(obj: &serde_json::Map<String, serde_json::Value>, names: &[&str]) -> Option<f64> {
    for name in names {
        let Some(value) = obj.get(*name) else {
            continue;
        };
        if let Some(n) = value.as_f64() {
            return Some(n);
        }
        if let Some(n) = value.as_i64() {
            return Some(n as f64);
        }
        if let Some(s) = value.as_str() {
            if let Ok(n) = s.parse::<f64>() {
                return Some(n);
            }
        }
    }
    None
}

fn axis_udim(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> (f64, f64) {
    if let Some(axis) = obj.get(key).and_then(|v| v.as_object()) {
        return (
            json_num(axis, &["scale"]).unwrap_or(0.0),
            json_num(axis, &["offset"]).unwrap_or(0.0),
        );
    }
    (0.0, 0.0)
}

fn parse_tile_size(value: &serde_json::Value) -> Option<TileSize> {
    let obj = value.as_object()?;
    let (x_from_axis, x_off_axis) = axis_udim(obj, "x");
    let (y_from_axis, y_off_axis) = axis_udim(obj, "y");
    Some(TileSize {
        x_scale: json_num(obj, &["xScale", "x_scale"]).unwrap_or(x_from_axis),
        x_offset: json_num(obj, &["xOffset", "x_offset"]).unwrap_or(x_off_axis),
        y_scale: json_num(obj, &["yScale", "y_scale"]).unwrap_or(y_from_axis),
        y_offset: json_num(obj, &["yOffset", "y_offset"]).unwrap_or(y_off_axis),
    })
}

fn normalize_scale_type(raw: Option<&str>) -> Option<String> {
    match raw?.trim().to_ascii_lowercase().as_str() {
        "stretch" => Some("Stretch".into()),
        "tile" => Some("Tile".into()),
        "fit" => Some("Fit".into()),
        "crop" => Some("Crop".into()),
        "slice" => Some("Slice".into()),
        "" => None,
        _ => None,
    }
}

fn tiling_from_row(row: &serde_json::Value) -> (Option<String>, Option<TileSize>) {
    let scale = row
        .get("scaleType")
        .or_else(|| row.get("scale_type"))
        .and_then(|v| v.as_str())
        .and_then(|s| normalize_scale_type(Some(s)));
    let tile = row
        .get("tileSize")
        .or_else(|| row.get("tile_size"))
        .and_then(parse_tile_size);
    (scale, tile)
}

fn apply_tiling(entry: &mut CatalogEntry, scale: Option<String>, tile: Option<TileSize>, overwrite: bool) {
    if overwrite || entry.scale_type.is_none() {
        if scale.is_some() {
            entry.scale_type = scale;
        }
    }
    if overwrite || entry.tile_size.is_none() {
        if tile.is_some() {
            entry.tile_size = tile;
        }
    }
}

fn upsert_studio_texture(
    name: &str,
    content: &str,
    meta: &mut CatalogMeta,
    scale_type: Option<String>,
    tile_size: Option<TileSize>,
) -> bool {
    let Some((key, stored_id)) = normalize_texture_content(content) else {
        return false;
    };
    if meta.items.contains_key(&key) {
        if let Some(entry) = meta.items.get_mut(&key) {
            if entry.name.trim().is_empty() && !name.trim().is_empty() {
                entry.name = name.trim().into();
            }
            if entry.roblox_asset_id.is_none() {
                entry.roblox_asset_id = Some(stored_id);
            }
            apply_tiling(entry, scale_type, tile_size, false);
        }
        return false;
    }
    let code = next_tex_code(meta);
    let label = if name.trim().is_empty() {
        display_name(key.strip_prefix("studio:").unwrap_or(&key))
    } else {
        name.trim().into()
    };
    if stored_id.chars().all(|c| c.is_ascii_digit()) {
        let _ = fetch_thumb(&stored_id);
    }
    meta.items.insert(
        key,
        CatalogEntry {
            code,
            roblox_asset_id: Some(stored_id),
            name: label,
            scale_type,
            tile_size,
        },
    );
    true
}

pub fn ingest_studio_textures(payload: &serde_json::Value) -> u32 {
    let rows = payload
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut meta = load_meta().unwrap_or_default();
    let mut added = 0u32;
    for row in rows {
        let content = row
            .get("content")
            .or_else(|| row.get("id"))
            .or_else(|| row.get("texture"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let name = row
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (scale, tile) = tiling_from_row(&row);
        if upsert_studio_texture(name, content, &mut meta, scale, tile) {
            added += 1;
        }
    }
    let _ = save_meta(&meta);
    clear_cache();
    want_scan().store(false, Ordering::SeqCst);
    if let Ok(mut last) = last_scan_count().lock() {
        *last = Some(added);
    }
    added
}

#[tauri::command]
pub fn add_studio_texture(name: String, content: String) -> Result<BankItem, String> {
    let mut meta = load_meta()?;
    if !upsert_studio_texture(&name, &content, &mut meta, None, None) {
        let (key, _) = normalize_texture_content(&content)
            .ok_or("ID de texture invalide (rbxassetid://… ou nombre)")?;
        save_meta(&meta)?;
        clear_cache();
        return get_item(&format!("tex:{key}"));
    }
    save_meta(&meta)?;
    clear_cache();
    let (key, _) = normalize_texture_content(&content)
        .ok_or("ID de texture invalide")?;
    get_item(&format!("tex:{key}"))
}

#[tauri::command]
pub fn set_texture_tiling(
    id: String,
    scale_type: Option<String>,
    tile_size: Option<TileSize>,
) -> Result<BankItem, String> {
    let item = get_item(&id)?;
    if !item.id.contains("studio:") {
        return Err("ScaleType / TileSize uniquement pour les textures importées de Studio".into());
    }
    let scale = normalize_scale_type(scale_type.as_deref())
        .ok_or("ScaleType invalide (Stretch, Fit, Crop, Tile, Slice)")?;
    let key = item
        .id
        .strip_prefix("tex:")
        .unwrap_or(&item.id)
        .to_string();
    let mut meta = load_meta()?;
    let entry = meta.items.entry(key).or_insert_with(|| CatalogEntry {
        code: item.code.clone(),
        roblox_asset_id: item.roblox_asset_id.clone(),
        name: item.name.clone(),
        ..Default::default()
    });
    apply_tiling(entry, Some(scale.clone()), tile_size.clone(), true);
    save_meta(&meta)?;
    patch_cache(&id, |row| {
        row.scale_type = Some(scale.clone());
        row.tile_size = tile_size.clone();
    });
    get_item(&id)
}

#[tauri::command]
pub fn import_studio_textures() -> Result<u32, String> {
    begin_studio_scan();
    for _ in 0..25 {
        std::thread::sleep(Duration::from_millis(400));
        if let Some(n) = take_studio_scan_result() {
            return Ok(n);
        }
    }
    cancel_studio_scan();
    Err("Studio n’a pas envoyé de textures. Ouvre la place dans Roblox Studio (plugin Lumen).".into())
}
