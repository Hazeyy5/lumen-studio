//! Miniatures des modèles 3D de la banque, rendues par Blender en arrière-plan.
//! Avant, chaque miniature était rendue dans l'interface (WebGL) et figeait Lumen.

use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

use crate::bank::BankItem;

const BATCH: usize = 16;

#[derive(Clone)]
struct Job {
    id: String,
    path: String,
}

#[derive(Default)]
struct State {
    queue: VecDeque<Job>,
    queued: HashSet<String>,
    failed: HashSet<String>,
    running: bool,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(State::default()))
}

fn lock() -> std::sync::MutexGuard<'static, State> {
    state().lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PreviewEvent {
    id: String,
    preview_path: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueStatus {
    pub blender: bool,
    pub queued: usize,
    pub left: usize,
}

fn script_path() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or("AppData Local introuvable")?
        .join("Lumen")
        .join("blender");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join("lumen-blender-thumbs.py");
    fs::write(&dest, include_str!("../resources/lumen-blender-thumbs.py")).map_err(|e| e.to_string())?;
    Ok(dest)
}

fn needs_preview(item: &BankItem) -> bool {
    item.kind == "mesh"
        && !item.path.starts_with("http")
        && Path::new(&item.path).is_file()
        && !item
            .preview_path
            .as_deref()
            .is_some_and(|p| p.starts_with("http") || Path::new(p).is_file())
}

/// Ajoute des modèles à la file. `front` : ceux de la page affichée passent devant le reste.
fn enqueue(app: &AppHandle, items: Vec<BankItem>, front: bool) -> QueueStatus {
    let blender = crate::blender::find_blender().is_some();
    let mut st = lock();
    let mut added = 0;
    if blender {
        let waiting: HashSet<String> = st.queue.iter().map(|j| j.id.clone()).collect();
        let fresh: Vec<Job> = items
            .into_iter()
            .filter(|item| {
                needs_preview(item)
                    && !st.failed.contains(&item.id)
                    // en cours de rendu (sorti de la file mais pas fini) : ne pas le relancer
                    && !(st.queued.contains(&item.id) && !waiting.contains(&item.id))
                    // déjà rendu, pas encore inscrit au catalogue (fin du lot)
                    && !crate::bank::preview_file(&item.id).is_ok_and(|p| p.is_file())
            })
            .map(|item| Job { id: item.id, path: item.path })
            .collect();
        if front {
            // Remonte aussi ceux déjà en file : la page affichée d'abord.
            let ids: HashSet<&str> = fresh.iter().map(|j| j.id.as_str()).collect();
            st.queue.retain(|j| !ids.contains(j.id.as_str()));
            for job in fresh.into_iter().rev() {
                st.queued.insert(job.id.clone());
                st.queue.push_front(job);
                added += 1;
            }
        } else {
            for job in fresh {
                if st.queued.insert(job.id.clone()) {
                    st.queue.push_back(job);
                    added += 1;
                }
            }
        }
        if !st.running && !st.queue.is_empty() {
            st.running = true;
            let app = app.clone();
            std::thread::spawn(move || worker(app));
        }
    }
    QueueStatus { blender, queued: added, left: st.queue.len() }
}

fn worker(app: AppHandle) {
    loop {
        let batch: Vec<Job> = {
            let mut st = lock();
            if st.queue.is_empty() {
                st.running = false;
                return;
            }
            let n = st.queue.len().min(BATCH);
            st.queue.drain(..n).collect()
        };
        let done = run_batch(&app, &batch);
        let mut st = lock();
        for job in &batch {
            st.queued.remove(&job.id);
        }
        // Blender a planté en cours de lot : le premier modèle non rendu est sans doute le fautif.
        let missing: Vec<&Job> = batch.iter().filter(|j| !done.contains(&j.id)).collect();
        if let Some(first) = missing.first() {
            st.failed.insert(first.id.clone());
            for job in missing.iter().skip(1).rev() {
                if !st.failed.contains(&job.id) && st.queued.insert(job.id.clone()) {
                    st.queue.push_front((*job).clone());
                }
            }
        }
        let left = st.queue.len();
        drop(st);
        let _ = app.emit("mesh-preview-progress", left);
    }
}

/// Rend un lot ; renvoie les ids traités (réussis ou refusés proprement par le script).
fn run_batch(app: &AppHandle, batch: &[Job]) -> HashSet<String> {
    let mut handled = HashSet::new();
    let (Some(blender), Ok(script)) = (crate::blender::find_blender(), script_path()) else {
        return batch.iter().map(|j| j.id.clone()).collect();
    };
    let jobs: Vec<serde_json::Value> = batch
        .iter()
        .filter_map(|job| {
            let out = crate::bank::preview_file(&job.id).ok()?;
            Some(serde_json::json!({ "id": job.id, "path": job.path, "out": out.to_string_lossy() }))
        })
        .collect();
    let Ok(dir) = dirs::data_local_dir()
        .ok_or(())
        .map(|d| d.join("Lumen").join("blender").join("thumbs"))
    else {
        return handled;
    };
    let _ = fs::create_dir_all(&dir);
    let jobs_file = dir.join(format!("jobs-{}.json", std::process::id()));
    if fs::write(&jobs_file, serde_json::Value::Array(jobs).to_string()).is_err() {
        return handled;
    }

    let mut cmd = Command::new(blender);
    cmd.arg("--background")
        .arg("--factory-startup")
        .arg("--python")
        .arg(&script)
        .arg("--")
        .arg("--jobs")
        .arg(&jobs_file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000; // ne ralentit pas le reste du PC
        cmd.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    let Ok(mut child) = cmd.spawn() else {
        return batch.iter().map(|j| j.id.clone()).collect();
    };
    let mut attached = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let mut parts = line.splitn(3, '\t');
            match (parts.next(), parts.next(), parts.next()) {
                (Some("LUMEN_THUMB_OK"), Some(id), Some(path)) => {
                    handled.insert(id.to_string());
                    let path = PathBuf::from(path).to_string_lossy().replace('/', "\\");
                    attached.push((id.to_string(), path.clone()));
                    let _ = app.emit("mesh-preview", PreviewEvent { id: id.to_string(), preview_path: path });
                }
                (Some("LUMEN_THUMB_FAIL"), Some(id), _) => {
                    handled.insert(id.to_string());
                    lock().failed.insert(id.to_string());
                }
                _ => {}
            }
        }
    }
    let _ = child.wait();
    let _ = fs::remove_file(&jobs_file);
    let _ = crate::bank::attach_previews(&attached);
    handled
}

/// Miniatures des modèles affichés (ids), rendues en priorité.
#[tauri::command(async)]
pub fn queue_mesh_previews(app: AppHandle, ids: Vec<String>) -> Result<QueueStatus, String> {
    let wanted: HashSet<String> = ids.into_iter().collect();
    let mut items: Vec<BankItem> = crate::bank::load_index()?
        .into_iter()
        .filter(|item| wanted.contains(&item.id))
        .collect();
    let found: HashSet<String> = items.iter().map(|i| i.id.clone()).collect();
    let others: HashSet<String> = wanted.difference(&found).cloned().collect();
    if !others.is_empty() {
        items.extend(crate::catalog::find_by_ids(&others)?);
    }
    Ok(enqueue(&app, items, true))
}

/// Toute la banque VibeStarter, en fond, après la page affichée.
#[tauri::command(async)]
pub fn queue_all_mesh_previews(app: AppHandle) -> Result<QueueStatus, String> {
    let mut items = crate::catalog::meshes_without_preview()?;
    items.extend(crate::bank::load_index()?.into_iter().filter(|i| i.kind == "mesh"));
    Ok(enqueue(&app, items, false))
}
