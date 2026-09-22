use crate::bank::BankItem;
use crate::keys::load_keys;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

const BANK_REPO: &str = "Hazeyy5/lumen-studio";
const BANK_TAG: &str = "bank";
const MAX_FILE_BYTES: u64 = 40 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub pulled: u32,
    pub pushed: u32,
    pub sharing: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SharedCatalog {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    items: Vec<SharedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedItem {
    id: String,
    code: String,
    name: String,
    kind: String,
    source: String,
    created_at: String,
    roblox_asset_id: Option<String>,
    hash: String,
    file: String,
    #[serde(default)]
    preview: Option<String>,
    #[serde(default)]
    key: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    #[allow(dead_code)]
    id: u64,
    upload_url: String,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize, Clone)]
struct GhAsset {
    id: u64,
    name: String,
    size: u64,
    browser_download_url: String,
}

fn sharing_on() -> bool {
    load_keys().map(|k| k.share_bank).unwrap_or(true)
}

fn should_share(item: &BankItem) -> bool {
    if item.source.eq_ignore_ascii_case("vibestarter")
        || item.source.eq_ignore_ascii_case("texture")
    {
        return false;
    }
    let path = Path::new(&item.path);
    path.is_file()
}

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent("Lumen/0.1")
        .build()
        .map_err(|e| e.to_string())
}

pub(crate) fn github_token() -> Option<String> {
    if let Ok(keys) = load_keys() {
        let t = keys.bank_sync_token.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let out = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

fn api_headers(token: Option<&str>) -> reqwest::header::HeaderMap {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Lumen/0.1"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    if let Some(token) = token {
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {token}")) {
            headers.insert(AUTHORIZATION, value);
        }
    }
    headers
}

fn get_release(client: &reqwest::blocking::Client, token: Option<&str>) -> Result<Option<GhRelease>, String> {
    let url = format!("https://api.github.com/repos/{BANK_REPO}/releases/tags/{BANK_TAG}");
    let res = client
        .get(&url)
        .headers(api_headers(token))
        .send()
        .map_err(|e| e.to_string())?;
    if res.status().as_u16() == 404 {
        return Ok(None);
    }
    if !res.status().is_success() {
        return Err(format!("GitHub banque: HTTP {}", res.status()));
    }
    res.json().map(Some).map_err(|e| e.to_string())
}

fn ensure_release(client: &reqwest::blocking::Client, token: Option<&str>) -> Result<GhRelease, String> {
    if let Some(release) = get_release(client, token)? {
        return Ok(release);
    }
    let token = token.ok_or_else(|| {
        "Le catalogue partagé n’existe pas encore. Le premier envoi (GitHub CLI ou jeton) le crée.".to_string()
    })?;
    let body = serde_json::json!({
        "tag_name": BANK_TAG,
        "name": "Banque partagée",
        "body": "Catalogue Lumen synchronisé entre utilisateurs. Pre-release volontaire : n’écrase pas latest.json.",
        "prerelease": true,
        "draft": false,
        "target_commitish": "main"
    });
    let created = client
        .post(format!("https://api.github.com/repos/{BANK_REPO}/releases"))
        .headers(api_headers(Some(token)))
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    if !created.status().is_success() {
        let status = created.status();
        let text = created.text().unwrap_or_default();
        return Err(format!("Impossible de créer la release banque ({status}): {text}"));
    }
    created.json().map_err(|e| e.to_string())
}

fn download_catalog(
    client: &reqwest::blocking::Client,
    release: &GhRelease,
) -> Result<SharedCatalog, String> {
    let Some(asset) = release.assets.iter().find(|a| a.name == "catalog.json") else {
        return Ok(SharedCatalog {
            version: 1,
            items: Vec::new(),
        });
    };
    let bytes = client
        .get(&asset.browser_download_url)
        .header("User-Agent", "Lumen/0.1")
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(SharedCatalog {
        version: 1,
        items: Vec::new(),
    }))
}

fn upload_bytes(
    client: &reqwest::blocking::Client,
    token: &str,
    release: &GhRelease,
    name: &str,
    bytes: &[u8],
    content_type: &str,
) -> Result<(), String> {
    if let Some(existing) = release.assets.iter().find(|a| a.name == name) {
        let _ = client
            .delete(format!(
                "https://api.github.com/repos/{BANK_REPO}/releases/assets/{}",
                existing.id
            ))
            .headers(api_headers(Some(token)))
            .send();
    }
    let upload = release
        .upload_url
        .split('{')
        .next()
        .unwrap_or(&release.upload_url);
    let url = format!("{upload}?name={}", urlencoding::encode(name));
    let res = client
        .post(&url)
        .header("User-Agent", "Lumen/0.1")
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", content_type)
        .body(bytes.to_vec())
        .send()
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Upload {name}: HTTP {}", res.status()));
    }
    Ok(())
}

fn ext_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase()
}

fn to_shared(item: &BankItem) -> SharedItem {
    let ext = ext_of(&item.path);
    SharedItem {
        id: item.id.clone(),
        code: item.code.clone(),
        name: item.name.clone(),
        kind: item.kind.clone(),
        source: item.source.clone(),
        created_at: item.created_at.clone(),
        roblox_asset_id: item.roblox_asset_id.clone(),
        hash: item.hash.clone(),
        file: format!("{}.{ext}", item.id),
        preview: item.preview_path.as_ref().map(|_| format!("preview-{}.png", item.id)),
        key: String::new(),
    }
}

fn pull_one(
    client: &reqwest::blocking::Client,
    release: &GhRelease,
    remote: &SharedItem,
) -> Result<bool, String> {
    if remote.source.eq_ignore_ascii_case("texture") {
        return pull_texture_file(client, release, remote);
    }
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == remote.file)
        .ok_or_else(|| format!("Fichier distant manquant: {}", remote.file))?;
    if asset.size > MAX_FILE_BYTES {
        return Err(format!("{} trop volumineux", remote.name));
    }
    let bytes = client
        .get(&asset.browser_download_url)
        .header("User-Agent", "Lumen/0.1")
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    let ext = ext_of(&remote.file);
    let dest = crate::bank::bank_root()?
        .join("files")
        .join(format!("{}.{ext}", remote.id));
    fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    let mut preview_path = None;
    if let Some(preview_name) = remote.preview.as_ref() {
        if let Some(prev) = release.assets.iter().find(|a| &a.name == preview_name) {
            if let Ok(pbytes) = client
                .get(&prev.browser_download_url)
                .header("User-Agent", "Lumen/0.1")
                .send()
                .and_then(|r| r.bytes())
            {
                let pdest = crate::bank::bank_root()?
                    .join("previews")
                    .join(format!("{}.png", remote.id));
                fs::create_dir_all(pdest.parent().unwrap_or(Path::new("."))).ok();
                if fs::write(&pdest, &pbytes).is_ok() {
                    preview_path = Some(pdest.to_string_lossy().into_owned());
                }
            }
        }
    }
    let item = BankItem {
        id: remote.id.clone(),
        name: remote.name.clone(),
        kind: remote.kind.clone(),
        path: dest.to_string_lossy().into(),
        source: remote.source.clone(),
        created_at: remote.created_at.clone(),
        roblox_asset_id: remote.roblox_asset_id.clone(),
        preview_path,
        code: remote.code.clone(),
        scale_type: None,
        tile_size: None,
        hash: remote.hash.clone(),
        shared: true,
    };
    crate::bank::ingest_shared(item)
}

fn pull_texture_file(
    client: &reqwest::blocking::Client,
    release: &GhRelease,
    remote: &SharedItem,
) -> Result<bool, String> {
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == remote.file)
        .ok_or_else(|| format!("Texture distante manquante: {}", remote.file))?;
    if asset.size > MAX_FILE_BYTES {
        return Err(format!("{} trop volumineux", remote.name));
    }
    let rel = remote.key.replace('\\', "/");
    if rel.is_empty() || rel.contains("..") {
        return Err("Chemin de texture invalide".into());
    }
    let dest = crate::textures::shared_textures_dir()?.join(&rel);
    if dest.is_file() {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = client
        .get(&asset.browser_download_url)
        .header("User-Agent", "Lumen/0.1")
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
    Ok(true)
}

fn push_texture(
    client: &reqwest::blocking::Client,
    token: &str,
    release: &mut GhRelease,
    catalog: &mut SharedCatalog,
    item: &BankItem,
) -> Result<bool, String> {
    if !Path::new(&item.path).is_file() {
        return Ok(false);
    }
    let key = item
        .id
        .strip_prefix("tex:")
        .unwrap_or(&item.id)
        .replace('\\', "/");
    if catalog.items.iter().any(|row| row.id == item.id || row.key == key) {
        return Ok(false);
    }
    let hash = crate::bank::sha256_file(Path::new(&item.path))?;
    let ext = ext_of(&item.path);
    let file = format!("{}.{ext}", item.code.replace(' ', "-"));
    let mut shared = to_shared(item);
    shared.file = file;
    shared.key = key;
    shared.hash = hash;
    let bytes = fs::read(&item.path).map_err(|e| e.to_string())?;
    upload_bytes(client, token, release, &shared.file, &bytes, "application/octet-stream")?;
    catalog.items.push(shared);
    *release = ensure_release(client, Some(token))?;
    Ok(true)
}

fn pull_texture_catalog(client: &reqwest::blocking::Client, release: &GhRelease) -> Result<(), String> {
    let Some(asset) = release.assets.iter().find(|a| a.name == "catalog-textures.json") else {
        return Ok(());
    };
    let bytes = client
        .get(&asset.browser_download_url)
        .header("User-Agent", "Lumen/0.1")
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    crate::textures::merge_catalog_json(&bytes)
}

fn push_texture_catalog(
    client: &reqwest::blocking::Client,
    token: &str,
    release: &GhRelease,
) -> Result<(), String> {
    let Some(docs) = dirs::document_dir() else {
        return Ok(());
    };
    let path = docs.join("Lumen").join("catalog-textures.json");
    if !path.is_file() {
        return Ok(());
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    upload_bytes(client, token, release, "catalog-textures.json", &bytes, "application/json")
}

fn push_one(
    client: &reqwest::blocking::Client,
    token: &str,
    release: &mut GhRelease,
    catalog: &mut SharedCatalog,
    item: &BankItem,
) -> Result<bool, String> {
    if !should_share(item) {
        return Ok(false);
    }
    let hash = if item.hash.trim().is_empty() {
        crate::bank::sha256_file(Path::new(&item.path))?
    } else {
        item.hash.clone()
    };
    if catalog.items.iter().any(|row| row.hash == hash || row.id == item.id) {
        crate::bank::mark_shared(&item.id)?;
        return Ok(false);
    }
    let meta = fs::metadata(&item.path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!("{} dépasse 40 Mo, non partagé", item.name));
    }
    let bytes = fs::read(&item.path).map_err(|e| e.to_string())?;
    let mut shared = to_shared(item);
    shared.hash = hash;
    upload_bytes(client, token, release, &shared.file, &bytes, "application/octet-stream")?;
    if let Some(preview) = item.preview_path.as_ref() {
        if Path::new(preview).is_file() {
            if let Ok(pbytes) = fs::read(preview) {
                if let Some(name) = shared.preview.as_ref() {
                    let _ = upload_bytes(client, token, release, name, &pbytes, "image/png");
                }
            }
        }
    }
    catalog.items.push(shared);
    crate::bank::mark_shared(&item.id)?;
    *release = ensure_release(client, Some(token))?;
    Ok(true)
}

fn save_catalog(
    client: &reqwest::blocking::Client,
    token: &str,
    release: &GhRelease,
    catalog: &SharedCatalog,
) -> Result<(), String> {
    let mut catalog = catalog.clone();
    catalog.version = 1;
    let json = serde_json::to_vec_pretty(&catalog).map_err(|e| e.to_string())?;
    upload_bytes(client, token, release, "catalog.json", &json, "application/json")
}

fn lock_sync() -> std::sync::MutexGuard<'static, bool> {
    static LOCK: std::sync::OnceLock<Mutex<bool>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(false))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn queue_push(item: &BankItem) {
    if !sharing_on() || !should_share(item) {
        return;
    }
    let item = item.clone();
    std::thread::spawn(move || {
        let _ = sync_once(false, Some(&item));
    });
}

fn sync_once(pull: bool, extra_push: Option<&BankItem>) -> Result<SyncStatus, String> {
    let sharing = sharing_on();
    if !sharing {
        return Ok(SyncStatus {
            pulled: 0,
            pushed: 0,
            sharing: false,
            message: "Partage désactivé dans Réglages.".into(),
        });
    }
    let mut busy = lock_sync();
    if *busy {
        return Ok(SyncStatus {
            pulled: 0,
            pushed: 0,
            sharing: true,
            message: "Synchronisation déjà en cours.".into(),
        });
    }
    *busy = true;
    drop(busy);
    let result = (|| {
        let client = http()?;
        let token = github_token();
        let mut release = match get_release(&client, token.as_deref())? {
            Some(release) => release,
            None if token.is_none() && extra_push.is_none() && pull => {
                return Ok(SyncStatus {
                    pulled: 0,
                    pushed: 0,
                    sharing: true,
                    message: "Catalogue partagé encore vide. Tes ajouts partiront dès qu’un jeton GitHub est dispo.".into(),
                });
            }
            None => ensure_release(&client, token.as_deref())?,
        };
        let mut catalog = download_catalog(&client, &release)?;
        let mut pulled = 0u32;
        if pull {
            let _ = pull_texture_catalog(&client, &release);
            for remote in catalog.items.clone() {
                match pull_one(&client, &release, &remote) {
                    Ok(true) => pulled += 1,
                    Ok(false) => {}
                    Err(_) => {}
                }
            }
            crate::textures::clear_cache();
        }
        let mut pushed = 0u32;
        if let Some(token) = token.as_deref() {
            let mut locals = crate::bank::load_index()?;
            if let Some(extra) = extra_push {
                if !locals.iter().any(|row| row.id == extra.id) {
                    locals.push(extra.clone());
                }
            }
            let mut changed = false;
            for item in &locals {
                match push_one(&client, token, &mut release, &mut catalog, item) {
                    Ok(true) => {
                        pushed += 1;
                        changed = true;
                    }
                    Ok(false) => {}
                    Err(_) => {}
                }
            }
            if let Ok(textures) = crate::textures::list_textures() {
                for item in &textures {
                    match push_texture(&client, token, &mut release, &mut catalog, item) {
                        Ok(true) => {
                            pushed += 1;
                            changed = true;
                        }
                        Ok(false) => {}
                        Err(_) => {}
                    }
                }
            }
            if changed {
                let _ = save_catalog(&client, token, &release, &catalog);
                let _ = push_texture_catalog(&client, token, &release);
            }
        }
        let message = if token.is_none() && pulled == 0 && extra_push.is_some() {
            "Banque lue. Pour envoyer tes ajouts, connecte GitHub CLI ou colle un jeton dans Réglages.".into()
        } else {
            format!("{pulled} reçu(s), {pushed} envoyé(s)")
        };
        Ok(SyncStatus {
            pulled,
            pushed,
            sharing: true,
            message,
        })
    })();
    *lock_sync() = false;
    result
}

#[tauri::command]
pub fn sync_shared_bank() -> Result<SyncStatus, String> {
    sync_once(true, None)
}
