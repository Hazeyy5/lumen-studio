use crate::keys::{load_keys, save_keys};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const AUTH_PORT: u16 = 17421;
const REDIRECT_URI: &str = "http://localhost:17421/callback";
const AUTHORIZE: &str = "https://apis.roblox.com/oauth/v1/authorize";
const TOKEN: &str = "https://apis.roblox.com/oauth/v1/token";
const USERINFO: &str = "https://apis.roblox.com/oauth/v1/userinfo";
const REVOKE: &str = "https://apis.roblox.com/oauth/v1/token/revoke";
const SCOPES: &str = "openid profile asset:read asset:write";
/// Client public de l’app Lumen (PKCE, pas de secret). Les comptes se connectent dessus.
const LUMEN_OAUTH_CLIENT_ID: &str = "8191681078061914952";

fn oauth_client_id(keys: &crate::keys::Keys) -> String {
    let custom = keys.roblox_oauth_client_id.trim();
    if custom.is_empty() {
        LUMEN_OAUTH_CLIENT_ID.to_string()
    } else {
        custom.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RobloxUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub picture: String,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    user: Option<RobloxUser>,
}

fn session_path() -> Result<PathBuf, String> {
    let dir = dirs::data_dir()
        .ok_or("AppData introuvable")?
        .join("Lumen");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("session.json"))
}

fn load_session() -> Result<Session, String> {
    let path = session_path()?;
    if !path.exists() {
        return Ok(Session::default());
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_session(session: &Session) -> Result<(), String> {
    fs::write(
        session_path()?,
        serde_json::to_string_pretty(session).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn random_token() -> String {
    let mut raw = Vec::with_capacity(32);
    raw.extend_from_slice(Uuid::new_v4().as_bytes());
    raw.extend_from_slice(Uuid::new_v4().as_bytes());
    URL_SAFE_NO_PAD.encode(&raw)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Lumen/0.1")
        .build()
        .map_err(|e| e.to_string())
}

fn exchange_token(
    client: &reqwest::blocking::Client,
    client_id: &str,
    code: &str,
    verifier: &str,
) -> Result<Session, String> {
    let res = client
        .post(TOKEN)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", verifier),
        ])
        .send()
        .map_err(|e| format!("Jeton Roblox: {e}"))?;
    let status = res.status();
    let json: serde_json::Value = res.json().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Roblox a refusé le jeton: {json}"));
    }
    session_from_token_json(json)
}

fn refresh_session(client: &reqwest::blocking::Client, session: &Session, client_id: &str) -> Result<Session, String> {
    if session.refresh_token.is_empty() {
        return Err("Session expirée. Reconnecte-toi.".into());
    }
    let res = client
        .post(TOKEN)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", session.refresh_token.as_str()),
        ])
        .send()
        .map_err(|e| format!("Rafraîchissement Roblox: {e}"))?;
    let status = res.status();
    let json: serde_json::Value = res.json().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err("Session expirée. Reconnecte-toi.".into());
    }
    session_from_token_json(json)
}

fn session_from_token_json(json: serde_json::Value) -> Result<Session, String> {
    let access = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("access_token manquant")?
        .to_string();
    let refresh = json
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let expires_in = json.get("expires_in").and_then(|v| v.as_u64()).unwrap_or(900);
    Ok(Session {
        access_token: access,
        refresh_token: refresh,
        expires_at: now().saturating_add(expires_in.saturating_sub(30)),
        user: None,
    })
}

fn fetch_user(client: &reqwest::blocking::Client, access_token: &str) -> Result<RobloxUser, String> {
    let res = client
        .get(USERINFO)
        .bearer_auth(access_token)
        .send()
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let json: serde_json::Value = res.json().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Profil Roblox: {json}"));
    }
    Ok(RobloxUser {
        id: json.get("sub").and_then(|v| v.as_str()).unwrap_or("").into(),
        username: json
            .get("preferred_username")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .into(),
        display_name: json
            .get("name")
            .or_else(|| json.get("nickname"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .into(),
        picture: json.get("picture").and_then(|v| v.as_str()).unwrap_or("").into(),
        profile: json.get("profile").and_then(|v| v.as_str()).unwrap_or("").into(),
    })
}

pub fn logged_in_user_id() -> Option<String> {
    load_session()
        .ok()?
        .user
        .map(|user| user.id.trim().to_string())
        .filter(|id| !id.is_empty())
}

pub fn access_token() -> Result<String, String> {
    let token = valid_session()?.access_token.trim().to_string();
    if token.is_empty() {
        return Err("non connecté".into());
    }
    Ok(token)
}

fn persist_user(user: &RobloxUser) {
    if let Ok(mut keys) = load_keys() {
        if keys.roblox_user_id.trim().is_empty() {
            keys.roblox_user_id = user.id.clone();
            let _ = save_keys(keys);
        }
    }
}

fn valid_session() -> Result<Session, String> {
    let mut session = load_session()?;
    if session.access_token.is_empty() {
        return Err("non connecté".into());
    }
    let keys = load_keys()?;
    let client_id = oauth_client_id(&keys);
    let client = http()?;
    let mut dirty = false;
    if session.expires_at <= now() {
        session = refresh_session(&client, &session, &client_id)?;
        dirty = true;
    }
    if session.user.is_none() {
        session.user = Some(fetch_user(&client, &session.access_token)?);
        dirty = true;
    }
    if dirty {
        save_session(&session)?;
    }
    Ok(session)
}

#[tauri::command]
pub async fn get_roblox_user() -> Result<Option<RobloxUser>, String> {
    tauri::async_runtime::spawn_blocking(|| match valid_session() {
        Ok(session) => Ok(session.user),
        Err(_) => Ok(None),
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn logout_roblox() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(logout_roblox_inner)
        .await
        .map_err(|e| e.to_string())?
}

fn logout_roblox_inner() -> Result<(), String> {
    if let Ok(session) = load_session() {
        if !session.refresh_token.is_empty() {
            if let Ok(keys) = load_keys() {
                let client_id = oauth_client_id(&keys);
                if let Ok(client) = http() {
                    let _ = client
                        .post(REVOKE)
                        .form(&[
                            ("token", session.refresh_token.as_str()),
                            ("client_id", client_id.as_str()),
                        ])
                        .send();
                }
            }
        }
    }
    let _ = fs::remove_file(session_path()?);
    Ok(())
}

#[tauri::command]
pub async fn start_roblox_login(app: tauri::AppHandle) -> Result<RobloxUser, String> {
    tauri::async_runtime::spawn_blocking(move || start_roblox_login_inner(app))
        .await
        .map_err(|e| e.to_string())?
}

fn start_roblox_login_inner(app: tauri::AppHandle) -> Result<RobloxUser, String> {
    let keys = load_keys()?;
    let client_id = oauth_client_id(&keys);

    let verifier = random_token();
    let challenge = pkce_challenge(&verifier);
    let state = random_token();
    let url = format!(
        "{AUTHORIZE}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        urlencoding::encode(&client_id),
        urlencoding::encode(REDIRECT_URI),
        urlencoding::encode(SCOPES),
        urlencoding::encode(&state),
        urlencoding::encode(&challenge),
    );

    let (tx, rx) = mpsc::channel::<Result<(String, String), String>>();
    let expected_state = state.clone();
    std::thread::spawn(move || {
        let server = match tiny_http::Server::http(format!("127.0.0.1:{AUTH_PORT}")) {
            Ok(s) => s,
            Err(err) => {
                let _ = tx.send(Err(format!("Impossible d’ouvrir le port {AUTH_PORT}: {err}")));
                return;
            }
        };
        for request in server.incoming_requests() {
            let raw = request.url().to_string();
            let path = raw.split('?').next().unwrap_or("/");
            if path == "/cancel" {
                let _ = request.respond(tiny_http::Response::from_string("ok"));
                break;
            }
            if path != "/callback" {
                let _ = request.respond(tiny_http::Response::from_string("Lumen"));
                continue;
            }
            let query = raw.split_once('?').map(|(_, q)| q).unwrap_or("");
            let params: Vec<(String, String)> = query
                .split('&')
                .filter_map(|pair| {
                    let (k, v) = pair.split_once('=')?;
                    Some((
                        k.to_string(),
                        urlencoding::decode(v).map(|s| s.into_owned()).unwrap_or_else(|_| v.to_string()),
                    ))
                })
                .collect();
            let err = params.iter().find(|(k, _)| k == "error").map(|(_, v)| v.clone());
            if let Some(err) = err {
                let html = callback_html("Connexion refusée", &escape_html(&err), false);
                let _ = request.respond(html_response(html));
                let _ = tx.send(Err(format!("Roblox: {err}")));
                break;
            }
            let code = params.iter().find(|(k, _)| k == "code").map(|(_, v)| v.clone());
            let got_state = params.iter().find(|(k, _)| k == "state").map(|(_, v)| v.clone());
            if got_state.as_deref() != Some(expected_state.as_str()) {
                let html = callback_html("État invalide", "Réessaie depuis Lumen.", false);
                let _ = request.respond(html_response(html));
                let _ = tx.send(Err("État OAuth invalide".into()));
                break;
            }
            if let Some(code) = code {
                let html = callback_html("Connecté", "Tu peux revenir à Lumen.", true);
                let _ = request.respond(html_response(html));
                let _ = tx.send(Ok((code, got_state.unwrap_or_default())));
            } else {
                let _ = request.respond(html_response(callback_html("Code manquant", "", false)));
                let _ = tx.send(Err("Code d’autorisation manquant".into()));
            }
            break;
        }
    });

    open_browser(&app, &url)?;
    let (code, _) = match rx.recv_timeout(Duration::from_secs(180)) {
        Ok(result) => result?,
        Err(_) => {
            let _ = http()?
                .get(format!("http://127.0.0.1:{AUTH_PORT}/cancel"))
                .send();
            return Err("Connexion expirée. Réessaie.".into());
        }
    };

    let client = http()?;
    let mut session = exchange_token(&client, &client_id, &code, &verifier)?;
    let user = fetch_user(&client, &session.access_token)?;
    persist_user(&user);
    session.user = Some(user.clone());
    save_session(&session)?;
    Ok(user)
}

fn open_browser(app: &tauri::AppHandle, url: &str) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("Impossible d’ouvrir le navigateur: {e}"))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn callback_html(title: &str, body: &str, ok: bool) -> String {
    format!(
        r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Lumen</title>
<style>
body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#f3eee4;color:#1c1712;font-family:Georgia,serif}}
.card{{background:#fffaf3;padding:36px 40px;border-radius:18px;max-width:28rem}}
h1{{font-size:28px;margin:0 0 8px}}
p{{color:#6b6156;line-height:1.5}}
.ok{{color:#355e49}}
</style></head><body><div class="card">
<h1 class="{class}">{title}</h1>
<p>{body}</p>
</div></body></html>"#,
        class = if ok { "ok" } else { "" }
    )
}

fn html_response(html: String) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(html).with_header(
        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap(),
    )
}
