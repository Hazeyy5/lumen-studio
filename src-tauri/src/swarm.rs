use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SwarmFile {
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub paused_at: Option<String>,
    #[serde(default)]
    pub agents: Vec<SwarmAgent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmAgent {
    pub local_id: String,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub resume_id: Option<String>,
}

pub fn swarm_path(project_path: &str) -> Result<PathBuf, String> {
    let dir = PathBuf::from(project_path);
    if !dir.join(".lumen.json").exists() {
        return Err("Ce n'est pas un projet Lumen".into());
    }
    Ok(dir.join(".lumen-swarm.json"))
}

#[tauri::command(async)]
pub fn load_swarm(project_path: String) -> Result<SwarmFile, String> {
    read_swarm(&project_path)
}

#[tauri::command(async)]
pub fn save_swarm(project_path: String, swarm: SwarmFile) -> Result<(), String> {
    write_swarm(&project_path, &swarm)
}

pub fn read_swarm(project_path: &str) -> Result<SwarmFile, String> {
    let path = swarm_path(project_path)?;
    if !path.exists() {
        return Ok(SwarmFile::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let raw = raw.trim_start_matches('\u{feff}');
    serde_json::from_str(raw).map_err(|e| e.to_string())
}

pub fn write_swarm(project_path: &str, swarm: &SwarmFile) -> Result<(), String> {
    let path = swarm_path(project_path)?;
    fs::write(
        path,
        serde_json::to_string_pretty(swarm).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn mark_paused(project_path: &str) {
    let mut swarm = read_swarm(project_path).unwrap_or_default();
    swarm.paused = true;
    swarm.paused_at = Some(now_secs());
    let _ = write_swarm(project_path, &swarm);
}

pub fn patch_resume_id(project_path: &str, local_id: &str, resume_id: &str) {
    let Ok(mut swarm) = read_swarm(project_path) else {
        return;
    };
    let mut changed = false;
    for agent in &mut swarm.agents {
        if agent.local_id == local_id && agent.resume_id.as_deref() != Some(resume_id) {
            agent.resume_id = Some(resume_id.to_string());
            changed = true;
        }
    }
    if changed {
        let _ = write_swarm(project_path, &swarm);
    }
}

pub fn upsert_agent(project_path: &str, agent: SwarmAgent, paused: bool) {
    let Ok(mut swarm) = read_swarm(project_path) else {
        return;
    };
    if let Some(existing) = swarm
        .agents
        .iter_mut()
        .find(|item| item.local_id == agent.local_id)
    {
        *existing = agent;
    } else {
        swarm.agents.push(agent);
    }
    swarm.paused = paused;
    let _ = write_swarm(project_path, &swarm);
}

fn now_secs() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

pub fn norm_path(path: &str) -> String {
    Path::new(path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}
