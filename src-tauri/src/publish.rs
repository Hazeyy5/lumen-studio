use crate::bank::{get_bank_item, mark_published, BankItem};
use crate::keys::load_keys;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    pub asset_id: String,
    pub operation: String,
    pub code: String,
    pub kind: String,
}

#[tauri::command]
pub fn publish_bank_item(id: String) -> Result<PublishResult, String> {
    publish_ref(&id)
}

#[tauri::command]
pub fn import_to_bank(file_path: String, source: Option<String>) -> Result<BankItem, String> {
    let item = crate::bank::import_to_bank(file_path, source)?;
    if crate::bank::is_inspiration(&item) {
        return Ok(item);
    }
    let (item, _) = auto_publish(item);
    Ok(item)
}

pub fn auto_publish(mut item: BankItem) -> (BankItem, Option<String>) {
    let (id, err) = try_publish(&item.id);
    if id.is_some() {
        item.roblox_asset_id = id;
        (item, None)
    } else {
        (item, err)
    }
}

pub fn publish_ref(id_or_code: &str) -> Result<PublishResult, String> {
    let item = get_bank_item(id_or_code)?;
    if crate::bank::is_inspiration(&item) {
        return Err(
            "Les images d’inspiration (INS-xxxx) ne se publient pas sur Roblox. Elles servent de référence visuelle."
                .into(),
        );
    }
    if let Some(existing) = item
        .roblox_asset_id
        .as_ref()
        .filter(|id| !id.trim().is_empty())
    {
        return Ok(PublishResult {
            asset_id: existing.clone(),
            operation: String::new(),
            code: item.code,
            kind: item.kind,
        });
    }
    publish_item(&item)
}

pub fn try_publish(id_or_code: &str) -> (Option<String>, Option<String>) {
    match publish_ref(id_or_code) {
        Ok(result) if !result.asset_id.is_empty() => (Some(result.asset_id), None),
        Ok(_) => (
            None,
            Some("Roblox n’a pas renvoyé d’assetId. Réessaie `publish`.".into()),
        ),
        Err(err) => (None, Some(err)),
    }
}

fn publish_item(item: &BankItem) -> Result<PublishResult, String> {
    let item = crate::catalog::materialize(item)?;
    let item = &item;
    let keys = load_keys()?;
    let auths = publish_auths(&keys)?;
    if item.path.trim().is_empty() || !Path::new(&item.path).is_file() {
        return Err("Fichier introuvable pour la publication. Pour une texture Studio, l’ID Roblox est déjà dans la carte.".into());
    }
    let raw = fs::read(&item.path).map_err(|e| e.to_string())?;
    let (asset_type, mut mime, mut filename, mut bytes) = prepare_upload(item, raw.clone())?;
    let mut reencoded = sniff_image(&raw).is_none();

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent("Lumen/0.1")
        .build()
        .map_err(|e| e.to_string())?;

    let mut last_err = String::new();
    for _attempt in 0..2 {
        let mut retry_png = false;
        for auth in &auths {
            let user_id = creator_user_id(&keys, auth)?;
            match upload_asset(
                &client,
                item,
                &bytes,
                &filename,
                asset_type,
                mime,
                &user_id,
                auth,
            ) {
                Ok(result) => return Ok(result),
                Err(err) => {
                    last_err = err;
                    if is_auth_error(&last_err) {
                        continue;
                    }
                    if item.kind == "image" && is_unsupported_image(&last_err) && !reencoded {
                        bytes = encode_png(&raw)?;
                        mime = "image/png";
                        filename = png_filename(item);
                        reencoded = true;
                        retry_png = true;
                        break;
                    }
                    return Err(explain_auth_error(&last_err));
                }
            }
        }
        if !retry_png {
            break;
        }
    }
    Err(explain_auth_error(&last_err))
}

fn is_unsupported_image(err: &str) -> bool {
    err.to_ascii_lowercase().contains("unsupported image format")
}

fn png_filename(item: &BankItem) -> String {
    let stem = if item.code.trim().is_empty() {
        "asset"
    } else {
        item.code.trim()
    };
    format!("{stem}.png")
}

fn sniff_image(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(("image/png", "png"));
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some(("image/jpeg", "jpeg"));
    }
    if bytes.starts_with(b"BM") {
        return Some(("image/bmp", "bmp"));
    }
    if bytes.len() >= 18 && bytes[bytes.len() - 18..].starts_with(b"TRUEVISION-XFILE") {
        return Some(("image/tga", "tga"));
    }
    None
}

fn encode_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|err| {
        format!(
            "Roblox n’accepte que PNG, JPEG, BMP ou TGA. Convertis le fichier (WebP/GIF exclus) : {err}"
        )
    })?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

fn prepare_upload(
    item: &BankItem,
    raw: Vec<u8>,
) -> Result<(&'static str, &'static str, String, Vec<u8>), String> {
    match item.kind.as_str() {
        "image" => {
            let stem = if item.code.trim().is_empty() {
                "asset".into()
            } else {
                item.code.trim().to_string()
            };
            if let Some((mime, ext)) = sniff_image(&raw) {
                return Ok(("Decal", mime, format!("{stem}.{ext}"), raw));
            }
            let png = encode_png(&raw)?;
            Ok(("Decal", "image/png", format!("{stem}.png"), png))
        }
        "mesh" => {
            let (asset_type, mime) = asset_spec(item)?;
            let filename = Path::new(&item.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("asset.bin")
                .to_string();
            Ok((asset_type, mime, filename, raw))
        }
        other => Err(format!("Lumen ne publie pas encore les assets « {other} »")),
    }
}

#[derive(Clone)]
enum PublishAuth {
    Bearer(String),
    ApiKey(String),
}

fn sanitize_api_key(raw: &str) -> String {
    raw.trim()
        .trim_matches('"')
        .trim()
        .trim_start_matches("Bearer ")
        .trim_start_matches("bearer ")
        .trim()
        .to_string()
}

fn publish_auths(keys: &crate::keys::Keys) -> Result<Vec<PublishAuth>, String> {
    let mut auths = Vec::new();
    if let Ok(token) = crate::oauth::access_token() {
        auths.push(PublishAuth::Bearer(token));
    }
    let api_key = sanitize_api_key(&keys.roblox_api_key);
    if !api_key.is_empty() {
        auths.push(PublishAuth::ApiKey(api_key));
    }
    if auths.is_empty() {
        return Err(
            "Reconnecte-toi à Roblox après avoir ajouté asset:read et asset:write à l’app OAuth, ou colle une clé Open Cloud utilisateur (API Assets) dans Réglages."
                .into(),
        );
    }
    Ok(auths)
}

fn apply_auth(
    req: reqwest::blocking::RequestBuilder,
    auth: &PublishAuth,
) -> reqwest::blocking::RequestBuilder {
    match auth {
        PublishAuth::Bearer(token) => req.header("Authorization", format!("Bearer {token}")),
        PublishAuth::ApiKey(key) => req.header("x-api-key", key.as_str()),
    }
}

fn is_auth_error(err: &str) -> bool {
    let upper = err.to_ascii_uppercase();
    upper.contains("PERMISSION_DENIED")
        || upper.contains("UNAUTHENTICATED")
        || upper.contains("NOT AUTHENTICATED")
        || upper.contains("INSUFFICIENT_SCOPE")
        || upper.contains("401")
}

fn explain_auth_error(err: &str) -> String {
    if is_auth_error(err) {
        format!(
            "{err}\n\nRoblox n’a pas reconnu Lumen. Dans le Creator Dashboard, sur l’app OAuth, active les scopes asset:read et asset:write, puis déconnecte-toi et reconnecte-toi. En secours : une clé Open Cloud utilisateur (pas groupe) avec l’API Assets, sans restriction IP trop stricte."
        )
    } else {
        err.into()
    }
}

fn creator_user_id(keys: &crate::keys::Keys, auth: &PublishAuth) -> Result<String, String> {
    match auth {
        PublishAuth::Bearer(_) => crate::oauth::logged_in_user_id()
            .filter(|id| !id.is_empty())
            .or_else(|| {
                let id = keys.roblox_user_id.trim();
                if id.is_empty() {
                    None
                } else {
                    Some(id.into())
                }
            })
            .ok_or_else(|| "UserId Roblox manquant. Reconnecte-toi.".into()),
        PublishAuth::ApiKey(_) => {
            let from_keys = keys.roblox_user_id.trim();
            if !from_keys.is_empty() {
                return Ok(from_keys.into());
            }
            crate::oauth::logged_in_user_id()
                .ok_or_else(|| "Connecte-toi à Roblox ou ajoute ton UserId dans Réglages".into())
        }
    }
}

fn upload_asset(
    client: &reqwest::blocking::Client,
    item: &BankItem,
    bytes: &[u8],
    filename: &str,
    asset_type: &str,
    mime: &str,
    user_id: &str,
    auth: &PublishAuth,
) -> Result<PublishResult, String> {
    let request = serde_json::json!({
        "assetType": asset_type,
        "displayName": roblox_display_name(item),
        "description": roblox_description(item),
        "creationContext": {
            "creator": { "userId": user_id }
        }
    });

    let form = reqwest::blocking::multipart::Form::new()
        .text("request", request.to_string())
        .part(
            "fileContent",
            reqwest::blocking::multipart::Part::bytes(bytes.to_vec())
                .file_name(filename.to_string())
                .mime_str(mime)
                .map_err(|e| e.to_string())?,
        );

    let res = apply_auth(
        client.post("https://apis.roblox.com/assets/v1/assets"),
        auth,
    )
    .multipart(form)
    .send()
    .map_err(|e| e.to_string())?;
    let status = res.status();
    let json: serde_json::Value = res.json().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Roblox a refusé l’upload: {json}"));
    }

    let operation = json
        .get("path")
        .or_else(|| json.get("operationId"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let asset_id = poll_operation(client, auth, &json)?;
    if !asset_id.is_empty() {
        mark_published(&item.id, &asset_id)?;
    }
    Ok(PublishResult {
        asset_id,
        operation,
        code: item.code.clone(),
        kind: item.kind.clone(),
    })
}

fn roblox_display_name(item: &BankItem) -> String {
    let mut name: String = item
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect();
    name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.chars().count() < 3 {
        name = if item.code.trim().chars().count() >= 3 {
            item.code.trim().to_string()
        } else {
            "Lumen asset".into()
        };
    }
    let suffix = {
        let code = item.code.trim();
        if code.is_empty() {
            String::new()
        } else {
            format!(" ({code})")
        }
    };
    let suffix_len = suffix.chars().count();
    if suffix_len >= 3 && suffix_len <= 50 {
        let budget = 50usize.saturating_sub(suffix_len).max(3);
        let head: String = name.chars().take(budget).collect();
        let combined = format!("{head}{suffix}");
        let len = combined.chars().count();
        if (3..=50).contains(&len) {
            return combined;
        }
    }
    let clipped: String = name.chars().take(50).collect();
    if clipped.chars().count() < 3 {
        "Lumen asset".into()
    } else {
        clipped
    }
}

fn roblox_description(item: &BankItem) -> String {
    let raw = format!("Published from Lumen {} ({})", item.code, item.source);
    let clipped: String = raw.chars().take(1000).collect();
    if clipped.chars().count() < 3 {
        "Published from Lumen".into()
    } else {
        clipped
    }
}

fn asset_spec(item: &BankItem) -> Result<(&'static str, &'static str), String> {
    let ext = Path::new(&item.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match item.kind.as_str() {
        "image" => Ok(("Decal", "image/png")),
        "mesh" => match ext.as_str() {
            "glb" => Ok(("Model", "model/gltf-binary")),
            "gltf" => Ok(("Model", "model/gltf+json")),
            "fbx" => Ok(("Model", "model/fbx")),
            other => Err(format!(
                "Format 3D non publié automatiquement (.{other}). Open Cloud accepte GLB, GLTF ou FBX."
            )),
        },
        other => Err(format!("Lumen ne publie pas encore les assets « {other} »")),
    }
}

fn poll_operation(
    client: &reqwest::blocking::Client,
    auth: &PublishAuth,
    initial: &serde_json::Value,
) -> Result<String, String> {
    if let Some(id) = extract_asset_id(initial) {
        return Ok(id);
    }
    let path = initial
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Pas d’operationId: {initial}"))?;
    let op_id = path.rsplit('/').next().unwrap_or(path);
    for _ in 0..45 {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let res = apply_auth(
            client.get(format!("https://apis.roblox.com/assets/v1/operations/{op_id}")),
            auth,
        )
        .send()
        .map_err(|e| e.to_string())?;
        let json: serde_json::Value = res.json().map_err(|e| e.to_string())?;
        if json.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
            if let Some(id) = extract_asset_id(&json).or_else(|| extract_asset_id(&json["response"])) {
                return Ok(id);
            }
            if let Some(msg) = json.pointer("/error/message").and_then(|v| v.as_str()) {
                if !msg.is_empty() {
                    return Err(format!("Roblox : {msg}"));
                }
            }
            return Err(format!("Terminé sans assetId: {json}"));
        }
    }
    Err("Délai dépassé en attendant Roblox".into())
}

fn extract_asset_id(json: &serde_json::Value) -> Option<String> {
    json.pointer("/response/assetId")
        .or_else(|| json.pointer("/assetId"))
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())))
}
