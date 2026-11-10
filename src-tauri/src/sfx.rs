//! Bruitages générés (`assets/sfx/<Nom>.sfx.json` + `<Nom>.wav`) : onglet Sons de Lumen et `tools/lumen-sfx.mjs`.

use base64::Engine;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Outil des agents (même synthèse que l'onglet Sons).
pub fn write_sfx_kit(dir: &Path) -> Result<(), String> {
    let tools = dir.join("tools");
    fs::create_dir_all(&tools).map_err(|e| e.to_string())?;
    fs::write(tools.join("lumen-sfx-core.mjs"), include_str!("../resources/lumen-sfx-core.mjs"))
        .map_err(|e| e.to_string())?;
    fs::write(tools.join("lumen-sfx.mjs"), include_str!("../resources/lumen-sfx.mjs")).map_err(|e| e.to_string())?;
    fs::create_dir_all(sfx_dir(dir)).map_err(|e| e.to_string())?;
    Ok(())
}

fn sfx_dir(project: &Path) -> PathBuf {
    project.join("assets").join("sfx")
}

fn valid_name(name: &str) -> Result<&str, String> {
    let ok = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(name)
    } else {
        Err(format!("Nom invalide « {name} » : lettres, chiffres et _, en commençant par une lettre."))
    }
}

fn short_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn read_ids(project: &Path) -> serde_json::Map<String, serde_json::Value> {
    fs::read_to_string(sfx_dir(project).join("ids.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SfxFile {
    pub name: String,
    pub text: String,
    pub modified: u64,
    pub wav_path: Option<String>,
    /// rbxassetid://… si publié (Roblox), chemin de l'asset importé (UEFN / Unreal).
    pub asset_id: Option<String>,
    /// Le WAV a changé depuis la publication.
    pub stale: bool,
}

#[tauri::command(async)]
pub fn list_sfx(project_path: String) -> Result<Vec<SfxFile>, String> {
    let project = Path::new(&project_path);
    let dir = sfx_dir(project);
    let mut ids = read_ids(project);
    // UEFN / Unreal : l'« id » est le chemin de l'asset importé dans le projet.
    for (k, v) in read_imported(project) {
        let asset = v.get("asset").and_then(|a| a.as_str()).unwrap_or("").to_string();
        let hash = v.get("hash").cloned().unwrap_or_default();
        ids.entry(k).or_insert(serde_json::json!({ "id": asset, "hash": hash }));
    }
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(out);
    };
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        let Some(name) = file.strip_suffix(".sfx.json") else {
            continue;
        };
        let text = fs::read_to_string(entry.path()).unwrap_or_default();
        let wav = dir.join(format!("{name}.wav"));
        let mut modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let wav_bytes = fs::read(&wav).ok();
        if let Some(t) = fs::metadata(&wav)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        {
            modified = modified.max(t.as_millis() as u64);
        }
        let entry_ids = ids.get(name);
        let asset_id = entry_ids
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let stale = match (&asset_id, &wav_bytes, entry_ids.and_then(|v| v.get("hash")).and_then(|v| v.as_str())) {
            (Some(_), Some(bytes), Some(h)) => short_hash(bytes) != h,
            _ => false,
        };
        out.push(SfxFile {
            name: name.to_string(),
            text,
            modified,
            wav_path: wav_bytes.map(|_| wav.to_string_lossy().into()),
            asset_id,
            stale,
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Enregistre un son généré dans l'onglet Sons (JSON + WAV rendu par l'app).
#[tauri::command(async)]
pub fn save_sfx(project_path: String, name: String, spec: String, wav_b64: String) -> Result<(), String> {
    let name = valid_name(name.trim())?;
    serde_json::from_str::<serde_json::Value>(&spec).map_err(|e| format!("JSON invalide : {e}"))?;
    let wav = base64::engine::general_purpose::STANDARD
        .decode(wav_b64.as_bytes())
        .map_err(|e| e.to_string())?;
    let dir = sfx_dir(Path::new(&project_path));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(format!("{name}.sfx.json")), spec).map_err(|e| e.to_string())?;
    fs::write(dir.join(format!("{name}.wav")), wav).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command(async)]
pub fn delete_sfx(project_path: String, name: String) -> Result<(), String> {
    let name = valid_name(name.trim())?;
    let dir = sfx_dir(Path::new(&project_path));
    let _ = fs::remove_file(dir.join(format!("{name}.sfx.json")));
    let _ = fs::remove_file(dir.join(format!("{name}.wav")));
    Ok(())
}

/// Publie le WAV sur Roblox (banque Lumen + Open Cloud) et note l'id dans assets/sfx/ids.json.
#[tauri::command(async)]
pub fn publish_sfx(project_path: String, name: String) -> Result<String, String> {
    let name = valid_name(name.trim())?;
    let project = Path::new(&project_path);
    let wav = sfx_dir(project).join(format!("{name}.wav"));
    let bytes = fs::read(&wav).map_err(|_| format!("Pas de WAV pour {name}"))?;
    let saved = crate::assets::upload_project_file(
        &project_path,
        &format!("assets/sfx/{name}.wav"),
        &format!("SFX {name}"),
    )?;
    let id = saved
        .roblox_asset_id
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| saved.publish_error.unwrap_or_else(|| "Roblox n'a pas renvoyé d'id".into()))?;
    let id = if id.starts_with("rbxassetid://") { id } else { format!("rbxassetid://{id}") };
    let mut ids = read_ids(project);
    ids.insert(name.to_string(), serde_json::json!({ "id": id, "hash": short_hash(&bytes) }));
    fs::write(
        sfx_dir(project).join("ids.json"),
        serde_json::to_string_pretty(&serde_json::Value::Object(ids)).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

// ---------------------------------------------------------------------------------------------
// UEFN / Unreal : les sons s'importent dans le projet par l'éditeur ouvert (pas de publication Roblox).

/// Kit son + skills audio et VFX propres au moteur, plus les notes LihFolk (principes communs).
pub fn write_engine_kit(dir: &Path, engine: &str) -> Result<(), String> {
    write_sfx_kit(dir)?;
    crate::projects::write_lihfolk_notes(dir)?;
    let (audio, vfx) = match engine {
        "uefn" => (
            include_str!("../resources/lumen-audio/SKILL-uefn.md"),
            include_str!("../resources/lumen-vfx/SKILL-uefn.md"),
        ),
        _ => (
            include_str!("../resources/lumen-audio/SKILL-unreal.md"),
            include_str!("../resources/lumen-vfx/SKILL-unreal.md"),
        ),
    };
    for agent in [".cursor", ".claude", ".codex", ".agents"] {
        for (name, body) in [("lumen-audio", audio), ("lumen-vfx", vfx)] {
            let skill_dir = dir.join(agent).join("skills").join(name);
            fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
            fs::write(skill_dir.join("SKILL.md"), body).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn read_imported(project: &Path) -> serde_json::Map<String, serde_json::Value> {
    fs::read_to_string(sfx_dir(project).join("imported.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn py_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// Commande envoyée à l'écouteur Python de UEFN (tools/uefn_listener.py, mêmes ports que le MCP).
fn uefn_python(code: &str) -> Result<serde_json::Value, String> {
    let status = crate::uefn::uefn_editor_status();
    let port = status
        .port
        .filter(|_| status.running)
        .ok_or("UEFN n'est pas ouvert (ou l'écouteur Lumen n'y tourne pas).")?;
    let body: serde_json::Value = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("http://127.0.0.1:{port}"))
        .json(&serde_json::json!({ "command": "execute_python", "params": { "code": code } }))
        .send()
        .map_err(|e| format!("UEFN : {e}"))?
        .json()
        .map_err(|e| e.to_string())?;
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        return Err(format!(
            "UEFN a refusé l'import : {}",
            body.get("error").and_then(|v| v.as_str()).unwrap_or("erreur inconnue")
        ));
    }
    let result = body.get("result").cloned().unwrap_or_default();
    if let Some(err) = result.get("stderr").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) {
        return Err(format!("UEFN : {err}"));
    }
    Ok(result)
}

fn import_uefn(project: &Path, wav: &Path) -> Result<String, String> {
    let module = fs::read_dir(project)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".uplugin")).map(String::from))
        .next()
        .ok_or("Pas de .uplugin dans le projet UEFN")?;
    let status = crate::uefn::uefn_editor_status();
    if let Some(open) = status.project_path.as_deref() {
        if !Path::new(open).eq(project) {
            return Err(format!("UEFN a un autre projet ouvert ({}). Ouvre celui-ci.", status.project.unwrap_or_default()));
        }
    }
    let dest = format!("/{module}/Lumen/Sfx");
    let code = format!(
        "import os\npath = {path}\nif not os.path.isfile(path):\n    raise FileNotFoundError(path)\ntask = unreal.AssetImportTask()\ntask.set_editor_property(\"filename\", path)\ntask.set_editor_property(\"destination_path\", {dest})\ntask.set_editor_property(\"automated\", True)\ntask.set_editor_property(\"save\", True)\ntask.set_editor_property(\"replace_existing\", True)\nunreal.AssetToolsHelpers.get_asset_tools().import_asset_tasks([task])\nresult = [str(p) for p in task.get_editor_property(\"imported_object_paths\")]\n",
        path = py_str(&wav.to_string_lossy().replace('\\', "/")),
        dest = py_str(&dest),
    );
    let result = uefn_python(&code)?;
    let paths: Vec<String> = result
        .get("result")
        .or(Some(&result))
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|p| p.as_str().map(String::from)).collect())
        .unwrap_or_default();
    paths.into_iter().next().ok_or_else(|| "UEFN n'a rien importé (format refusé ?)".into())
}

/// Appel d'un outil par le MCP de l'éditeur Unreal (HTTP « streamable » : JSON ou flux SSE).
fn unreal_mcp_call(toolset: &str, tool: &str, arguments: serde_json::Value) -> Result<serde_json::Value, String> {
    const URL: &str = "http://127.0.0.1:8000/mcp";
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let parse = |text: &str| -> Option<serde_json::Value> {
        if text.trim_start().starts_with('{') {
            serde_json::from_str(text.trim()).ok()
        } else {
            text.lines()
                .filter_map(|l| l.trim().strip_prefix("data:"))
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok())
                .find(|j| j.get("result").is_some() || j.get("error").is_some())
        }
    };
    let post = |body: serde_json::Value, session: Option<&str>| {
        let mut req = client
            .post(URL)
            .header("accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2025-06-18")
            .json(&body);
        if let Some(s) = session {
            req = req.header("Mcp-Session-Id", s);
        }
        req.send().map_err(|_| "L'éditeur Unreal n'est pas ouvert (ou son serveur MCP n'a pas démarré).".to_string())
    };
    let init = post(
        serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "lumen", "version": "1.0" } } }),
        None,
    )?;
    let session = init
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let _ = init.text();
    let _ = post(
        serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        session.as_deref(),
    )
    .map(|r| r.text());
    let res = post(
        serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": "call_tool",
            "arguments": { "toolset_name": toolset, "tool_name": tool, "arguments": arguments } } }),
        session.as_deref(),
    )?;
    let text = res.text().map_err(|e| e.to_string())?;
    let json = parse(&text).ok_or_else(|| format!("Réponse MCP illisible : {}", text.chars().take(300).collect::<String>()))?;
    if let Some(err) = json.get("error") {
        return Err(format!("Unreal : {err}"));
    }
    let result = json.get("result").cloned().unwrap_or_default();
    if result.get("isError").and_then(|v| v.as_bool()) == Some(true) {
        let msg = result
            .pointer("/content/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("erreur inconnue");
        return Err(format!("Unreal : {msg}"));
    }
    Ok(result)
}

fn import_unreal(name: &str) -> Result<String, String> {
    let result = unreal_mcp_call(
        "lumen_toolset.LumenTools",
        "import_file",
        serde_json::json!({ "source_file": format!("assets/sfx/{name}.wav"), "folder_path": "/Game/Lumen/Sfx", "asset_name": format!("SFX_{name}") }),
    )?;
    let text = result
        .pointer("/content/0/text")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| result.to_string());
    Ok(text
        .split(|c: char| c == '"' || c == '\'' || c == ',' || c == '[' || c == ']' || c.is_whitespace())
        .find(|s| s.starts_with("/Game/"))
        .map(String::from)
        .unwrap_or_else(|| format!("/Game/Lumen/Sfx/SFX_{name}")))
}

/// Importe le WAV dans le projet UEFN ou Unreal ouvert (Content/Lumen/Sfx) et le note dans assets/sfx/imported.json.
#[tauri::command(async)]
pub fn import_sfx(project_path: String, name: String) -> Result<String, String> {
    let name = valid_name(name.trim())?;
    let project = crate::blender::plain_path(&crate::assets::assert_lumen_project(&project_path)?);
    let wav = sfx_dir(&project).join(format!("{name}.wav"));
    let bytes = fs::read(&wav).map_err(|_| format!("Pas de WAV pour {name}"))?;
    let asset = if crate::uefn::is_uefn_project(&project_path) {
        import_uefn(&project, &wav)?
    } else if crate::unreal::is_unreal_project(&project_path) {
        import_unreal(name)?
    } else {
        return Err("Projet Roblox : utilise « Publier sur Roblox ».".into());
    };
    let mut imported = read_imported(&project);
    imported.insert(name.to_string(), serde_json::json!({ "asset": asset, "hash": short_hash(&bytes) }));
    fs::write(
        sfx_dir(&project).join("imported.json"),
        serde_json::to_string_pretty(&serde_json::Value::Object(imported)).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(asset)
}
