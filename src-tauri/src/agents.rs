use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::bins::find_binary;
use crate::swarm::{self, SwarmAgent};

pub struct AgentSession {
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub _master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    pub project_path: String,
    pub kind: String,
    pub local_id: String,
    pub resume_id: String,
}

pub type SessionMap = Arc<Mutex<HashMap<String, AgentSession>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub id: String,
    pub label: String,
    pub found: bool,
    pub path: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChunk {
    pub session_id: String,
    pub data: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedAgent {
    pub session_id: String,
    pub resume_id: String,
    pub local_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAgent {
    pub session_id: String,
    pub local_id: String,
    pub kind: String,
    pub resume_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResume {
    pub session_id: String,
    pub local_id: String,
    pub resume_id: String,
}

#[tauri::command]
pub fn detect_agents() -> Vec<AgentStatus> {
    vec![
        status("claude", "Claude Code", &["claude"]),
        status("codex", "Codex", &["codex"]),
        status("cursor", "Cursor", &["cursor-agent", "agent"]),
        status("antigravity", "Antigravity", &["agy", "antigravity"]),
    ]
}

fn status(id: &str, label: &str, bins: &[&str]) -> AgentStatus {
    match find_binary(bins) {
        Some(path) => AgentStatus {
            id: id.into(),
            label: label.into(),
            found: true,
            path: Some(path.to_string_lossy().into()),
        },
        None => AgentStatus {
            id: id.into(),
            label: label.into(),
            found: false,
            path: None,
        },
    }
}

#[tauri::command]
pub fn start_agent(
    app: AppHandle,
    sessions: tauri::State<SessionMap>,
    kind: String,
    project_path: String,
    resume_id: Option<String>,
    local_id: Option<String>,
    resume: Option<bool>,
) -> Result<StartedAgent, String> {
    let local_id = local_id.filter(|id| !id.is_empty()).unwrap_or_else(|| Uuid::new_v4().to_string());
    let incoming_resume = resume_id.filter(|id| !id.trim().is_empty());
    let resuming = resume.unwrap_or(false) || incoming_resume.is_some();
    let short = local_id.chars().take(8).collect::<String>();
    let display_name = format!("lumen-{kind}-{short}");

    let assigned_resume = if let Some(id) = incoming_resume.clone() {
        id
    } else if kind == "claude" && !resuming {
        Uuid::new_v4().to_string()
    } else {
        String::new()
    };

    let (program, args) = match kind.as_str() {
        "claude" => {
            let bin = find_binary(&["claude"]).ok_or("Claude Code n'est pas installé")?;
            let mut args = Vec::new();
            if let Ok(refs) = crate::projects::references_of(&project_path) {
                for other in refs {
                    args.push("--add-dir".into());
                    args.push(other.path);
                }
            }
            if resuming {
                args.push("--resume".into());
                if !assigned_resume.is_empty() {
                    args.push(assigned_resume.clone());
                }
            } else {
                args.push("--session-id".into());
                args.push(assigned_resume.clone());
            }
            args.push("--name".into());
            args.push(display_name);
            (bin, args)
        }
        "codex" => {
            let bin = find_binary(&["codex"]).ok_or("Codex n'est pas installé")?;
            let args = if resuming {
                if let Some(id) = incoming_resume.as_ref() {
                    vec!["resume".into(), id.clone()]
                } else {
                    vec!["resume".into(), "--last".into()]
                }
            } else {
                Vec::new()
            };
            (bin, args)
        }
        "cursor" => {
            let mut extra = vec![
                "--trust".into(),
                "--workspace".into(),
                project_path.clone(),
            ];
            if let Some(id) = incoming_resume.as_ref() {
                extra.push("--resume".into());
                extra.push(id.clone());
            } else if resuming {
                extra.push("--continue".into());
            }
            cursor_command(&project_path, &extra)?
        }
        "antigravity" => {
            let bin = find_binary(&["agy", "antigravity"])
                .ok_or("Antigravity n'est pas installé. Va dans Réglages.")?;
            ensure_agy_auto_approve();
            let mut args = vec!["--dangerously-skip-permissions".into()];
            if let Ok(refs) = crate::projects::references_of(&project_path) {
                for other in refs {
                    args.push("--add-dir".into());
                    args.push(other.path);
                }
            }
            if !assigned_resume.is_empty() {
                args.push("--conversation".into());
                args.push(assigned_resume.clone());
            } else if resuming {
                args.push("--continue".into());
            }
            (bin, args)
        }
        other => return Err(format!("Agent inconnu: {other}")),
    };

    let known_cursor = if kind == "cursor" && incoming_resume.is_none() {
        cursor_chat_ids(&project_path)
    } else {
        HashSet::new()
    };
    let known_codex = if kind == "codex" && incoming_resume.is_none() {
        codex_session_ids(&project_path)
    } else {
        HashSet::new()
    };
    let known_agy = if kind == "antigravity" && incoming_resume.is_none() {
        agy_conversation_ids(&project_path)
    } else {
        HashSet::new()
    };
    let started_ms = now_ms();

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 32,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let mut cmd = CommandBuilder::new(program);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.cwd(&project_path);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("FORCE_COLOR", "3");
    cmd.env("TERM_PROGRAM", "Lumen");
    cmd.env("LUMEN_ASSET_URL", "http://127.0.0.1:17422");
    cmd.env("LUMEN_PROJECT", &project_path);
    if let Ok(refs) = crate::projects::references_of(&project_path) {
        if !refs.is_empty() {
            let names: Vec<String> = refs.iter().map(|p| p.name.clone()).collect();
            let paths: Vec<String> = refs.iter().map(|p| p.path.clone()).collect();
            cmd.env("LUMEN_REF_NAME", names.join("|"));
            cmd.env("LUMEN_REF_PROJECT", paths.join("|"));
        }
    }
    let _ = crate::projects::ensure_agent_bridge(&project_path);
    if kind == "cursor" {
        cmd.env("CURSOR_INVOKED_AS", "agent");
        if let Ok(keys) = crate::keys::load_keys() {
            if !keys.cursor.is_empty() {
                cmd.env("CURSOR_API_KEY", keys.cursor);
            }
        }
        if let Some(root) = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent() {
            cmd.env("NODE_PATH", root.join("node_modules"));
        }
    }

    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let session_id = Uuid::new_v4().to_string();
    let emit_id = session_id.clone();
    let app_clone = app.clone();

    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_clone.emit(
                        "agent-chunk",
                        AgentChunk {
                            session_id: emit_id.clone(),
                            data,
                        },
                    );
                }
                Err(_) => break,
            }
        }
        let _ = app_clone.emit(
            "agent-exit",
            AgentChunk {
                session_id: emit_id,
                data: String::new(),
            },
        );
    });

    sessions
        .lock()
        .map_err(|e| e.to_string())?
        .insert(
            session_id.clone(),
            AgentSession {
                writer: Arc::new(Mutex::new(writer)),
                _master: pair.master,
                child,
                project_path: project_path.clone(),
                kind: kind.clone(),
                local_id: local_id.clone(),
                resume_id: assigned_resume.clone(),
            },
        );

    swarm::upsert_agent(
        &project_path,
        SwarmAgent {
            local_id: local_id.clone(),
            kind: kind.clone(),
            title: label_for(&kind).into(),
            resume_id: if assigned_resume.is_empty() {
                None
            } else {
                Some(assigned_resume.clone())
            },
        },
        false,
    );

    if assigned_resume.is_empty() && matches!(kind.as_str(), "cursor" | "codex" | "antigravity") {
        let app_discover = app.clone();
        let sessions_map = sessions.inner().clone();
        let session_id_d = session_id.clone();
        let local_id_d = local_id.clone();
        let kind_d = kind.clone();
        let project_d = project_path.clone();
        thread::spawn(move || {
            for delay in [800, 1600, 2800, 5000, 9000] {
                thread::sleep(Duration::from_millis(delay));
                let found = match kind_d.as_str() {
                    "cursor" => newest_cursor_chat(&project_d, started_ms, &known_cursor),
                    "codex" => newest_codex_session(&project_d, started_ms, &known_codex),
                    "antigravity" => newest_agy_conversation(&project_d, &known_agy),
                    _ => None,
                };
                if let Some(resume) = found {
                    if let Ok(mut map) = sessions_map.lock() {
                        if let Some(session) = map.get_mut(&session_id_d) {
                            session.resume_id = resume.clone();
                        }
                    }
                    swarm::patch_resume_id(&project_d, &local_id_d, &resume);
                    let _ = app_discover.emit(
                        "agent-resume-id",
                        AgentResume {
                            session_id: session_id_d,
                            local_id: local_id_d,
                            resume_id: resume,
                        },
                    );
                    break;
                }
            }
        });
    }

    Ok(StartedAgent {
        session_id,
        resume_id: assigned_resume,
        local_id,
    })
}

fn label_for(kind: &str) -> &'static str {
    match kind {
        "claude" => "Claude Code",
        "codex" => "Codex",
        "cursor" => "Cursor",
        "antigravity" => "Antigravity",
        _ => "Agent",
    }
}

fn cursor_command(project_path: &str, extra: &[String]) -> Result<(PathBuf, Vec<String>), String> {
    if let Some((program, mut args)) = cursor_native_cli() {
        args.extend(extra.iter().cloned());
        return Ok((program, args));
    }
    if let Some(bin) = find_binary(&["cursor-agent", "agent"]) {
        return Ok((bin, extra.to_vec()));
    }
    let node = find_binary(&["node"]).ok_or(
        "Cursor CLI introuvable. Installe Cursor CLI, ou Node.js pour le pont SDK.",
    )?;
    let bridge = cursor_bridge_path()?;
    Ok((
        node,
        vec![
            bridge.to_string_lossy().into_owned(),
            project_path.to_string(),
        ],
    ))
}

fn cursor_native_cli() -> Option<(PathBuf, Vec<String>)> {
    let versions = dirs::data_local_dir()?.join("cursor-agent").join("versions");
    if !versions.is_dir() {
        return None;
    }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&versions)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir() && path.join("node.exe").exists() && path.join("index.js").exists())
        .collect();
    dirs.sort();
    let latest = dirs.pop()?;
    Some((
        latest.join("node.exe"),
        vec![latest.join("index.js").to_string_lossy().into_owned()],
    ))
}

fn cursor_bridge_path() -> Result<PathBuf, String> {
    let resource = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/cursor-bridge.mjs");
    if resource.exists() {
        return Ok(resource);
    }
    Err("Pont Cursor manquant (resources/cursor-bridge.mjs)".into())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn cursor_chats_root() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".cursor").join("chats"))
}

fn cursor_chats_for_project(project_path: &str) -> Vec<(String, u128)> {
    let Some(root) = cursor_chats_root() else {
        return Vec::new();
    };
    if !root.is_dir() {
        return Vec::new();
    }
    let want = swarm::norm_path(project_path);
    let mut out = Vec::new();
    let Ok(workspaces) = std::fs::read_dir(root) else {
        return out;
    };
    for workspace in workspaces.flatten() {
        let ws_path = workspace.path();
        if !ws_path.is_dir() {
            continue;
        }
        let Ok(chats) = std::fs::read_dir(&ws_path) else {
            continue;
        };
        for chat in chats.flatten() {
            let chat_dir = chat.path();
            let meta = chat_dir.join("meta.json");
            let Ok(raw) = std::fs::read_to_string(&meta) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            let cwd = value
                .get("cwd")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if swarm::norm_path(cwd) != want {
                continue;
            }
            let created = value
                .get("createdAtMs")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u128;
            if let Some(id) = chat_dir.file_name().and_then(|n| n.to_str()) {
                out.push((id.to_string(), created));
            }
        }
    }
    out
}

fn cursor_chat_ids(project_path: &str) -> HashSet<String> {
    cursor_chats_for_project(project_path)
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

fn newest_cursor_chat(
    project_path: &str,
    started_ms: u128,
    known: &HashSet<String>,
) -> Option<String> {
    let mut chats = cursor_chats_for_project(project_path);
    chats.retain(|(id, created)| !known.contains(id) && *created + 50 >= started_ms.saturating_sub(4000));
    chats.sort_by_key(|(_, created)| *created);
    chats.pop().map(|(id, _)| id)
}

fn codex_home() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".codex"))
}

fn codex_sessions_for_project(project_path: &str) -> Vec<(String, u128)> {
    let Some(root) = codex_home().map(|h| h.join("sessions")) else {
        return Vec::new();
    };
    if !root.is_dir() {
        return Vec::new();
    }
    let want = swarm::norm_path(project_path);
    let mut out = Vec::new();
    visit_jsonl(&root, &mut |path| {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return;
        };
        let Some(first) = raw.lines().next() else {
            return;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(first) else {
            return;
        };
        let payload = value.get("payload").unwrap_or(&value);
        let cwd = payload
            .get("cwd")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if swarm::norm_path(cwd) != want {
            return;
        }
        let id = payload
            .get("session_id")
            .or_else(|| payload.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if id.is_empty() {
            return;
        }
        let created = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis())
            .unwrap_or(0);
        out.push((id.to_string(), created));
    });
    out
}

fn visit_jsonl(dir: &Path, on_file: &mut impl FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_jsonl(&path, on_file);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            on_file(&path);
        }
    }
}

fn codex_session_ids(project_path: &str) -> HashSet<String> {
    codex_sessions_for_project(project_path)
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

fn newest_codex_session(
    project_path: &str,
    started_ms: u128,
    known: &HashSet<String>,
) -> Option<String> {
    let mut sessions = codex_sessions_for_project(project_path);
    sessions.retain(|(id, created)| !known.contains(id) && *created + 50 >= started_ms.saturating_sub(4000));
    sessions.sort_by_key(|(_, created)| *created);
    sessions.pop().map(|(id, _)| id)
}

fn ensure_agy_auto_approve() {
    let Some(path) = dirs::home_dir().map(|home| {
        home.join(".gemini")
            .join("antigravity-cli")
            .join("settings.json")
    }) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = value.as_object_mut() {
        obj.insert("toolPermission".into(), serde_json::json!("always-proceed"));
        obj.insert(
            "artifactReviewPolicy".into(),
            serde_json::json!("always-proceed"),
        );
    }
    if let Ok(raw) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(path, raw);
    }
}

fn agy_last_conversations_path() -> Option<PathBuf> {
    Some(
        dirs::home_dir()?
            .join(".gemini")
            .join("antigravity-cli")
            .join("cache")
            .join("last_conversations.json"),
    )
}

fn agy_conversation_for_project(project_path: &str) -> Option<String> {
    let path = agy_last_conversations_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let map: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let want = swarm::norm_path(project_path);
    map.as_object()?
        .iter()
        .find(|(key, _)| swarm::norm_path(key) == want)
        .and_then(|(_, value)| value.as_str().map(|s| s.to_string()))
}

fn agy_conversation_ids(project_path: &str) -> HashSet<String> {
    agy_conversation_for_project(project_path)
        .into_iter()
        .collect()
}

fn newest_agy_conversation(project_path: &str, known: &HashSet<String>) -> Option<String> {
    let id = agy_conversation_for_project(project_path)?;
    if known.contains(&id) {
        None
    } else {
        Some(id)
    }
}

#[tauri::command]
pub fn live_agents(
    sessions: tauri::State<SessionMap>,
    project_path: String,
) -> Result<Vec<LiveAgent>, String> {
    let map = sessions.lock().map_err(|e| e.to_string())?;
    let want = swarm::norm_path(&project_path);
    Ok(map
        .iter()
        .filter(|(_, session)| swarm::norm_path(&session.project_path) == want)
        .map(|(id, session)| LiveAgent {
            session_id: id.clone(),
            local_id: session.local_id.clone(),
            kind: session.kind.clone(),
            resume_id: session.resume_id.clone(),
        })
        .collect())
}

#[tauri::command]
pub fn write_agent(
    sessions: tauri::State<SessionMap>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let map = sessions.lock().map_err(|e| e.to_string())?;
    let session = map.get(&session_id).ok_or("Session agent fermée")?;
    let mut writer = session.writer.lock().map_err(|e| e.to_string())?;
    writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resize_agent(
    sessions: tauri::State<SessionMap>,
    session_id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let map = sessions.lock().map_err(|e| e.to_string())?;
    let session = map.get(&session_id).ok_or("Session agent fermée")?;
    session
        ._master
        .resize(PtySize {
            rows: rows.max(2),
            cols: cols.max(8),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_agent(
    sessions: tauri::State<SessionMap>,
    session_id: String,
) -> Result<(), String> {
    let mut map = sessions.lock().map_err(|e| e.to_string())?;
    if let Some(session) = map.remove(&session_id) {
        drop(map);
        interrupt_then_kill(session);
    }
    Ok(())
}

#[tauri::command]
pub fn pause_project(
    sessions: tauri::State<SessionMap>,
    project_path: String,
) -> Result<(), String> {
    pause_project_inner(&sessions, &project_path)
}

fn pause_project_inner(sessions: &SessionMap, project_path: &str) -> Result<(), String> {
    let want = swarm::norm_path(project_path);
    let mut map = sessions.lock().map_err(|e| e.to_string())?;
    let ids: Vec<String> = map
        .iter()
        .filter(|(_, session)| swarm::norm_path(&session.project_path) == want)
        .map(|(id, _)| id.clone())
        .collect();
    let stopped: Vec<AgentSession> = ids
        .into_iter()
        .filter_map(|id| map.remove(&id))
        .collect();
    drop(map);
    interrupt_all(stopped);
    swarm::mark_paused(project_path);
    Ok(())
}

pub fn pause_all_on_exit(app: &AppHandle) {
    let Some(sessions) = app.try_state::<SessionMap>() else {
        return;
    };
    let Ok(mut map) = sessions.lock() else {
        return;
    };
    let mut projects: HashSet<String> = HashSet::new();
    let mut stopped = Vec::new();
    for (_, session) in map.drain() {
        projects.insert(session.project_path.clone());
        stopped.push(session);
    }
    drop(map);
    interrupt_all(stopped);
    for project in projects {
        swarm::mark_paused(&project);
    }
}

fn interrupt_then_kill(session: AgentSession) {
    interrupt_all(vec![session]);
}

fn interrupt_all(sessions: Vec<AgentSession>) {
    for session in &sessions {
        if let Ok(mut writer) = session.writer.lock() {
            let _ = writer.write_all(&[0x03]);
            let _ = writer.flush();
        }
    }
    if !sessions.is_empty() {
        thread::sleep(Duration::from_millis(280));
    }
    for mut session in sessions {
        let _ = session.child.kill();
    }
}
