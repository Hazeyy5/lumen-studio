use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::assets::{generate_image_for_project, generate_mesh_for_project, SavedAsset};
use crate::blender::run_blender_mesh;
use crate::bank::{get_bank_item, search_library};
use crate::keys::load_keys;
use crate::projects::{list_reference_files, read_reference_file, references_of};
use crate::publish::publish_ref;
use crate::review::{
    present_library_choice, emit_progress, present_review, review_library_item, ReviewAction,
};

pub const PORT: u16 = 17422;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    ok: bool,
    gemini: bool,
    meshy: bool,
    tripo: bool,
    blender: bool,
    mesh_provider: String,
    roblox_publish: bool,
}

fn json_response(code: u16, body: String) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(body)
        .with_status_code(code)
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..])
                .unwrap(),
        )
}

fn error_json(code: u16, message: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    json_response(
        code,
        serde_json::json!({ "error": message }).to_string(),
    )
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct IconJob {
    id: String,
    project_path: String,
    model_path: String,
    zoom: f64,
    vertical: f64,
    horizontal: f64,
    outline: bool,
    color: String,
    thickness: f64,
    shadow: bool,
    opacity: f64,
    blur: f64,
    offset_y: f64,
    resolution: u32,
}

fn icon_waiters() -> &'static Mutex<HashMap<String, mpsc::Sender<Result<String, String>>>> {
    static WAITERS: OnceLock<Mutex<HashMap<String, mpsc::Sender<Result<String, String>>>>> =
        OnceLock::new();
    WAITERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn num_or(value: Option<&serde_json::Value>, fallback: f64) -> f64 {
    value.and_then(|v| v.as_f64()).unwrap_or(fallback)
}

fn resolve_icon_model(project: &str, input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Modèle manquant. Donne un fichier .glb ou un code VS- / LUM-.".into());
    }
    let direct = PathBuf::from(input);
    let candidate = if direct.is_file() {
        direct
    } else {
        let joined = PathBuf::from(project).join(input);
        if joined.is_file() {
            joined
        } else if input.contains('-') || input.chars().all(|c| c.is_ascii_alphanumeric()) {
            let item = get_bank_item(input)?;
            let item = crate::catalog::materialize(&item)?;
            let path = PathBuf::from(&item.path);
            if !path.is_file() {
                return Err("Ce modèle n’est pas un fichier local.".into());
            }
            path
        } else {
            return Err("Modèle introuvable. Donne un .glb du projet ou un code de la banque.".into());
        }
    };
    let ext = candidate
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "glb" && ext != "gltf" {
        return Err(
            "Le modèle vers icône lit un .glb ou un .gltf. Ce fichier a un autre format.".into(),
        );
    }
    Ok(candidate.to_string_lossy().into())
}

fn render_model_icon(app: &AppHandle, job: IconJob) -> Result<String, String> {
    let (tx, rx) = mpsc::channel();
    {
        let mut waiters = icon_waiters().lock().map_err(|e| e.to_string())?;
        if !waiters.is_empty() {
            return Err("Une capture d’icône est déjà en cours.".into());
        }
        waiters.insert(job.id.clone(), tx);
    }
    if let Err(err) = app.emit("lumen-icon", &job) {
        icon_waiters().lock().ok().map(|mut waiters| waiters.remove(&job.id));
        return Err(err.to_string());
    }
    match rx.recv_timeout(Duration::from_secs(70)) {
        Ok(result) => result,
        Err(_) => {
            icon_waiters().lock().ok().map(|mut waiters| waiters.remove(&job.id));
            Err("La capture a expiré. Laisse la fenêtre Lumen ouverte, pas réduite, puis réessaie.".into())
        }
    }
}

#[tauri::command]
pub fn complete_icon_job(
    id: String,
    error: Option<String>,
    result: Option<serde_json::Value>,
) -> Result<(), String> {
    let tx = icon_waiters()
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&id);
    let Some(tx) = tx else {
        return Ok(());
    };
    if let Some(message) = error.filter(|text| !text.trim().is_empty()) {
        let _ = tx.send(Err(message));
        return Ok(());
    }
    let Some(result) = result else {
        let _ = tx.send(Err("Capture vide".into()));
        return Ok(());
    };
    let _ = tx.send(Ok(result.to_string()));
    Ok(())
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            return Some(
                urlencoding::decode(v)
                    .map(|s| s.into_owned())
                    .unwrap_or_else(|_| v.to_string()),
            );
        }
    }
    None
}

fn generate_with_review(
    app: &AppHandle,
    kind: &str,
    project_path: &str,
    mut prompt: String,
) -> Result<SavedAsset, String> {
    loop {
        emit_progress(app, kind, &prompt);
        let asset = if kind == "image" {
            generate_image_for_project(project_path, &prompt)?
        } else {
            generate_mesh_for_project(project_path, &prompt)?
        };
        match present_review(app, &asset)? {
            ReviewAction::Approve => return Ok(asset),
            ReviewAction::Retry { prompt: next } => {
                prompt = next;
            }
            ReviewAction::Reject => {
                return Err("Asset refusé dans Lumen. Relance avec un autre prompt.".into());
            }
        }
    }
}

fn handle(app: AppHandle, mut request: tiny_http::Request) {
    let raw_url = request.url().to_string();
    let url = raw_url.split('?').next().unwrap_or("/").to_string();
    let method = request.method().clone();
    let mut body = String::new();
    let _ = std::io::Read::read_to_string(request.as_reader(), &mut body);

    if url == "/status" && method == tiny_http::Method::Get {
        let keys = match load_keys() {
            Ok(k) => k,
            Err(err) => {
                let _ = request.respond(error_json(500, &err));
                return;
            }
        };
        let payload = Status {
            ok: true,
            gemini: !keys.gemini.trim().is_empty(),
            meshy: !keys.meshy.trim().is_empty(),
            tripo: !keys.tripo.trim().is_empty(),
            blender: crate::blender::find_blender().is_some(),
            mesh_provider: if keys.mesh_provider.eq_ignore_ascii_case("tripo") {
                "tripo".into()
            } else if keys.mesh_provider.eq_ignore_ascii_case("blender") {
                "blender".into()
            } else {
                "meshy".into()
            },
            roblox_publish: crate::oauth::access_token().is_ok()
                || !keys.roblox_api_key.trim().is_empty(),
        };
        let _ = request.respond(json_response(
            200,
            serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into()),
        ));
        return;
    }

    if url == "/library" && method == tiny_http::Method::Get {
        let query = query_param(&raw_url, "q").unwrap_or_default();
        let kind = query_param(&raw_url, "kind");
        let purpose = query_param(&raw_url, "for")
            .or_else(|| query_param(&raw_url, "purpose"))
            .or_else(|| query_param(&raw_url, "pour"))
            .unwrap_or_default();
        match search_library(&query, kind.as_deref(), 10) {
            Ok(items) => {
                let chosen = present_library_choice(&app, &query, &purpose, &items)
                    .ok()
                    .flatten();
                let slim: Vec<serde_json::Value> = items
                    .iter()
                    .map(|item| {
                        serde_json::json!({
                            "code": item.code,
                            "name": item.name,
                            "kind": if crate::bank::is_inspiration(item) {
                                "inspiration".into()
                            } else if crate::bank::is_texture(item) {
                                "texture".into()
                            } else {
                                item.kind.clone()
                            },
                            "source": item.source,
                            "inspiration": crate::bank::is_inspiration(item),
                            "needsReview": true,
                        })
                    })
                    .collect();
                let _ = request.respond(json_response(
                    200,
                    serde_json::to_string(&serde_json::json!({
                        "ok": true,
                        "count": slim.len(),
                        "items": slim,
                        "chosen": chosen,
                    }))
                    .unwrap_or_else(|_| "{}".into()),
                ));
            }
            Err(err) => {
                let _ = request.respond(error_json(400, &err));
            }
        }
        return;
    }

    if url == "/propose" && method == tiny_http::Method::Post {
        let payload: serde_json::Value =
            serde_json::from_str(&body).unwrap_or_else(|_| serde_json::json!({}));
        let mut codes: Vec<String> = payload
            .get("codes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        if codes.is_empty() {
            if let Some(raw) = payload.get("codes").and_then(|v| v.as_str()) {
                codes = raw
                    .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
        let label = payload
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("proposition")
            .to_string();
        let purpose = payload
            .get("purpose")
            .or_else(|| payload.get("for"))
            .or_else(|| payload.get("pour"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut items = Vec::new();
        for code in &codes {
            if let Ok(item) = get_bank_item(code) {
                items.push(item);
            }
        }
        if items.is_empty() {
            let _ = request.respond(error_json(400, "Aucun code reconnu (ex. VS-2608)"));
            return;
        }
        items.truncate(10);
        let chosen = present_library_choice(&app, &label, &purpose, &items)
            .ok()
            .flatten();
        let slim: Vec<serde_json::Value> = items
            .iter()
            .map(|item| {
                serde_json::json!({
                    "code": item.code,
                    "name": item.name,
                    "kind": item.kind,
                    "shown": true,
                })
            })
            .collect();
        let _ = request.respond(json_response(
            200,
            serde_json::json!({
                "ok": true,
                "count": slim.len(),
                "items": slim,
                "chosen": chosen,
            })
            .to_string(),
        ));
        return;
    }

    if url == "/reference/status" && method == tiny_http::Method::Get {
        let from = query_param(&raw_url, "from").unwrap_or_default();
        match references_of(&from) {
            Ok(refs) => {
                let items: Vec<serde_json::Value> = refs
                    .into_iter()
                    .map(|p| serde_json::json!({ "name": p.name, "path": p.path }))
                    .collect();
                let _ = request.respond(json_response(
                    200,
                    serde_json::json!({
                        "ok": true,
                        "linked": !items.is_empty(),
                        "projects": items,
                    })
                    .to_string(),
                ));
            }
            Err(err) => {
                let _ = request.respond(error_json(400, &err));
            }
        }
        return;
    }

    if url == "/reference/files" && method == tiny_http::Method::Get {
        let from = query_param(&raw_url, "from").unwrap_or_default();
        let needle = query_param(&raw_url, "ref").unwrap_or_default();
        let prefix = query_param(&raw_url, "prefix").unwrap_or_default();
        match list_reference_files(&from, &needle, &prefix) {
            Ok((other, files)) => {
                let _ = request.respond(json_response(
                    200,
                    serde_json::json!({
                        "ok": true,
                        "name": other.name,
                        "path": other.path,
                        "files": files,
                    })
                    .to_string(),
                ));
            }
            Err(err) => {
                let _ = request.respond(error_json(400, &err));
            }
        }
        return;
    }

    if url == "/reference/file" && method == tiny_http::Method::Get {
        let from = query_param(&raw_url, "from").unwrap_or_default();
        let needle = query_param(&raw_url, "ref").unwrap_or_default();
        let rel = query_param(&raw_url, "path").unwrap_or_default();
        match read_reference_file(&from, &needle, &rel) {
            Ok((other, text)) => {
                let _ = request.respond(json_response(
                    200,
                    serde_json::json!({
                        "ok": true,
                        "name": other.name,
                        "path": rel,
                        "projectPath": other.path,
                        "text": text,
                    })
                    .to_string(),
                ));
            }
            Err(err) => {
                let _ = request.respond(error_json(400, &err));
            }
        }
        return;
    }

    if url == "/asset" && method == tiny_http::Method::Get {
        let code = query_param(&raw_url, "code").unwrap_or_default();
        let project = query_param(&raw_url, "project")
            .or_else(|| query_param(&raw_url, "projectPath"))
            .unwrap_or_default();
        match review_library_item(&app, &code) {
            Ok(item) => {
                let mut value = serde_json::to_value(&item).unwrap_or_else(|_| serde_json::json!({}));
                if crate::bank::is_inspiration(&item) {
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert("inspiration".into(), serde_json::json!(true));
                        obj.insert("publishSkipped".into(), serde_json::json!(true));
                        obj.remove("robloxAssetId");
                        if !project.trim().is_empty() {
                            match crate::bank::copy_inspiration_into_project(&item, &project) {
                                Ok((local_path, relative_path)) => {
                                    obj.insert("localPath".into(), serde_json::json!(local_path));
                                    obj.insert(
                                        "relativePath".into(),
                                        serde_json::json!(relative_path),
                                    );
                                }
                                Err(err) => {
                                    obj.insert("copyError".into(), serde_json::json!(err));
                                }
                            }
                        }
                    }
                    let _ = request.respond(json_response(
                        200,
                        value.to_string(),
                    ));
                    return;
                }
                if crate::uefn::is_uefn_project(&project) {
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert("uefn".into(), serde_json::json!(true));
                        obj.insert("publishSkipped".into(), serde_json::json!(true));
                        obj.remove("robloxAssetId");
                        match crate::uefn::copy_asset_for_uefn(&item, &project) {
                            Ok((local_path, relative_path)) => {
                                obj.insert("localPath".into(), serde_json::json!(local_path));
                                obj.insert("relativePath".into(), serde_json::json!(relative_path));
                            }
                            Err(err) => {
                                obj.insert("copyError".into(), serde_json::json!(err));
                            }
                        }
                    }
                    let _ = request.respond(json_response(200, value.to_string()));
                    return;
                }
                let published = if item
                    .roblox_asset_id
                    .as_ref()
                    .map(|id| id.trim().is_empty())
                    .unwrap_or(true)
                {
                    match publish_ref(&code) {
                        Ok(result) => {
                            let mut item = item;
                            item.roblox_asset_id = Some(result.asset_id);
                            item
                        }
                        Err(_) => item,
                    }
                } else {
                    item
                };
                let _ = request.respond(json_response(
                    200,
                    serde_json::to_string(&published).unwrap_or_else(|_| "{}".into()),
                ));
            }
            Err(err) => {
                let _ = request.respond(error_json(404, &err));
            }
        }
        return;
    }

    if url == "/publish" && method == tiny_http::Method::Post {
        let payload: serde_json::Value = serde_json::from_str(&body).unwrap_or_else(|_| serde_json::json!({}));
        let code = payload
            .get("code")
            .or_else(|| payload.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if code.is_empty() {
            let _ = request.respond(error_json(400, "code manquant (ex. LUM-0001)"));
            return;
        }
        if code.to_ascii_uppercase().starts_with("INS-") {
            let _ = request.respond(error_json(
                400,
                "Les images d’inspiration (INS-xxxx) ne se publient pas sur Roblox.",
            ));
            return;
        }
        if let Err(err) = review_library_item(&app, &code) {
            let _ = request.respond(error_json(400, &err));
            return;
        }
        match publish_ref(&code) {
            Ok(result) => {
                let _ = request.respond(json_response(
                    200,
                    serde_json::to_string(&result).unwrap_or_else(|_| "{}".into()),
                ));
            }
            Err(err) => {
                let _ = request.respond(error_json(400, &err));
            }
        }
        return;
    }

    if url == "/blender" && method == tiny_http::Method::Post {
        let payload: serde_json::Value =
            serde_json::from_str(&body).unwrap_or_else(|_| serde_json::json!({}));
        let project_path = payload
            .get("projectPath")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let title = payload
            .get("title")
            .or_else(|| payload.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("mesh")
            .trim()
            .to_string();
        let script = payload
            .get("script")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let script_path = payload
            .get("scriptPath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if project_path.is_empty() {
            let _ = request.respond(error_json(400, "projectPath manquant"));
            return;
        }
        emit_progress(&app, "mesh", &title);
        match run_blender_mesh(
            &project_path,
            &script,
            script_path.as_deref(),
            &title,
        ) {
            Ok(asset) => match present_review(&app, &asset) {
                Ok(ReviewAction::Approve) => {
                    let _ = request.respond(json_response(
                        200,
                        serde_json::to_string(&asset).unwrap_or_else(|_| "{}".into()),
                    ));
                }
                Ok(_) => {
                    let _ = request.respond(error_json(
                        400,
                        "Modèle Blender refusé dans Lumen. Corrige le script et relance.",
                    ));
                }
                Err(err) => {
                    let _ = request.respond(error_json(400, &err));
                }
            },
            Err(err) => {
                let _ = request.respond(error_json(400, &err));
            }
        }
        return;
    }

    if (url == "/image" || url == "/mesh") && method == tiny_http::Method::Post {
        let payload: serde_json::Value = serde_json::from_str(&body).unwrap_or_else(|_| serde_json::json!({}));
        let prompt = payload
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let project_path = payload
            .get("projectPath")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if prompt.is_empty() {
            let _ = request.respond(error_json(400, "prompt manquant"));
            return;
        }
        if project_path.is_empty() {
            let _ = request.respond(error_json(400, "projectPath manquant"));
            return;
        }
        let kind = if url == "/image" { "image" } else { "mesh" };
        match generate_with_review(&app, kind, &project_path, prompt) {
            Ok(asset) => {
                let _ = request.respond(json_response(
                    200,
                    serde_json::to_string(&asset).unwrap_or_else(|_| "{}".into()),
                ));
            }
            Err(err) => {
                let _ = request.respond(error_json(400, &err));
            }
        }
        return;
    }

    if url == "/icon" && method == tiny_http::Method::Post {
        let payload: serde_json::Value =
            serde_json::from_str(&body).unwrap_or_else(|_| serde_json::json!({}));
        let project_path = payload
            .get("projectPath")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let model = payload
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if project_path.is_empty() {
            let _ = request.respond(error_json(400, "projectPath manquant"));
            return;
        }
        if !Path::new(&project_path).is_dir() {
            let _ = request.respond(error_json(400, "Projet introuvable"));
            return;
        }
        let model_path = match resolve_icon_model(&project_path, &model) {
            Ok(path) => path,
            Err(err) => {
                let _ = request.respond(error_json(400, &err));
                return;
            }
        };
        let resolution = payload
            .get("resolution")
            .and_then(|v| v.as_u64())
            .unwrap_or(512)
            .clamp(256, 1024) as u32;
        let job = IconJob {
            id: Uuid::new_v4().to_string(),
            project_path,
            model_path,
            zoom: num_or(payload.get("zoom"), 1.0).clamp(0.6, 2.4),
            vertical: num_or(payload.get("vertical"), 0.0).clamp(-1.0, 1.0),
            horizontal: num_or(payload.get("horizontal"), 0.0).clamp(-1.0, 1.0),
            outline: payload.get("outline").and_then(|v| v.as_bool()).unwrap_or(true),
            color: payload
                .get("color")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with('#') && s.len() == 7)
                .unwrap_or("#111111")
                .to_string(),
            thickness: num_or(payload.get("thickness"), 4.0).clamp(0.0, 24.0),
            shadow: payload.get("shadow").and_then(|v| v.as_bool()).unwrap_or(true),
            opacity: num_or(payload.get("opacity"), 0.35).clamp(0.0, 1.0),
            blur: num_or(payload.get("blur"), 16.0).clamp(0.0, 40.0),
            offset_y: num_or(payload.get("offsetY"), 12.0).clamp(-40.0, 40.0),
            resolution,
        };
        emit_progress(&app, "image", "Icône 2D depuis le modèle");
        match render_model_icon(&app, job) {
            Ok(body) => {
                let _ = request.respond(json_response(200, body));
            }
            Err(err) => {
                let _ = request.respond(error_json(400, &err));
            }
        }
        return;
    }

    let _ = request.respond(json_response(200, "{\"ok\":true,\"service\":\"lumen-assets\"}".into()));
}

pub fn spawn_server(app: AppHandle) {
    thread::spawn(move || {
        let server = match tiny_http::Server::http(format!("127.0.0.1:{PORT}")) {
            Ok(s) => s,
            Err(_) => return,
        };
        for request in server.incoming_requests() {
            let app = app.clone();
            thread::spawn(move || handle(app, request));
        }
    });
}
