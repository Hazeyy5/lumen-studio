use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TileSize {
    #[serde(default)]
    pub x_scale: f64,
    #[serde(default)]
    pub x_offset: f64,
    #[serde(default)]
    pub y_scale: f64,
    #[serde(default)]
    pub y_offset: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankItem {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub source: String,
    pub created_at: String,
    pub roblox_asset_id: Option<String>,
    #[serde(default)]
    pub preview_path: Option<String>,
    #[serde(default)]
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_size: Option<TileSize>,
}

fn bank_root() -> Result<PathBuf, String> {
    let dir = dirs::document_dir()
        .ok_or("Documents introuvable")?
        .join("Lumen")
        .join("bank");
    fs::create_dir_all(dir.join("files")).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn index_path() -> Result<PathBuf, String> {
    Ok(bank_root()?.join("index.json"))
}

fn load_index() -> Result<Vec<BankItem>, String> {
    let path = index_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut items: Vec<BankItem> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if ensure_codes(&mut items) {
        save_index(&items)?;
    }
    Ok(items)
}

fn next_code(items: &[BankItem], source: &str) -> String {
    if source.eq_ignore_ascii_case("inspiration") {
        let mut max = 0u32;
        for item in items {
            if let Some(n) = parse_ins_code(&item.code) {
                max = max.max(n);
            }
        }
        return format!("INS-{:04}", max + 1);
    }
    let mut max = 0u32;
    for item in items {
        if let Some(n) = parse_code(&item.code) {
            max = max.max(n);
        }
    }
    format!("LUM-{:04}", max + 1)
}

fn parse_ins_code(code: &str) -> Option<u32> {
    code.trim()
        .strip_prefix("INS-")
        .or_else(|| code.trim().strip_prefix("ins-"))
        .and_then(|s| s.parse().ok())
}

pub fn is_inspiration(item: &BankItem) -> bool {
    item.source.eq_ignore_ascii_case("inspiration")
        || parse_ins_code(&item.code).is_some()
}

pub fn is_texture(item: &BankItem) -> bool {
    item.source.eq_ignore_ascii_case("texture")
        || parse_tex_code(&item.code).is_some()
}

fn parse_tex_code(code: &str) -> Option<u32> {
    code.trim()
        .strip_prefix("TEX-")
        .or_else(|| code.trim().strip_prefix("tex-"))
        .and_then(|s| s.parse().ok())
}

fn parse_code(code: &str) -> Option<u32> {
    code.trim()
        .strip_prefix("LUM-")
        .or_else(|| code.trim().strip_prefix("lum-"))
        .and_then(|s| s.parse().ok())
}

fn ensure_codes(items: &mut [BankItem]) -> bool {
    let mut max = items.iter().filter_map(|i| parse_code(&i.code)).max().unwrap_or(0);
    let mut changed = false;
    for item in items.iter_mut() {
        if item.code.trim().is_empty() {
            max += 1;
            item.code = format!("LUM-{max:04}");
            changed = true;
        }
    }
    changed
}

fn save_index(items: &[BankItem]) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(items).map_err(|e| e.to_string())?;
    fs::write(index_path()?, raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_bank() -> Result<Vec<BankItem>, String> {
    let mut items = load_index()?;
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

pub fn search_library(
    query: &str,
    kind: Option<&str>,
    limit: usize,
) -> Result<Vec<BankItem>, String> {
    let limit = limit.clamp(1, 80);
    let kind = kind
        .map(|k| k.trim().to_ascii_lowercase())
        .filter(|k| !k.is_empty() && k != "all");
    let tokens: Vec<String> = query
        .split_whitespace()
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| {
            !t.is_empty()
                && t != "image"
                && t != "mesh"
                && t != "icon"
                && t != "icône"
                && t != "inspiration"
                && t != "inspire"
                && t != "texture"
                && t != "textures"
                && t != "tex"
        })
        .collect();
    let mut items = load_index()?;
    items.extend(crate::catalog::list_vibestarter()?);
    items.extend(crate::textures::list_textures()?);
    let q_lower = query.to_ascii_lowercase();
    let query_wants_insp = q_lower.split_whitespace().any(|t| {
        matches!(t, "inspiration" | "inspire" | "moodboard" | "mood")
    });
    let query_wants_tex = q_lower.split_whitespace().any(|t| {
        matches!(t, "texture" | "textures" | "tex")
    });
    if let Some(kind) = kind.as_deref() {
        if kind == "inspiration" || kind == "inspire" || kind == "mood" || kind == "ref" {
            items.retain(is_inspiration);
        } else if kind == "texture" || kind == "textures" || kind == "tex" {
            items.retain(is_texture);
        } else {
            let want = if kind == "icon" || kind == "icons" || kind == "icone" {
                "image"
            } else {
                kind
            };
            let also_insp = want == "image"
                && tokens.iter().any(|t| {
                    matches!(
                        t.as_str(),
                        "hud" | "ui" | "shop" | "boutique" | "menu" | "interface" | "gui"
                    )
                });
            items.retain(|item| {
                if also_insp && is_inspiration(item) {
                    return true;
                }
                item.kind.eq_ignore_ascii_case(want) && !is_inspiration(item)
            });
        }
    } else if query_wants_insp {
        items.retain(is_inspiration);
    } else if query_wants_tex {
        items.retain(is_texture);
    }
    if !tokens.is_empty() {
        let mut ranked: Vec<(i32, BankItem)> = items
            .into_iter()
            .filter_map(|item| {
                let score = library_score(&item, &tokens);
                (score > 0).then_some((score, item))
            })
            .collect();
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
        items = ranked.into_iter().map(|(_, item)| item).collect();
    } else {
        items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }
    items.truncate(limit);
    Ok(items)
}

fn library_score(item: &BankItem, tokens: &[String]) -> i32 {
    let name = item.name.to_ascii_lowercase();
    let code = item.code.to_ascii_lowercase();
    let path = item.path.to_ascii_lowercase();
    let source = item.source.to_ascii_lowercase();
    let mut score = 0;
    let mut hits = 0;
    for token in tokens {
        if code == *token || code.eq_ignore_ascii_case(token) {
            score += 120;
            hits += 1;
            continue;
        }
        if name == *token {
            score += 60;
            hits += 1;
            continue;
        }
        if name.contains(token) {
            score += 18;
            hits += 1;
            continue;
        }
        if path.contains(token)
            || code.contains(token)
            || source.contains(token)
            || item
                .roblox_asset_id
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase()
                .contains(token)
        {
            score += 6;
            hits += 1;
        }
    }
    if hits == 0 {
        return 0;
    }
    if hits == tokens.len() {
        score += 25;
    }
    if is_inspiration(item) {
        score += 8;
    }
    if is_texture(item) {
        score += 10;
    }
    score
}

pub fn import_to_bank(file_path: String, source: Option<String>) -> Result<BankItem, String> {
    let src = PathBuf::from(&file_path);
    if !src.exists() {
        return Err("Fichier introuvable".into());
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_lowercase();
    let kind = match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "bmp" => "image",
        "glb" | "gltf" | "fbx" | "obj" => "mesh",
        "ogg" | "mp3" | "wav" => "audio",
        _ => "file",
    };
    let source = source.unwrap_or_else(|| "import".into());
    if source.eq_ignore_ascii_case("inspiration") && kind != "image" {
        return Err("L’inspiration n’accepte que des images (png, jpg, webp)".into());
    }
    let id = Uuid::new_v4().to_string();
    let name = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("asset")
        .to_string();
    let dest = bank_root()?
        .join("files")
        .join(format!("{id}.{ext}"));
    fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    let mut items = load_index()?;
    let code = next_code(&items, &source);
    let item = BankItem {
        id,
        name,
        kind: kind.into(),
        path: dest.to_string_lossy().into(),
        source,
        created_at: now(),
        roblox_asset_id: None,
        preview_path: None,
        code,
        scale_type: None,
        tile_size: None,
    };
    items.push(item.clone());
    save_index(&items)?;
    Ok(item)
}

pub fn add_bytes_to_bank(
    bytes: &[u8],
    filename: &str,
    kind: &str,
    source: &str,
) -> Result<BankItem, String> {
    let id = Uuid::new_v4().to_string();
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let dest = bank_root()?
        .join("files")
        .join(format!("{id}.{ext}"));
    fs::write(&dest, bytes).map_err(|e| e.to_string())?;
    let mut items = load_index()?;
    let code = next_code(&items, source);
    let item = BankItem {
        id,
        name: Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("asset")
            .into(),
        kind: kind.into(),
        path: dest.to_string_lossy().into(),
        source: source.into(),
        created_at: now(),
        roblox_asset_id: None,
        preview_path: None,
        code,
        scale_type: None,
        tile_size: None,
    };
    items.push(item.clone());
    save_index(&items)?;
    Ok(item)
}

pub fn attach_preview(id: &str, preview_path: String) -> Result<(), String> {
    let mut items = load_index()?;
    if let Some(item) = items.iter_mut().find(|i| i.id == id || i.code.eq_ignore_ascii_case(id)) {
        item.preview_path = Some(preview_path);
        return save_index(&items);
    }
    crate::catalog::attach_preview(id, preview_path)
        .or_else(|_| crate::textures::get_item(id).map(|_| ()))
}

#[tauri::command]
pub fn save_mesh_preview(id: String, png_base64: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png_base64.trim())
        .map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("Aperçu vide".into());
    }
    let dir = bank_root()?.join("previews");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else {
                '_'
            }
        })
        .collect();
    let dest = dir.join(format!("{safe}.png"));
    fs::write(&dest, bytes).map_err(|e| e.to_string())?;
    let path = dest.to_string_lossy().into_owned();
    attach_preview(&id, path.clone())?;
    Ok(path)
}

#[tauri::command]
pub fn read_lumen_file(path: String) -> Result<String, String> {
    let requested = PathBuf::from(&path);
    let canon = requested.canonicalize().map_err(|e| e.to_string())?;
    if !file_in_banks(&canon) {
        return Err("Fichier hors des banques Lumen".into());
    }
    let bytes = fs::read(&canon).map_err(|e| e.to_string())?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

pub fn mark_published(id: &str, asset_id: &str) -> Result<(), String> {
    let mut items = load_index()?;
    if let Some(item) = items
        .iter_mut()
        .find(|i| i.id == id || i.code.eq_ignore_ascii_case(id))
    {
        item.roblox_asset_id = Some(asset_id.into());
        return save_index(&items);
    }
    crate::catalog::mark_published(id, asset_id)
        .or_else(|_| crate::textures::mark_published(id, asset_id))
}

pub fn copy_inspiration_into_project(
    item: &BankItem,
    project_path: &str,
) -> Result<(String, String), String> {
    if !is_inspiration(item) {
        return Err("Cet asset n’est pas une image d’inspiration".into());
    }
    let project = crate::assets::assert_lumen_project(project_path)?;
    let dir = project.join("assets").join("inspiration");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ext = Path::new(&item.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let safe: String = item
        .code
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let dest = dir.join(format!("{safe}.{ext}"));
    fs::copy(&item.path, &dest).map_err(|e| e.to_string())?;
    let relative = format!("assets/inspiration/{safe}.{ext}");
    Ok((dest.to_string_lossy().into_owned(), relative))
}

pub fn get_bank_item(id_or_code: &str) -> Result<BankItem, String> {
    let needle = id_or_code.trim();
    if needle.is_empty() {
        return Err("ID d’asset manquant".into());
    }
    if let Some(item) = load_index()?.into_iter().find(|item| {
        item.id.eq_ignore_ascii_case(needle) || item.code.eq_ignore_ascii_case(needle)
    }) {
        return Ok(item);
    }
    crate::catalog::get_item(needle)
        .or_else(|_| crate::textures::get_item(needle))
}

fn normalize_os_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_start_matches(r"\\?\")
        .to_lowercase()
}

fn file_in_banks(canon: &Path) -> bool {
    let n = normalize_os_path(canon);
    if let Some(doc) = dirs::document_dir() {
        let lumen = normalize_os_path(&doc.join("Lumen"));
        if !lumen.is_empty() && n.starts_with(&lumen) {
            return true;
        }
    }
    if n.contains(r"\assetsdownloader\vibestarter_assets") {
        return true;
    }
    if n.contains(r"\images\textures") {
        return true;
    }
    crate::catalog::is_under_vibestarter(canon) || crate::textures::is_under_textures(canon)
}

fn now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}
