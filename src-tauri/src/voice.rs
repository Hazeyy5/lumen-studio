//! Dictée vocale locale : whisper.cpp (whisper-cli) + modèle large-v3-turbo, téléchargés au premier usage
//! dans %LOCALAPPDATA%\Lumen\whisper. Rien ne sort du PC une fois installé.
use base64::Engine;
use serde::Serialize;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter};

const MODEL_FILE: &str = "ggml-large-v3-turbo-q5_0.bin";
const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin";
const MODEL_MIN_BYTES: u64 = 500_000_000;
const BIN_ASSET: &str = "whisper-bin-x64.zip";
/// Vocabulaire qui oriente la transcription (noms propres et termes techniques du projet).
const VOCAB: &str = "Lumen, Roblox, Roblox Studio, roblox-ts, Rojo, UEFN, Unreal, Verse, Fortnite, Blender, Figma, \
ParticleEmitter, Beam, Trail, VFX, tycoon, obby, golem, Claude, Codex, Cursor.";

fn whisper_dir() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or("AppData Local introuvable")?
        .join("Lumen")
        .join("whisper");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn cli_path(dir: &Path) -> PathBuf {
    dir.join("whisper-cli.exe")
}

fn model_ready(dir: &Path) -> bool {
    fs::metadata(dir.join(MODEL_FILE))
        .map(|m| m.len() >= MODEL_MIN_BYTES)
        .unwrap_or(false)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    ready: bool,
    engine: bool,
    model: bool,
}

#[tauri::command]
pub fn voice_status() -> Result<VoiceStatus, String> {
    let dir = whisper_dir()?;
    let engine = cli_path(&dir).exists();
    let model = model_ready(&dir);
    Ok(VoiceStatus { ready: engine && model, engine, model })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    stage: &'static str,
    done: u64,
    total: u64,
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("Lumen")
        .timeout(None)
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

fn install_engine(app: &AppHandle, dir: &Path) -> Result<(), String> {
    let http = client()?;
    let _ = app.emit("voice-install-progress", InstallProgress { stage: "engine", done: 0, total: 0 });
    let releases: serde_json::Value = http
        .get("https://api.github.com/repos/ggml-org/whisper.cpp/releases?per_page=15")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("GitHub whisper.cpp : {e}"))?
        .json()
        .map_err(|e| e.to_string())?;
    let url = releases
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|r| r["assets"].as_array().cloned().unwrap_or_default())
        .find(|a| a["name"].as_str() == Some(BIN_ASSET))
        .and_then(|a| a["browser_download_url"].as_str().map(String::from))
        .ok_or("Aucune release whisper.cpp avec whisper-bin-x64.zip")?;
    let bytes = http
        .get(&url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Téléchargement whisper.cpp : {e}"))?
        .bytes()
        .map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut found = false;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().replace('\\', "/");
        let base = name.rsplit('/').next().unwrap_or("").to_string();
        let keep = base == "whisper-cli.exe" || base.to_ascii_lowercase().ends_with(".dll");
        if !keep || file.is_dir() {
            continue;
        }
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        fs::write(dir.join(&base), buf).map_err(|e| e.to_string())?;
        found |= base == "whisper-cli.exe";
    }
    if !found {
        return Err("whisper-cli.exe absent de l'archive whisper.cpp".into());
    }
    Ok(())
}

fn install_model(app: &AppHandle, dir: &Path) -> Result<(), String> {
    let http = client()?;
    let mut resp = http
        .get(MODEL_URL)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Téléchargement du modèle : {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    let part = dir.join(format!("{MODEL_FILE}.part"));
    let mut out = fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    let mut last = 0u64;
    loop {
        let n = resp.read(&mut buf).map_err(|e| format!("Téléchargement du modèle : {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if done - last >= 4 << 20 {
            last = done;
            let _ = app.emit("voice-install-progress", InstallProgress { stage: "model", done, total });
        }
    }
    drop(out);
    if done < MODEL_MIN_BYTES || (total > 0 && done != total) {
        let _ = fs::remove_file(&part);
        return Err("Modèle incomplet, réessaie.".into());
    }
    fs::rename(&part, dir.join(MODEL_FILE)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Télécharge ce qui manque (moteur ~20 Mo, modèle ~550 Mo) en émettant `voice-install-progress`.
#[tauri::command(async)]
pub fn voice_install(app: AppHandle) -> Result<VoiceStatus, String> {
    let dir = whisper_dir()?;
    if !cli_path(&dir).exists() {
        install_engine(&app, &dir)?;
    }
    if !model_ready(&dir) {
        install_model(&app, &dir)?;
    }
    voice_status()
}

/// Phrases inventées par Whisper sur du silence ou du bruit.
fn is_hallucination(text: &str) -> bool {
    let t = text.trim().trim_matches(|c: char| c == '.' || c == '!' || c.is_whitespace()).to_lowercase();
    t.is_empty()
        || t.contains("amara.org")
        || t.starts_with("sous-titr")
        || t == "merci d'avoir regardé"
        || t == "merci"
        || t == "merci à tous"
        || (t.starts_with('[') && t.ends_with(']'))
        || (t.starts_with('(') && t.ends_with(')'))
}

/// Transcrit un WAV 16 kHz mono (base64) en français.
#[tauri::command(async)]
pub fn voice_transcribe(wav_b64: String, seconds: f64) -> Result<String, String> {
    let dir = whisper_dir()?;
    if !cli_path(&dir).exists() || !model_ready(&dir) {
        return Err("Dictée non installée".into());
    }
    let wav = base64::engine::general_purpose::STANDARD
        .decode(wav_b64.as_bytes())
        .map_err(|e| e.to_string())?;
    let file = std::env::temp_dir().join(format!("lumen-voice-{}.wav", uuid::Uuid::new_v4()));
    fs::write(&file, wav).map_err(|e| e.to_string())?;
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
    let mut cmd = Command::new(cli_path(&dir));
    cmd.current_dir(&dir)
        .arg("-m")
        .arg(dir.join(MODEL_FILE))
        .arg("-f")
        .arg(&file)
        .args(["-l", "fr", "-nt", "-np", "-t", &threads.to_string(), "--prompt", VOCAB])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Clip court : fenêtre d'encodage réduite à la durée réelle (3 à 4 fois plus rapide sur CPU).
    if seconds > 0.0 && seconds <= 28.0 {
        let ctx = ((seconds * 50.0).ceil() as i64 + 128).clamp(384, 1500);
        cmd.args(["-ac", &ctx.to_string()]);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| format!("whisper-cli : {e}"));
    let _ = fs::remove_file(&file);
    let out = out?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: String = err.lines().rev().take(4).collect::<Vec<_>>().join(" | ");
        return Err(format!("Transcription échouée : {tail}"));
    }
    let text = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !is_hallucination(l))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(text)
}

/// Ajoute une note vocale datée dans notes/notes-vocales.md du projet.
#[tauri::command]
pub fn voice_save_note(project_path: String, stamp: String, text: String) -> Result<String, String> {
    let dir = Path::new(&project_path).join("notes");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("notes-vocales.md");
    let fresh = !path.exists();
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    if fresh {
        writeln!(
            f,
            "# Notes vocales\n\nDictées dans Lumen (bouton « Note vocale »). Idées, bugs vus en test, choses à faire : les plus récentes en bas.\n"
        )
        .map_err(|e| e.to_string())?;
    }
    let clean = stamp.replace(['\n', '\r'], " ");
    writeln!(f, "- **{}** — {}", clean.trim(), text.trim().replace(['\n', '\r'], " ")).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into())
}

/// Le micro sert seulement à la dictée locale : la WebView l'obtient sans fenêtre d'autorisation.
pub fn allow_microphone(app: &tauri::App) {
    #[cfg(windows)]
    {
        use tauri::Manager;
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, COREWEBVIEW2_PERMISSION_STATE_ALLOW,
        };
        use webview2_com::PermissionRequestedEventHandler;
        for (_, window) in app.webview_windows() {
            let _ = window.with_webview(|webview| unsafe {
                let Ok(core) = webview.controller().CoreWebView2() else { return };
                let mut token = 0i64;
                let _ = core.add_PermissionRequested(
                    &PermissionRequestedEventHandler::create(Box::new(|_, args| {
                        let Some(args) = args else { return Ok(()) };
                        let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                        args.PermissionKind(&mut kind)?;
                        if kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE {
                            args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                        }
                        Ok(())
                    })),
                    &mut token,
                );
            });
        }
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// Efface un refus du micro mémorisé par WebView2 (clic sur « Bloquer »), avant l'ouverture de la fenêtre.
pub fn clear_blocked_mic() {
    let Some(base) = dirs::data_local_dir() else { return };
    let prefs = base.join("studio.lumen.app").join("EBWebView").join("Default").join("Preferences");
    let Ok(text) = fs::read_to_string(&prefs) else { return };
    let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&text) else { return };
    let Some(mic) = json
        .pointer_mut("/profile/content_settings/exceptions/media_stream_mic")
        .and_then(|v| v.as_object_mut())
    else {
        return;
    };
    let before = mic.len();
    mic.retain(|_, v| v["setting"].as_i64() != Some(2));
    if mic.len() != before {
        if let Ok(out) = serde_json::to_string(&json) {
            let _ = fs::write(&prefs, out);
        }
    }
}
