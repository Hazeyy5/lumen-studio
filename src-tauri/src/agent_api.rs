use serde::Serialize;
use std::thread;
use tauri::AppHandle;

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
