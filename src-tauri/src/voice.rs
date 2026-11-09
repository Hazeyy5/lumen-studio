//! Dictée vocale locale : whisper.cpp + modèle large-v3-turbo, téléchargés au premier usage dans
//! %LOCALAPPDATA%\Lumen\whisper. Sur processeur : un whisper-cli par dictée. Option GPU NVIDIA (whisper\cuda) : un
//! whisper-server reste chargé (lancé dès que l'utilisateur commence à parler, arrêté après 15 min sans dictée).
//! Rien ne sort du PC.
use base64::Engine;
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const MODEL_FILE: &str = "ggml-large-v3-turbo-q5_0.bin";
const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin";
const MODEL_MIN_BYTES: u64 = 500_000_000;
const BIN_ASSET: &str = "whisper-bin-x64.zip";
const CUDA_ASSET: &str = "whisper-bin-win-cuda-12.4.0-x64.zip";
const IDLE_STOP: Duration = Duration::from_secs(15 * 60);
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

fn cuda_dir(dir: &Path) -> PathBuf {
    dir.join("cuda")
}

fn gpu_ready(dir: &Path) -> bool {
    let c = cuda_dir(dir);
    c.join("whisper-server.exe").exists() && c.join("ggml-cuda.dll").exists()
}

fn has_nvidia() -> bool {
    Path::new(r"C:\Windows\System32\nvidia-smi.exe").exists() || which::which("nvidia-smi").is_ok()
}

fn model_ready(dir: &Path) -> bool {
    fs::metadata(dir.join(MODEL_FILE))
        .map(|m| m.len() >= MODEL_MIN_BYTES)
        .unwrap_or(false)
}

fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    ready: bool,
    engine: bool,
    model: bool,
    /// Carte NVIDIA détectée.
    gpu_available: bool,
    /// Moteur GPU installé.
    gpu_ready: bool,
}

#[tauri::command]
pub fn voice_status() -> Result<VoiceStatus, String> {
    let dir = whisper_dir()?;
    let engine = cli_path(&dir).exists();
    let model = model_ready(&dir);
    Ok(VoiceStatus {
        ready: engine && model,
        engine,
        model,
        gpu_available: has_nvidia(),
        gpu_ready: gpu_ready(&dir),
    })
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
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

fn release_asset_url(http: &reqwest::blocking::Client, asset: &str) -> Result<String, String> {
    let releases: serde_json::Value = http
        .get("https://api.github.com/repos/ggml-org/whisper.cpp/releases?per_page=15")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("GitHub whisper.cpp : {e}"))?
        .json()
        .map_err(|e| e.to_string())?;
    releases
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|r| r["assets"].as_array().cloned().unwrap_or_default())
        .find(|a| a["name"].as_str() == Some(asset))
        .and_then(|a| a["browser_download_url"].as_str().map(String::from))
        .ok_or_else(|| format!("Aucune release whisper.cpp avec {asset}"))
}

/// Téléchargement en flux vers `dest` (via .part), avec progression.
fn download(app: &AppHandle, url: &str, dest: &Path, stage: &'static str, min_bytes: u64) -> Result<(), String> {
    let http = client()?;
    let mut resp = http
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Téléchargement : {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    let part = dest.with_extension("part");
    let mut out = fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    let mut last = 0u64;
    let _ = app.emit("voice-install-progress", InstallProgress { stage, done: 0, total });
    loop {
        let n = resp.read(&mut buf).map_err(|e| format!("Téléchargement : {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if done - last >= 4 << 20 {
            last = done;
            let _ = app.emit("voice-install-progress", InstallProgress { stage, done, total });
        }
    }
    drop(out);
    if done < min_bytes || (total > 0 && done != total) {
        let _ = fs::remove_file(&part);
        return Err("Téléchargement incomplet, réessaie.".into());
    }
    let _ = fs::remove_file(dest);
    fs::rename(&part, dest).map_err(|e| e.to_string())
}

/// Extrait les exécutables voulus et toutes les DLL d'une archive whisper.cpp.
fn extract(zip_path: &Path, dest: &Path, exes: &[&str]) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut found = 0;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().replace('\\', "/");
        let base = name.rsplit('/').next().unwrap_or("").to_string();
        let is_exe = exes.contains(&base.as_str());
        if !is_exe && !base.to_ascii_lowercase().ends_with(".dll") {
            continue;
        }
        let mut out = fs::File::create(dest.join(&base)).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        found += usize::from(is_exe);
    }
    if found < exes.len() {
        return Err(format!("Archive whisper.cpp incomplète ({})", exes.join(", ")));
    }
    Ok(())
}

fn install_engine(app: &AppHandle, dir: &Path) -> Result<(), String> {
    let _ = app.emit("voice-install-progress", InstallProgress { stage: "engine", done: 0, total: 0 });
    let url = release_asset_url(&client()?, BIN_ASSET)?;
    let zip = dir.join("engine.zip");
    download(app, &url, &zip, "engine", 1_000_000)?;
    stop_server();
    let res = extract(&zip, dir, &["whisper-cli.exe", "whisper-server.exe"]);
    let _ = fs::remove_file(&zip);
    res
}

/// Télécharge ce qui manque (moteur ~20 Mo, modèle ~550 Mo) en émettant `voice-install-progress`.
#[tauri::command(async)]
pub fn voice_install(app: AppHandle) -> Result<VoiceStatus, String> {
    let dir = whisper_dir()?;
    if !cli_path(&dir).exists() || !dir.join("whisper-server.exe").exists() {
        install_engine(&app, &dir)?;
    }
    if !model_ready(&dir) {
        download(&app, MODEL_URL, &dir.join(MODEL_FILE), "model", MODEL_MIN_BYTES)?;
    }
    voice_status()
}

/// Moteur GPU (CUDA, ~700 Mo) pour les cartes NVIDIA : transcription plusieurs fois plus rapide.
#[tauri::command(async)]
pub fn voice_install_gpu(app: AppHandle) -> Result<VoiceStatus, String> {
    let dir = whisper_dir()?;
    if !gpu_ready(&dir) {
        let url = release_asset_url(&client()?, CUDA_ASSET)?;
        let zip = dir.join("cuda.zip");
        download(&app, &url, &zip, "gpu", 100_000_000)?;
        let _ = app.emit("voice-install-progress", InstallProgress { stage: "unpack", done: 0, total: 0 });
        let res = extract(&zip, &cuda_dir(&dir), &["whisper-server.exe"]);
        let _ = fs::remove_file(&zip);
        res?;
    }
    stop_server();
    voice_status()
}

// ---------------------------------------------------------------------------------------------
// Serveur whisper gardé en mémoire.

struct Server {
    child: Child,
    port: u16,
    gpu: bool,
    last: Instant,
}

fn server_slot() -> &'static Mutex<Option<Server>> {
    static SLOT: OnceLock<Mutex<Option<Server>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Arrête le serveur (fermeture de Lumen, changement de moteur, inactivité).
pub fn stop_server() {
    let mut slot = server_slot().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut s) = slot.take() {
        let _ = s.child.kill();
        let _ = s.child.wait();
    }
}

fn spawn_idle_watch() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(30));
            let idle = server_slot()
                .lock()
                .map(|slot| slot.as_ref().is_some_and(|s| s.last.elapsed() > IDLE_STOP))
                .unwrap_or(false);
            if idle {
                stop_server();
            }
        });
    });
}

fn free_port() -> Result<u16, String> {
    let l = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    Ok(l.local_addr().map_err(|e| e.to_string())?.port())
}

fn silence_wav() -> Vec<u8> {
    let n = 16000usize;
    let mut v = Vec::with_capacity(44 + n * 2);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&((36 + n * 2) as u32).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&16000u32.to_le_bytes());
    v.extend_from_slice(&32000u32.to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&((n * 2) as u32).to_le_bytes());
    v.resize(44 + n * 2, 0);
    v
}

fn post_inference(port: u16, wav: Vec<u8>, audio_ctx: Option<i64>, timeout: Duration) -> Result<String, String> {
    let mut form = reqwest::blocking::multipart::Form::new()
        .part(
            "file",
            reqwest::blocking::multipart::Part::bytes(wav)
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| e.to_string())?,
        )
        .text("response_format", "text")
        .text("language", "fr")
        .text("prompt", VOCAB);
    if let Some(ctx) = audio_ctx {
        form = form.text("audio_ctx", ctx.to_string());
    }
    let res = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("http://127.0.0.1:{port}/inference"))
        .multipart(form)
        .send()
        .map_err(|e| format!("whisper-server : {e}"))?;
    if !res.status().is_success() {
        return Err(format!("whisper-server : HTTP {}", res.status()));
    }
    res.text().map_err(|e| e.to_string())
}

/// Lance le serveur s'il ne tourne pas (GPU si installé), et le prépare avec une seconde de silence.
fn ensure_server() -> Result<(u16, bool), String> {
    let dir = whisper_dir()?;
    if !model_ready(&dir) {
        return Err("Dictée non installée".into());
    }
    let mut slot = server_slot().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = slot.as_mut() {
        if matches!(s.child.try_wait(), Ok(None)) {
            s.last = Instant::now();
            return Ok((s.port, s.gpu));
        }
        *slot = None;
    }
    // Sur processeur, whisper-cli avec une fenêtre réduite est plus rapide et plus juste que le serveur :
    // le serveur ne sert qu'au GPU (il évite de recharger ~500 Mo de bibliothèques CUDA à chaque dictée).
    if !gpu_ready(&dir) {
        return Err("Pas de moteur GPU".into());
    }
    let (exe_dir, gpu) = (cuda_dir(&dir), true);
    let port = free_port()?;
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
    let mut cmd = Command::new(exe_dir.join("whisper-server.exe"));
    cmd.current_dir(&exe_dir)
        .arg("-m")
        .arg(dir.join(MODEL_FILE))
        .args(["-l", "fr", "-t", &threads.to_string(), "--host", "127.0.0.1", "--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    no_window(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("whisper-server : {e}"))?;
    let started = Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!("whisper-server s'est arrêté ({status})"));
        }
        if std::net::TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(200)).is_ok() {
            break;
        }
        if started.elapsed() > Duration::from_secs(90) {
            let _ = child.kill();
            return Err("whisper-server ne démarre pas".into());
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    // Première inférence : charge les noyaux GPU (compilés une seule fois par le pilote, jusqu'à ~40 s la toute première fois).
    if let Err(err) = post_inference(port, silence_wav(), None, Duration::from_secs(180)) {
        let _ = child.kill();
        return Err(err);
    }
    *slot = Some(Server { child, port, gpu, last: Instant::now() });
    spawn_idle_watch();
    Ok((port, gpu))
}

/// Appelé quand l'utilisateur commence à parler : le moteur se charge pendant qu'il parle.
#[tauri::command(async)]
pub fn voice_warmup() -> Result<bool, String> {
    if !gpu_ready(&whisper_dir()?) {
        return Ok(false);
    }
    ensure_server().map(|(_, gpu)| gpu)
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

fn clean_text(raw: &str) -> String {
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !is_hallucination(l))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Fenêtre d'encodage réduite à la durée réelle d'un clip court (3 à 4 fois plus rapide sur CPU, avec whisper-cli).
fn short_ctx(seconds: f64) -> Option<i64> {
    (seconds > 0.0 && seconds <= 28.0).then(|| ((seconds * 50.0).ceil() as i64 + 128).clamp(384, 1500))
}

/// Transcrit un WAV 16 kHz mono (base64) en français.
#[tauri::command(async)]
pub fn voice_transcribe(wav_b64: String, seconds: f64) -> Result<String, String> {
    let dir = whisper_dir()?;
    if !model_ready(&dir) {
        return Err("Dictée non installée".into());
    }
    let wav = base64::engine::general_purpose::STANDARD
        .decode(wav_b64.as_bytes())
        .map_err(|e| e.to_string())?;
    if gpu_ready(&dir) {
        if let Ok((port, _)) = ensure_server() {
            match post_inference(port, wav.clone(), None, Duration::from_secs(300)) {
                Ok(text) => return Ok(clean_text(&text)),
                Err(_) => stop_server(),
            }
        }
    }
    if !cli_path(&dir).exists() {
        return Err("Dictée non installée".into());
    }
    transcribe_cli(&dir, wav, seconds)
}

/// Secours : un whisper-cli par dictée (plus lent, recharge le modèle à chaque fois).
fn transcribe_cli(dir: &Path, wav: Vec<u8>, seconds: f64) -> Result<String, String> {
    let file = std::env::temp_dir().join(format!("lumen-voice-{}.wav", uuid::Uuid::new_v4()));
    fs::write(&file, wav).map_err(|e| e.to_string())?;
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
    let mut cmd = Command::new(cli_path(dir));
    cmd.current_dir(dir)
        .arg("-m")
        .arg(dir.join(MODEL_FILE))
        .arg("-f")
        .arg(&file)
        .args(["-l", "fr", "-nt", "-np", "-t", &threads.to_string(), "--prompt", VOCAB])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(ctx) = short_ctx(seconds) {
        cmd.args(["-ac", &ctx.to_string()]);
    }
    no_window(&mut cmd);
    let out = cmd.output().map_err(|e| format!("whisper-cli : {e}"));
    let _ = fs::remove_file(&file);
    let out = out?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: String = err.lines().rev().take(4).collect::<Vec<_>>().join(" | ");
        return Err(format!("Transcription échouée : {tail}"));
    }
    Ok(clean_text(&String::from_utf8_lossy(&out.stdout)))
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
