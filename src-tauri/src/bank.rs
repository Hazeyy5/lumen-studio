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
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub shared: bool,
}

pub(crate) fn bank_root() -> Result<PathBuf, String> {
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

pub(crate) fn load_index() -> Result<Vec<BankItem>, String> {
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

pub(crate) fn save_index(items: &[BankItem]) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(items).map_err(|e| e.to_string())?;
    fs::write(index_path()?, raw).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn list_bank() -> Result<Vec<BankItem>, String> {
    let mut items = load_index()?;
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankCounts {
    pub lumen: usize,
    pub inspiration: usize,
    pub vibe: usize,
    pub vibe_images: usize,
    pub vibe_meshes: usize,
    pub textures: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankPage {
    pub total: usize,
    pub items: Vec<BankItem>,
    pub recent: Vec<BankItem>,
}

fn matches_query(item: &BankItem, query: &str, kind: &str) -> bool {
    if !kind.is_empty() && kind != "all" && item.kind != kind {
        return false;
    }
    if query.is_empty() {
        return true;
    }
    item.name.to_ascii_lowercase().contains(query)
        || item.code.to_ascii_lowercase().contains(query)
        || item
            .roblox_asset_id
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase()
            .contains(query)
}

fn page_of(items: &[BankItem], query: &str, kind: &str, offset: usize, limit: usize) -> (usize, Vec<BankItem>) {
    let mut total = 0usize;
    let mut page = Vec::new();
    for item in items {
        if !matches_query(item, query, kind) {
            continue;
        }
        if total >= offset && page.len() < limit {
            page.push(item.clone());
        }
        total += 1;
    }
    (total, page)
}

#[tauri::command(async)]
pub fn bank_counts(force: Option<bool>) -> Result<BankCounts, String> {
    let force = force.unwrap_or(false);
    let index = load_index()?;
    let inspiration = index.iter().filter(|item| is_inspiration(item)).count();
    let (vibe, vibe_images, vibe_meshes) = crate::catalog::with_vibestarter(force, |items| {
        let images = items.iter().filter(|item| item.kind == "image").count();
        let meshes = items.iter().filter(|item| item.kind == "mesh").count();
        (items.len(), images, meshes)
    })?;
    if force {
        crate::textures::clear_cache();
    }
    let textures = crate::textures::list_textures().map(|items| items.len()).unwrap_or(0);
    Ok(BankCounts {
        lumen: index.len().saturating_sub(inspiration),
        inspiration,
        vibe,
        vibe_images,
        vibe_meshes,
        textures,
    })
}

#[tauri::command(async)]
pub fn bank_page(
    shelf: String,
    kind: Option<String>,
    query: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<BankPage, String> {
    let kind = kind.unwrap_or_default().trim().to_ascii_lowercase();
    let query = query.unwrap_or_default().trim().to_ascii_lowercase();
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(48).clamp(1, 60);
    let shelf = shelf.trim().to_ascii_lowercase();
    if shelf == "vibestarter" {
        return crate::catalog::with_vibestarter(false, |items| {
            let (total, page) = page_of(items, &query, &kind, offset, limit);
            let recent = if query.is_empty() {
                items.iter().take(12).cloned().collect()
            } else {
                Vec::new()
            };
            BankPage {
                total,
                items: page,
                recent,
            }
        });
    }
    let source = if shelf == "textures" {
        crate::textures::list_textures().unwrap_or_default()
    } else {
        let mut items = load_index()?;
        if shelf == "inspiration" {
            items.retain(is_inspiration);
        } else {
            items.retain(|item| !is_inspiration(item));
        }
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        items
    };
    let (total, page) = page_of(&source, &query, &kind, offset, limit);
    Ok(BankPage {
        total,
        items: page,
        recent: Vec::new(),
    })
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
    let hash = sha256_file(&dest)?;
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
        hash,
        shared: false,
    };
    items.push(item.clone());
    save_index(&items)?;
    crate::sync::queue_push(&item);
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
        hash: sha256_hex(bytes),
        shared: false,
    };
    items.push(item.clone());
    save_index(&items)?;
    crate::sync::queue_push(&item);
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

#[tauri::command]
pub fn export_lumen_bank(dest: String) -> Result<u32, String> {
    let root = bank_root()?;
    let dest_path = PathBuf::from(&dest);
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = fs::File::create(&dest_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut count = 0u32;
    let index = root.join("index.json");
    if index.is_file() {
        zip.start_file("index.json", opts)
            .map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut zip, &fs::read(&index).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    }
    let files = root.join("files");
    if files.is_dir() {
        count += zip_plain_dir(&mut zip, opts, &files, "files")?;
    }
    let previews = root.join("previews");
    if previews.is_dir() {
        zip_plain_dir(&mut zip, opts, &previews, "previews")?;
    }
    if let Some(dir) = dirs::document_dir() {
        let tex = dir.join("Lumen").join("catalog-textures.json");
        if tex.is_file() {
            zip.start_file("catalog-textures.json", opts)
                .map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut zip, &fs::read(&tex).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(count)
}

#[tauri::command]
pub fn import_lumen_bank(source: String) -> Result<u32, String> {
    let src = PathBuf::from(&source);
    if !src.is_file() {
        return Err("Fichier zip introuvable".into());
    }
    let file = fs::File::open(&src).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let root = bank_root()?;
    let files_dir = root.join("files");
    let previews_dir = root.join("previews");
    fs::create_dir_all(&files_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&previews_dir).map_err(|e| e.to_string())?;
    let mut incoming: Vec<BankItem> = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().replace('\\', "/");
        if name.contains("..") {
            continue;
        }
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).map_err(|e| e.to_string())?;
        if name == "index.json" {
            incoming = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        } else if let Some(file_name) = name.strip_prefix("files/") {
            if file_name.is_empty() || file_name.contains('/') {
                continue;
            }
            let dest = files_dir.join(file_name);
            if !dest.exists() {
                fs::write(dest, &bytes).map_err(|e| e.to_string())?;
            }
        } else if let Some(file_name) = name.strip_prefix("previews/") {
            if file_name.is_empty() || file_name.contains('/') {
                continue;
            }
            let dest = previews_dir.join(file_name);
            if !dest.exists() {
                fs::write(dest, &bytes).map_err(|e| e.to_string())?;
            }
        } else if name == "catalog-textures.json" {
            crate::textures::merge_catalog_json(&bytes)?;
        }
    }
    let mut items = load_index()?;
    let existing: std::collections::HashSet<String> = items
        .iter()
        .map(|item| item.code.to_ascii_lowercase())
        .collect();
    let mut added = 0u32;
    for mut item in incoming {
        if existing.contains(&item.code.to_ascii_lowercase()) {
            continue;
        }
        let file_name = Path::new(&item.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if !file_name.is_empty() {
            let local = files_dir.join(file_name);
            if local.is_file() {
                item.path = local.to_string_lossy().into();
            }
        }
        if let Some(prev) = item.preview_path.as_ref() {
            let preview_name = Path::new(prev)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if !preview_name.is_empty() {
                let local = previews_dir.join(preview_name);
                if local.is_file() {
                    item.preview_path = Some(local.to_string_lossy().into());
                }
            }
        }
        crate::sync::queue_push(&item);
        items.push(item);
        added += 1;
    }
    save_index(&items)?;
    Ok(added)
}

fn zip_plain_dir(
    zip: &mut zip::ZipWriter<fs::File>,
    opts: zip::write::SimpleFileOptions,
    dir: &Path,
    prefix: &str,
) -> Result<u32, String> {
    let mut count = 0u32;
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("asset.bin");
        zip.start_file(format!("{prefix}/{name}"), opts)
            .map_err(|e| e.to_string())?;
        std::io::Write::write_all(zip, &fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
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

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(crate) fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok(sha256_hex(&bytes))
}

pub(crate) fn ingest_shared(mut item: BankItem) -> Result<bool, String> {
    let mut items = load_index()?;
    if !item.hash.trim().is_empty()
        && items.iter().any(|row| !row.hash.is_empty() && row.hash == item.hash)
    {
        return Ok(false);
    }
    if items
        .iter()
        .any(|row| row.id == item.id || row.code.eq_ignore_ascii_case(&item.code))
    {
        return Ok(false);
    }
    item.shared = true;
    items.push(item);
    save_index(&items)?;
    Ok(true)
}

pub(crate) fn mark_shared(id: &str) -> Result<(), String> {
    let mut items = load_index()?;
    if let Some(item) = items.iter_mut().find(|row| row.id == id) {
        item.shared = true;
        save_index(&items)?;
    }
    Ok(())
}
