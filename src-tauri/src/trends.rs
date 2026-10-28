use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

use crate::bins::find_binary;

const FORTNITE_API: &str = "https://api.fortnite.com/ecosystem/v1";
const ROBLOX_SORTS: [&str; 3] = ["top-trending", "up-and-coming", "top-playing-now"];
const CATALOG_REFRESH_SECS: u64 = 6 * 3600;

// Demande un avis à l'agent installé, sans terminal : Claude Code en mode -p, sinon Codex exec.
// Le brief passe par stdin (trop long pour une ligne de commande Windows).
#[tauri::command(async)]
pub fn trends_advice(prompt: String) -> Result<String, String> {
    let (bin, args, label): (_, Vec<&str>, &str) = if let Some(bin) = find_binary(&["claude"]) {
        (bin, vec!["-p", "--tools", "", "--no-session-persistence"], "Claude Code")
    } else if let Some(bin) = find_binary(&["codex"]) {
        (bin, vec!["exec", "--skip-git-repo-check", "-"], "Codex")
    } else {
        return Err("Installe Claude Code ou Codex (Réglages) pour avoir l'avis de l'IA.".into());
    };

    let mut cmd = Command::new(&bin);
    cmd.args(&args)
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd.spawn().map_err(|e| format!("{label} : {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes()).map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || text.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("{label} n'a pas répondu. {err}"));
    }
    Ok(text)
}

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("Lumen/0.1")
        .build()
        .map_err(|e| e.to_string())
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ---------- Roblox : les jeux en tendance (API explore publique, sans CORS d'où le passage par Rust) ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobloxGame {
    pub universe_id: u64,
    pub place_id: u64,
    pub name: String,
    pub players: u64,
    pub genre: String,
    pub up_votes: u64,
    pub down_votes: u64,
    pub sorts: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExploreGame {
    universe_id: u64,
    #[serde(default)]
    root_place_id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    player_count: u64,
    #[serde(default)]
    total_up_votes: u64,
    #[serde(default)]
    total_down_votes: u64,
    #[serde(default)]
    genre_l1: String,
    #[serde(default)]
    is_sponsored: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExploreSort {
    #[serde(default)]
    sort_id: String,
    #[serde(default)]
    games: Vec<ExploreGame>,
}

#[derive(Deserialize)]
struct ExploreResponse {
    #[serde(default)]
    sorts: Vec<ExploreSort>,
}

#[tauri::command(async)]
pub fn roblox_trending(limit: Option<usize>) -> Result<Vec<RobloxGame>, String> {
    let limit = limit.unwrap_or(40);
    let session = uuid::Uuid::new_v4();
    let url = format!(
        "https://apis.roblox.com/explore-api/v1/get-sorts?sessionId={session}&device=computer&country=all"
    );
    let res: ExploreResponse = http()?
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Roblox : {e}"))?
        .json()
        .map_err(|e| format!("Roblox : {e}"))?;
    let mut games: Vec<RobloxGame> = Vec::new();
    for sort in res.sorts.into_iter().filter(|s| ROBLOX_SORTS.contains(&s.sort_id.as_str())) {
        for g in sort.games.into_iter().filter(|g| !g.is_sponsored).take(limit) {
            if let Some(found) = games.iter_mut().find(|x| x.universe_id == g.universe_id) {
                found.sorts.push(sort.sort_id.clone());
                found.players = found.players.max(g.player_count);
                continue;
            }
            games.push(RobloxGame {
                universe_id: g.universe_id,
                place_id: g.root_place_id,
                name: g.name,
                players: g.player_count,
                genre: g.genre_l1,
                up_votes: g.total_up_votes,
                down_votes: g.total_down_votes,
                sorts: vec![sort.sort_id.clone()],
            });
        }
    }
    Ok(games)
}

// ---------- Fortnite : catalogue complet des îles, gardé sur disque ----------
// L'API liste les îles de la plus récente à la plus ancienne : après le premier parcours (~285 k îles,
// ~1 min 30), une mise à jour s'arrête dès qu'elle retombe sur une île connue.

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Island(String, String, String, Vec<String>); // code, titre, créateur, tags

#[derive(Default, Serialize, Deserialize)]
struct Catalog {
    fetched_at: u64,
    islands: Vec<Island>,
}

#[derive(Deserialize)]
struct IslandPage {
    data: Vec<IslandRow>,
    meta: PageMeta,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IslandRow {
    code: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    creator_code: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct PageMeta {
    page: PageCursor,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageCursor {
    next_cursor: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogProgress {
    islands: usize,
    first: bool,
}

struct Indexed {
    catalog: Catalog,
    words: Vec<HashSet<String>>,
    index: HashMap<String, Vec<usize>>,
}

fn state() -> &'static Mutex<Option<Indexed>> {
    static STATE: OnceLock<Mutex<Option<Indexed>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

fn catalog_path() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir().ok_or("AppData Local introuvable")?.join("Lumen");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("fortnite-islands.json"))
}

fn fetch_page(client: &reqwest::blocking::Client, cursor: Option<&str>) -> Result<IslandPage, String> {
    let mut url = format!("{FORTNITE_API}/islands?size=1000");
    if let Some(c) = cursor {
        url.push_str("&after=");
        url.push_str(&urlencode(c));
    }
    for attempt in 0..5u64 {
        let res = client.get(&url).send().map_err(|e| format!("Fortnite : {e}"))?;
        if res.status().as_u16() == 429 || res.status().is_server_error() {
            std::thread::sleep(Duration::from_millis(1500 * (attempt + 1)));
            continue;
        }
        return res
            .error_for_status()
            .map_err(|e| format!("Fortnite : {e}"))?
            .json()
            .map_err(|e| format!("Fortnite : {e}"));
    }
    Err("Fortnite : l'API limite le débit, réessaie dans une minute.".into())
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn refresh_catalog(app: &AppHandle, mut catalog: Catalog) -> Result<Catalog, String> {
    let client = http()?;
    let known: HashSet<String> = catalog.islands.iter().map(|i| i.0.clone()).collect();
    let first = known.is_empty();
    let mut fresh: Vec<Island> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = fetch_page(&client, cursor.as_deref())?;
        let empty = page.data.is_empty();
        let mut hit_known = false;
        for row in page.data {
            if known.contains(&row.code) {
                hit_known = true;
                continue;
            }
            fresh.push(Island(row.code, row.title, row.creator_code, row.tags));
        }
        let _ = app.emit("catalog-progress", CatalogProgress { islands: known.len() + fresh.len(), first });
        cursor = page.meta.page.next_cursor;
        if empty || hit_known || cursor.is_none() {
            break;
        }
    }
    fresh.extend(catalog.islands);
    catalog.islands = fresh;
    catalog.fetched_at = now_secs();
    if let Ok(path) = catalog_path() {
        if let Ok(json) = serde_json::to_vec(&catalog) {
            let _ = fs::write(path, json);
        }
    }
    Ok(catalog)
}

const STOP: &[&str] = &[
    "a", "an", "the", "of", "and", "or", "to", "in", "on", "for", "with", "my", "your", "our", "de", "la", "le",
    "les", "du", "des", "un", "une", "et", "en", "x2", "x3", "2x", "update", "new", "event", "beta", "alpha", "soon",
    "test", "testing", "rp", "v1", "v2", "v3", "part", "free", "upd", "now",
];

fn fold(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        _ => c,
    }
}

// Mots significatifs d'un titre : sans [balises], emojis, numéros de version ni mots vides.
fn words(title: &str) -> Vec<String> {
    let mut clean = String::with_capacity(title.len());
    let mut depth = 0;
    for c in title.to_lowercase().chars().map(fold) {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = (depth - 1).max(0),
            _ if depth > 0 => {}
            c if c.is_ascii_alphanumeric() => clean.push(c),
            _ => clean.push(' '),
        }
    }
    let mut out: Vec<String> = Vec::new();
    for w in clean.split_whitespace() {
        if w.len() < 2 || STOP.contains(&w) || w.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if !out.iter().any(|x| x == w) {
            out.push(w.to_string());
        }
    }
    out
}

fn build_index(catalog: Catalog) -> Indexed {
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();
    let words: Vec<HashSet<String>> = catalog
        .islands
        .iter()
        .enumerate()
        .map(|(i, island)| {
            let set: HashSet<String> = words(&island.1).into_iter().collect();
            for w in &set {
                index.entry(w.clone()).or_default().push(i);
            }
            set
        })
        .collect();
    Indexed { catalog, words, index }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FortniteCandidate {
    pub code: String,
    pub title: String,
    pub creator: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FortniteMatch {
    pub name: String,
    pub copies: usize,
    pub candidates: Vec<FortniteCandidate>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogMatches {
    pub islands: usize,
    pub fetched_at: u64,
    pub matches: Vec<FortniteMatch>,
}

// Pour chaque nom de jeu Roblox, les îles Fortnite dont le titre reprend ses mots
// (tous jusqu'à 3 mots, 75 % au-delà). D'abord celles de `prefer` (les îles classées, meilleur rang en tête),
// à un mot près, puis les plus proches : un titre copié 200 fois ne doit pas cacher la copie qui marche.
#[tauri::command(async)]
pub fn fortnite_catalog_match(
    app: AppHandle,
    names: Vec<String>,
    prefer: Option<Vec<String>>,
    force: Option<bool>,
) -> Result<CatalogMatches, String> {
    let preferred: HashMap<String, usize> =
        prefer.unwrap_or_default().into_iter().enumerate().map(|(i, code)| (code, i)).collect();
    let mut guard = state().lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        let cached = catalog_path()
            .ok()
            .and_then(|p| fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice::<Catalog>(&bytes).ok())
            .unwrap_or_default();
        *guard = Some(build_index(cached));
    }
    let stale = guard
        .as_ref()
        .map(|s| s.catalog.islands.is_empty() || now_secs().saturating_sub(s.catalog.fetched_at) > CATALOG_REFRESH_SECS)
        .unwrap_or(true);
    if stale || force.unwrap_or(false) {
        let old = guard.take().map(|s| s.catalog).unwrap_or_default();
        let refreshed = refresh_catalog(&app, old)?;
        *guard = Some(build_index(refreshed));
    }
    let s = guard.as_ref().ok_or("Catalogue Fortnite indisponible")?;

    let matches = names
        .into_iter()
        .map(|name| {
            let query = words(&name);
            if query.is_empty() {
                return FortniteMatch { name, copies: 0, candidates: Vec::new() };
            }
            let need = if query.len() <= 3 { query.len() } else { (query.len() * 3).div_ceil(4) };
            let mut counts: HashMap<usize, usize> = HashMap::new();
            for w in &query {
                for &i in s.index.get(w).map(|v| v.as_slice()).unwrap_or(&[]) {
                    *counts.entry(i).or_default() += 1;
                }
            }
            let mut hits: Vec<(usize, usize)> = counts.into_iter().filter(|(_, c)| *c >= need).collect();
            hits.sort_by_key(|(i, c)| {
                let gap = s.words[*i].len().abs_diff(query.len());
                let rank = if gap <= 1 { preferred.get(&s.catalog.islands[*i].0).copied() } else { None };
                (rank.unwrap_or(usize::MAX), std::cmp::Reverse(*c), gap, *i)
            });
            FortniteMatch {
                name,
                copies: hits.len(),
                candidates: hits
                    .iter()
                    .take(5)
                    .map(|(i, _)| {
                        let island = &s.catalog.islands[*i];
                        FortniteCandidate {
                            code: island.0.clone(),
                            title: island.1.clone(),
                            creator: island.2.clone(),
                            tags: island.3.clone(),
                        }
                    })
                    .collect(),
            }
        })
        .collect();
    Ok(CatalogMatches { islands: s.catalog.islands.len(), fetched_at: s.catalog.fetched_at, matches })
}
