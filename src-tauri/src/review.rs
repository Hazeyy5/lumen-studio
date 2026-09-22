use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, UserAttentionType};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

use crate::assets::SavedAsset;
use crate::bank::{get_bank_item, BankItem};

#[derive(Debug, Clone)]
pub enum ReviewAction {
    Approve,
    Retry { prompt: String },
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPayload {
    pub id: String,
    pub status: String,
    pub kind: String,
    pub origin: String,
    pub name: String,
    pub prompt: String,
    pub code: Option<String>,
    pub path: String,
    pub preview_path: Option<String>,
    pub roblox_asset_id: Option<String>,
    pub can_retry: bool,
    pub error: Option<String>,
}

struct Pending {
    payload: ReviewPayload,
    tx: mpsc::Sender<ReviewAction>,
}

fn queue() -> &'static Mutex<HashMap<String, Pending>> {
    static QUEUE: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_queue() -> std::sync::MutexGuard<'static, HashMap<String, Pending>> {
    queue().lock().unwrap_or_else(|e| e.into_inner())
}

fn approved_codes() -> &'static Mutex<HashMap<String, Instant>> {
    static MAP: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

fn was_just_approved(code: &str) -> bool {
    let key = code.trim().to_ascii_lowercase();
    if key.is_empty() {
        return false;
    }
    let mut map = approved_codes().lock().unwrap_or_else(|e| e.into_inner());
    map.retain(|_, at| at.elapsed() < Duration::from_secs(300));
    map.contains_key(&key)
}

fn mark_approved(code: &str) {
    let key = code.trim().to_ascii_lowercase();
    if key.is_empty() {
        return;
    }
    approved_codes()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, Instant::now());
}

fn nudge_app(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.request_user_attention(Some(UserAttentionType::Informational));
    }
}

fn os_notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

pub fn emit_progress(app: &AppHandle, kind: &str, prompt: &str) {
    let payload = ReviewPayload {
        id: String::new(),
        status: "generating".into(),
        kind: kind.into(),
        origin: "generate".into(),
        name: prompt.into(),
        prompt: prompt.into(),
        code: None,
        path: String::new(),
        preview_path: None,
        roblox_asset_id: None,
        can_retry: true,
        error: None,
    };
    let _ = app.emit("asset-review", &payload);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryProposalItem {
    pub code: String,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub preview_path: Option<String>,
    #[serde(default)]
    pub roblox_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryProposal {
    pub id: String,
    pub query: String,
    #[serde(default)]
    pub purpose: String,
    pub items: Vec<LibraryProposalItem>,
}

struct PendingChoice {
    payload: LibraryProposal,
    tx: mpsc::Sender<Option<String>>,
}

fn choice_queue() -> &'static Mutex<HashMap<String, PendingChoice>> {
    static QUEUE: OnceLock<Mutex<HashMap<String, PendingChoice>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_choice() -> std::sync::MutexGuard<'static, HashMap<String, PendingChoice>> {
    choice_queue().lock().unwrap_or_else(|e| e.into_inner())
}

fn proposal_from_items(query: &str, purpose: &str, items: &[BankItem]) -> LibraryProposal {
    let shown: Vec<LibraryProposalItem> = items
        .iter()
        .take(10)
        .map(|item| {
            let preview = item.preview_path.clone().or_else(|| {
                let lower = item.path.to_ascii_lowercase();
                (lower.ends_with(".png")
                    || lower.ends_with(".jpg")
                    || lower.ends_with(".jpeg")
                    || lower.ends_with(".webp")
                    || lower.ends_with(".gif"))
                .then(|| item.path.clone())
            });
            LibraryProposalItem {
                code: item.code.clone(),
                name: item.name.clone(),
                kind: if crate::bank::is_inspiration(item) {
                    "inspiration".into()
                } else {
                    item.kind.clone()
                },
                path: item.path.clone(),
                preview_path: preview,
                roblox_asset_id: item.roblox_asset_id.clone(),
            }
        })
        .collect();
    let label = query.trim();
    let use_for = purpose.trim();
    LibraryProposal {
        id: Uuid::new_v4().to_string(),
        query: if label.is_empty() {
            "banque".into()
        } else {
            label.into()
        },
        purpose: use_for.into(),
        items: shown,
    }
}

pub fn present_library_choice(
    app: &AppHandle,
    query: &str,
    purpose: &str,
    items: &[BankItem],
) -> Result<Option<String>, String> {
    if items.is_empty() {
        return Ok(None);
    }
    let payload = proposal_from_items(query, purpose, items);
    let id = payload.id.clone();
    let (tx, rx) = mpsc::channel();
    lock_choice().insert(id, PendingChoice {
        payload: payload.clone(),
        tx,
    });
    let _ = app.emit("library-propose", &payload);
    nudge_app(app);
    let n = payload.items.len();
    let why = if payload.purpose.is_empty() {
        payload.query.clone()
    } else {
        payload.purpose.clone()
    };
    os_notify(
        app,
        "Lumen — choisis un asset",
        &format!("{n} proposition{} : {}", if n > 1 { "s" } else { "" }, why),
    );
    rx.recv()
        .map_err(|_| "Lumen a été fermé pendant le choix d’asset".into())
}

#[tauri::command]
pub fn pending_library_choices() -> Vec<LibraryProposal> {
    lock_choice()
        .values()
        .map(|pending| pending.payload.clone())
        .collect()
}

#[tauri::command]
pub fn resolve_library_choice(id: String, code: Option<String>) -> Result<(), String> {
    let pending = lock_choice()
        .remove(&id)
        .ok_or("Aucun choix d’asset en attente")?;
    let picked = code
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .filter(|s| {
            pending
                .payload
                .items
                .iter()
                .any(|item| item.code.eq_ignore_ascii_case(s))
        });
    pending
        .tx
        .send(picked)
        .map_err(|_| "L’agent n’attend plus ce choix".into())
}

fn wait_for_review(app: &AppHandle, payload: ReviewPayload) -> Result<ReviewAction, String> {
    let id = payload.id.clone();
    let (tx, rx) = mpsc::channel();
    lock_queue().insert(
        id,
        Pending {
            payload: payload.clone(),
            tx,
        },
    );
    let _ = app.emit("asset-review", &payload);
    nudge_app(app);
    let kind_label = if payload.kind == "mesh" {
        "mesh 3D"
    } else {
        "image"
    };
    let title = if payload.origin == "inspiration" {
        "Lumen — inspiration UI".into()
    } else if payload.origin == "library" {
        format!("Lumen — valider un {kind_label} de la banque")
    } else {
        format!("Lumen — valider ce {kind_label}")
    };
    let body = payload
        .code
        .as_deref()
        .filter(|c| !c.is_empty())
        .map(|c| format!("{c} · {}", payload.name))
        .unwrap_or_else(|| payload.name.clone());
    os_notify(app, &title, &body);
    rx.recv()
        .map_err(|_| "Lumen a été fermé pendant la validation de l’asset".into())
}

pub fn present_review(app: &AppHandle, asset: &SavedAsset) -> Result<ReviewAction, String> {
    let preview_path = asset.preview_path.clone().or_else(|| {
        asset
            .bank_id
            .as_ref()
            .and_then(|bank_id| get_bank_item(bank_id).ok())
            .and_then(|item| item.preview_path.or(Some(item.path)))
    });
    let payload = ReviewPayload {
        id: Uuid::new_v4().to_string(),
        status: "ready".into(),
        kind: asset.kind.clone(),
        origin: "generate".into(),
        name: asset.prompt.clone(),
        prompt: asset.prompt.clone(),
        code: asset.code.clone(),
        path: asset.path.clone(),
        preview_path,
        roblox_asset_id: asset.roblox_asset_id.clone(),
        can_retry: asset.can_retry,
        error: None,
    };
    wait_for_review(app, payload)
}

pub fn present_library_review(app: &AppHandle, item: &BankItem) -> Result<ReviewAction, String> {
    if was_just_approved(&item.code) {
        return Ok(ReviewAction::Approve);
    }
    let preview_path = item
        .preview_path
        .clone()
        .or_else(|| {
            let lower = item.path.to_ascii_lowercase();
            (lower.ends_with(".png")
                || lower.ends_with(".jpg")
                || lower.ends_with(".jpeg")
                || lower.ends_with(".webp"))
            .then(|| item.path.clone())
        });
    let inspiration = crate::bank::is_inspiration(item);
    let payload = ReviewPayload {
        id: Uuid::new_v4().to_string(),
        status: "ready".into(),
        kind: item.kind.clone(),
        origin: if inspiration {
            "inspiration".into()
        } else {
            "library".into()
        },
        name: if inspiration {
            format!("Inspiration · {}", item.name)
        } else {
            item.name.clone()
        },
        prompt: item.name.clone(),
        code: Some(item.code.clone()),
        path: item.path.clone(),
        preview_path,
        roblox_asset_id: item.roblox_asset_id.clone(),
        can_retry: false,
        error: None,
    };
    let action = wait_for_review(app, payload)?;
    if matches!(action, ReviewAction::Approve) {
        mark_approved(&item.code);
    }
    Ok(action)
}

pub fn review_library_item(app: &AppHandle, code: &str) -> Result<BankItem, String> {
    let item = get_bank_item(code)?;
    match present_library_review(app, &item)? {
        ReviewAction::Approve => Ok(get_bank_item(code).unwrap_or(item)),
        ReviewAction::Retry { .. } => Err("Asset banque refusé".into()),
        ReviewAction::Reject => Err(format!(
            "Asset {} refusé dans Lumen. Choisis-en un autre ou génère.",
            item.code
        )),
    }
}

#[tauri::command]
pub fn pending_asset_reviews() -> Vec<ReviewPayload> {
    lock_queue()
        .values()
        .map(|pending| pending.payload.clone())
        .collect()
}

#[tauri::command]
pub fn resolve_asset_review(
    id: String,
    action: String,
    prompt: Option<String>,
) -> Result<(), String> {
    let pending = lock_queue()
        .remove(&id)
        .ok_or("Aucune validation en attente pour cet asset")?;
    let decision = match action.as_str() {
        "retry" => {
            let next = prompt
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or(pending.payload.prompt);
            ReviewAction::Retry { prompt: next }
        }
        "reject" => ReviewAction::Reject,
        _ => ReviewAction::Approve,
    };
    if matches!(decision, ReviewAction::Approve) {
        if let Some(code) = pending.payload.code.as_deref() {
            mark_approved(code);
        }
    }
    pending
        .tx
        .send(decision)
        .map_err(|_| "L’agent n’attend plus cette validation".into())
}
