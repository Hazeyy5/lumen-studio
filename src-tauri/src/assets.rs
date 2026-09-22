use serde::Serialize;
use std::fs;
use std::path::PathBuf;

use crate::keys::load_keys;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImage {
    pub data_url: String,
    pub prompt: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshJob {
    pub id: String,
    pub provider: String,
    pub status: String,
    pub model_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAsset {
    pub kind: String,
    pub path: String,
    pub bank_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roblox_asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_path: Option<String>,
    pub prompt: String,
    #[serde(default)]
    pub can_retry: bool,
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
        "asset".into()
    } else {
        take
    }
}

pub fn assert_lumen_project(project_path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(project_path);
    if !path.is_dir() {
        return Err("Dossier projet introuvable".into());
    }
    if !path.join(".lumen.json").exists() {
        return Err("Ce dossier n’est pas un projet Lumen".into());
    }
    let root = crate::projects::projects_root()?;
    let canon = path.canonicalize().map_err(|e| e.to_string())?;
    let root_canon = root.canonicalize().map_err(|e| e.to_string())?;
    if !canon.starts_with(&root_canon) {
        return Err("Projet hors du dossier Lumen".into());
    }
    Ok(canon)
}

pub fn generate_image_for_project(project_path: &str, prompt: &str) -> Result<SavedAsset, String> {
    let project = assert_lumen_project(project_path)?;
    let generated = generate_image(prompt.to_string())?;
    let filename = format!("{}-{}.png", slug_name(prompt), now_stamp());
    let saved = save_image_to_project(
        project.to_string_lossy().into_owned(),
        generated.data_url,
        filename,
    )?;
    Ok(SavedAsset {
        kind: "image".into(),
        path: saved.path.clone(),
        bank_id: Some(saved.bank_id),
        code: Some(saved.code),
        roblox_asset_id: saved.roblox_asset_id,
        publish_error: saved.publish_error,
        preview_path: Some(saved.path),
        prompt: prompt.into(),
        can_retry: true,
    })
}

pub fn generate_mesh_for_project(project_path: &str, prompt: &str) -> Result<SavedAsset, String> {
    let project = assert_lumen_project(project_path)?;
    let job = generate_mesh(prompt.to_string())?;
    let mut last = job;
    for _ in 0..45 {
        std::thread::sleep(std::time::Duration::from_secs(4));
        last = poll_mesh(last.provider.clone(), last.id.clone())?;
        if let Some(url) = last.model_url.clone() {
            let name = format!("{}-{}", slug_name(prompt), now_stamp());
            let item = save_mesh_url(
                url,
                name,
                Some(project.to_string_lossy().into_owned()),
                last.thumbnail_url.clone(),
            )?;
            return Ok(SavedAsset {
                kind: "mesh".into(),
                path: item.path,
                bank_id: Some(item.id),
                code: Some(item.code),
                roblox_asset_id: item.roblox_asset_id,
                publish_error: None,
                preview_path: item.preview_path,
                prompt: prompt.into(),
                can_retry: true,
            });
        }
        let status = last.status.to_uppercase();
        if status == "FAILED" || status == "CANCELED" || status == "ERROR" || status == "CANCELLED" {
            return Err(format!("Génération 3D échouée ({status})"));
        }
    }
    Err("Génération 3D trop longue (timeout 3 min)".into())
}

fn now_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[tauri::command]
pub fn generate_image(prompt: String) -> Result<GeneratedImage, String> {
    let keys = load_keys()?;
    if keys.gemini.trim().is_empty() {
        return Err("Ajoute une clé API Gemini dans Réglages".into());
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent?key={}",
        keys.gemini.trim()
    );
    let body = serde_json::json!({
        "contents": [{
            "parts": [{ "text": wrap_image_prompt(&prompt) }]
        }],
        "generationConfig": {
            "responseModalities": ["IMAGE", "TEXT"]
        }
    });

    let res = client()?
        .post(url)
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let json: serde_json::Value = res.json().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(json.to_string());
    }

    let b64 = json
        .pointer("/candidates/0/content/parts")
        .and_then(|p| p.as_array())
        .and_then(|parts| {
            parts.iter().find_map(|part| {
                part.pointer("/inlineData/data")
                    .or_else(|| part.pointer("/inline_data/data"))
                    .and_then(|v| v.as_str())
            })
        })
        .ok_or_else(|| format!("Gemini n'a pas renvoyé d'image: {json}"))?;

    use base64::Engine;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| e.to_string())?;
    let png = finish_generated_image(&prompt, &raw)?;
    let out_b64 = base64::engine::general_purpose::STANDARD.encode(&png);

    Ok(GeneratedImage {
        data_url: format!("data:image/png;base64,{out_b64}"),
        prompt,
    })
}

fn wrap_image_prompt(prompt: &str) -> String {
    if wants_opaque_background(prompt) {
        return format!("Generate a Roblox-ready game asset image. {prompt}");
    }
    format!(
        "Generate a Roblox-ready 2D game asset as a PNG with a real transparent alpha channel.\n\
         User request: {prompt}\n\
         Hard rules: isolated subject only; no background, no studio backdrop, no floor, no sky, \
         no checkerboard, no white/gray/colored plate, no shadow catcher. \
         Opaque pixels = the asset only. Transparent pixels everywhere else."
    )
}

fn wants_opaque_background(prompt: &str) -> bool {
    let p = prompt.to_ascii_lowercase();
    if p.contains("fond transparent")
        || p.contains("sans fond")
        || p.contains("transparent background")
        || p.contains("alpha")
    {
        return false;
    }
    [
        "fond blanc",
        "fond noir",
        "fond coloré",
        "fond colore",
        "solid background",
        "white background",
        "black background",
        "seamless",
        "tileable",
        "tiling",
        "skybox",
        "paysage",
        "landscape",
        "wallpaper",
        "arrière-plan rempli",
        "arriere-plan rempli",
    ]
    .iter()
    .any(|key| p.contains(key))
}

fn finish_generated_image(prompt: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = match image::load_from_memory(bytes) {
        Ok(img) => img,
        Err(_) => return Ok(bytes.to_vec()),
    };
    let mut rgba = img.to_rgba8();
    if !wants_opaque_background(prompt) {
        punch_flat_background(&mut rgba);
    }
    encode_rgba_png(&rgba)
}

fn encode_rgba_png(img: &image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

fn color_dist(pixel: &image::Rgba<u8>, bg: [u8; 3]) -> u32 {
    let dr = pixel[0].abs_diff(bg[0]) as u32;
    let dg = pixel[1].abs_diff(bg[1]) as u32;
    let db = pixel[2].abs_diff(bg[2]) as u32;
    dr.max(dg).max(db)
}

fn punch_flat_background(img: &mut image::RgbaImage) {
    let (w, h) = img.dimensions();
    if w < 8 || h < 8 {
        return;
    }
    let already = img.pixels().filter(|p| p[3] < 16).count();
    if already * 12 > (w * h) as usize {
        return;
    }
    let samples = [
        img.get_pixel(0, 0).0,
        img.get_pixel(w - 1, 0).0,
        img.get_pixel(0, h - 1).0,
        img.get_pixel(w - 1, h - 1).0,
        img.get_pixel(w / 2, 0).0,
        img.get_pixel(w / 2, h - 1).0,
        img.get_pixel(0, h / 2).0,
        img.get_pixel(w - 1, h / 2).0,
    ];
    let bg = [
        (samples.iter().map(|p| p[0] as u32).sum::<u32>() / 8) as u8,
        (samples.iter().map(|p| p[1] as u32).sum::<u32>() / 8) as u8,
        (samples.iter().map(|p| p[2] as u32).sum::<u32>() / 8) as u8,
    ];
    if samples.iter().any(|p| color_dist(&image::Rgba(*p), bg) > 28) {
        return;
    }
    let backup = img.clone();
    let tol = 40u32;
    let mut seen = vec![false; (w * h) as usize];
    let mut stack: Vec<(u32, u32)> = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    let mut punched = 0u32;
    while let Some((x, y)) = stack.pop() {
        let i = (y * w + x) as usize;
        if seen[i] {
            continue;
        }
        seen[i] = true;
        let pixel = *img.get_pixel(x, y);
        if pixel[3] < 16 || color_dist(&pixel, bg) > tol {
            continue;
        }
        img.put_pixel(x, y, image::Rgba([pixel[0], pixel[1], pixel[2], 0]));
        punched += 1;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
    let total = w * h;
    if punched < total / 12 || punched > (total * 9) / 10 {
        *img = backup;
    }
}

#[tauri::command]
pub fn generate_mesh(prompt: String) -> Result<MeshJob, String> {
    let keys = load_keys()?;
    if keys.mesh_provider.eq_ignore_ascii_case("blender") {
        return Err(
            "Moteur 3D = Blender. Écris un script bpy (sphères, cubes, cylindres, matériaux) dans assets/blender/, puis : node tools/lumen-asset.mjs blender assets/blender/nom.py \"Titre\"".into(),
        );
    }
    let provider = if keys.mesh_provider == "tripo" {
        "tripo"
    } else {
        "meshy"
    };
    match provider {
        "tripo" => start_tripo(&keys.tripo, &prompt),
        _ => start_meshy(&keys.meshy, &prompt),
    }
}

fn start_meshy(key: &str, prompt: &str) -> Result<MeshJob, String> {
    if key.trim().is_empty() {
        return Err("Ajoute une clé API Meshy dans Réglages".into());
    }
    let res = client()?
        .post("https://api.meshy.ai/openapi/v2/text-to-3d")
        .bearer_auth(key.trim())
        .json(&serde_json::json!({
            "mode": "preview",
            "prompt": prompt,
            "art_style": "realistic",
            "should_remesh": true
        }))
        .send()
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = res.json().map_err(|e| e.to_string())?;
    let id = json
        .get("result")
        .and_then(|v| v.as_str())
        .or_else(|| json.get("id").and_then(|v| v.as_str()))
        .ok_or_else(|| format!("Réponse Meshy inattendue: {json}"))?;
    Ok(MeshJob {
        id: id.into(),
        provider: "meshy".into(),
        status: "PENDING".into(),
        model_url: None,
        thumbnail_url: None,
    })
}

fn start_tripo(key: &str, prompt: &str) -> Result<MeshJob, String> {
    if key.trim().is_empty() {
        return Err("Ajoute une clé API Tripo dans Réglages".into());
    }
    let res = client()?
        .post("https://api.tripo3d.ai/v2/openapi/task")
        .header("Authorization", format!("Bearer {}", key.trim()))
        .json(&serde_json::json!({
            "type": "text_to_model",
            "prompt": prompt
        }))
        .send()
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = res.json().map_err(|e| e.to_string())?;
    let id = json
        .pointer("/data/task_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Réponse Tripo inattendue: {json}"))?;
    Ok(MeshJob {
        id: id.into(),
        provider: "tripo".into(),
        status: "PENDING".into(),
        model_url: None,
        thumbnail_url: None,
    })
}

#[tauri::command]
pub fn poll_mesh(provider: String, id: String) -> Result<MeshJob, String> {
    let keys = load_keys()?;
    if provider == "tripo" {
        let res = client()?
            .get(format!("https://api.tripo3d.ai/v2/openapi/task/{id}"))
            .header("Authorization", format!("Bearer {}", keys.tripo.trim()))
            .send()
            .map_err(|e| e.to_string())?;
        let json: serde_json::Value = res.json().map_err(|e| e.to_string())?;
        let status = json
            .pointer("/data/status")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();
        let model_url = json
            .pointer("/data/output/model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let thumbnail_url = json
            .pointer("/data/output/rendered_image")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        return Ok(MeshJob {
            id,
            provider,
            status,
            model_url,
            thumbnail_url,
        });
    }

    let res = client()?
        .get(format!("https://api.meshy.ai/openapi/v2/text-to-3d/{id}"))
        .bearer_auth(keys.meshy.trim())
        .send()
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = res.json().map_err(|e| e.to_string())?;
    let status = json
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN")
        .to_string();
    let model_url = json
        .pointer("/model_urls/glb")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let thumbnail_url = json
        .get("thumbnail_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(MeshJob {
        id,
        provider,
        status,
        model_url,
        thumbnail_url,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedImageFile {
    pub path: String,
    pub bank_id: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roblox_asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_error: Option<String>,
}

#[tauri::command]
pub fn save_image_to_project(
    project_path: String,
    data_url: String,
    filename: String,
) -> Result<SavedImageFile, String> {
    let dir = PathBuf::from(&project_path).join("assets").join("images");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let raw = data_url
        .split(',')
        .nth(1)
        .ok_or("Image invalide")?;
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|e| e.to_string())?;
    let path = dir.join(&filename);
    fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let item = crate::bank::add_bytes_to_bank(&bytes, &filename, "image", "gemini")?;
    let (published, publish_error) = crate::publish::auto_publish(item);
    Ok(SavedImageFile {
        path: path.to_string_lossy().into(),
        bank_id: published.id,
        code: published.code,
        roblox_asset_id: published.roblox_asset_id,
        publish_error,
    })
}

#[tauri::command]
pub fn save_mesh_url(
    url: String,
    name: String,
    project_path: Option<String>,
    thumbnail_url: Option<String>,
) -> Result<crate::bank::BankItem, String> {
    let bytes = client()?
        .get(&url)
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    let mut item = crate::bank::add_bytes_to_bank(&bytes, &format!("{name}.glb"), "mesh", "3d")?;
    if let Some(thumb) = thumbnail_url.filter(|u| !u.is_empty()) {
        if let Ok(png) = client()?.get(&thumb).send().and_then(|r| r.bytes()) {
            let preview = PathBuf::from(&item.path).with_extension("png");
            if fs::write(&preview, &png).is_ok() {
                let preview_s = preview.to_string_lossy().into_owned();
                let _ = crate::bank::attach_preview(&item.id, preview_s.clone());
                item.preview_path = Some(preview_s);
            }
        }
    }
    let (mut item, _) = crate::publish::auto_publish(item);
    if let Some(project_path) = project_path {
        if let Ok(project) = assert_lumen_project(&project_path) {
            let dir = project.join("assets").join("meshes");
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let dest = dir.join(format!("{name}.glb"));
            fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
            item.path = dest.to_string_lossy().into();
            return Ok(item);
        }
    }
    Ok(item)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PastedImage {
    pub path: String,
    pub relative_path: String,
}

#[tauri::command]
pub fn save_pasted_image(
    project_path: String,
    data_base64: String,
    mime: Option<String>,
) -> Result<PastedImage, String> {
    let project = assert_lumen_project(&project_path)?;
    let mime = mime.unwrap_or_default().to_ascii_lowercase();
    let ext = if mime.contains("jpeg") || mime.contains("jpg") {
        "jpg"
    } else if mime.contains("webp") {
        "webp"
    } else if mime.contains("gif") {
        "gif"
    } else {
        "png"
    };
    let cleaned: String = data_base64
        .split(',')
        .last()
        .unwrap_or(&data_base64)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cleaned)
        .map_err(|e| format!("Image invalide : {e}"))?;
    if bytes.is_empty() {
        return Err("Image vide".into());
    }
    if bytes.len() > 12 * 1024 * 1024 {
        return Err("Image trop lourde (12 Mo max)".into());
    }
    let dir = project.join("assets").join("inbox");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_millis())
        .unwrap_or(0);
    let name = format!("paste-{}-{:03}.{}", now_stamp(), millis, ext);
    let dest = dir.join(&name);
    fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    let relative = format!("assets/inbox/{name}");
    Ok(PastedImage {
        path: dest.to_string_lossy().into_owned(),
        relative_path: relative,
    })
}
