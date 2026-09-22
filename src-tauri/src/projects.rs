use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub name: String,
    pub path: String,
    pub created_at: String,
    #[serde(default)]
    pub bound_place_name: Option<String>,
    #[serde(default)]
    pub bound_place_id: Option<i64>,
    /// Autres projets Lumen, lecture seule.
    #[serde(default)]
    pub reference_paths: Vec<String>,
    /// Ancien champ (un seul projet). Migré vers `referencePaths`.
    #[serde(default)]
    pub reference_path: Option<String>,
}

pub(crate) fn projects_root() -> Result<PathBuf, String> {
    let dir = dirs::document_dir()
        .ok_or("Documents introuvable")?
        .join("Lumen")
        .join("projects");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn read_json_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    let raw = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    Ok(raw.trim().to_string())
}

fn parse_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = read_json_text(path)?;
    if raw.is_empty() {
        return Err(format!("Fichier JSON vide : {}", path.display()));
    }
    serde_json::from_str(&raw).map_err(|e| format!("{} : {e}", path.display()))
}

fn meta_path(dir: &Path) -> PathBuf {
    dir.join(".lumen.json")
}

#[tauri::command]
pub fn list_projects() -> Result<Vec<Project>, String> {
    let root = projects_root()?;
    let mut out = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(mut project) = parse_json_file::<Project>(&meta_path(&path)) {
            project.path = path.to_string_lossy().into();
            out.push(project);
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

#[tauri::command]
pub fn create_project(name: String) -> Result<Project, String> {
    let slug = slugify(&name);
    if slug.is_empty() {
        return Err("Nom de projet invalide".into());
    }
    let dir = projects_root()?.join(&slug);
    if dir.exists() {
        return Err("Un projet porte déjà ce nom".into());
    }
    fs::create_dir_all(dir.join("src").join("server")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("src").join("client")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("src").join("shared")).map_err(|e| e.to_string())?;

    let project = Project {
        name: name.trim().to_string(),
        path: dir.to_string_lossy().into(),
        created_at: chrono_like_now(),
        bound_place_name: None,
        bound_place_id: None,
        reference_paths: Vec::new(),
        reference_path: None,
    };

    fs::write(
        meta_path(&dir),
        serde_json::to_string_pretty(&project).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    write_rbxts_layout(&dir)?;

    fs::write(
        dir.join("src/shared/config.ts"),
        r#"export const GAME_NAME = "Nouveau jeu Lumen";
export const VERSION = "0.1.0";
export const COIN_PAD_REWARD = 1;
"#,
    )
    .map_err(|e| e.to_string())?;

    fs::write(
        dir.join("src/server/main.server.ts"),
        r#"import { COIN_PAD_REWARD, GAME_NAME } from "shared/config";

const Players = game.GetService("Players");

print(`[Lumen] ${GAME_NAME} — serveur prêt`);

function leaderstats(player: Player) {
	const folder = new Instance("Folder");
	folder.Name = "leaderstats";
	const coins = new Instance("IntValue");
	coins.Name = "Coins";
	coins.Value = 0;
	coins.Parent = folder;
	folder.Parent = player;
}

function coinPad() {
	const existing = game.Workspace.FindFirstChild("CoinPad");
	if (existing) existing.Destroy();

	const pad = new Instance("Part");
	pad.Name = "CoinPad";
	pad.Anchored = true;
	pad.Size = new Vector3(12, 1, 12);
	pad.Position = new Vector3(18, 0.5, 0);
	pad.Color = Color3.fromRGB(232, 168, 88);
	pad.Material = Enum.Material.SmoothPlastic;
	pad.Parent = game.Workspace;

	const lastGrant = new Map<number, number>();
	pad.Touched.Connect((hit) => {
		const character = hit.Parent;
		if (!character) return;
		const player = Players.GetPlayerFromCharacter(character);
		if (!player) return;
		const now = os.clock();
		if ((lastGrant.get(player.UserId) ?? 0) + 0.8 > now) return;
		lastGrant.set(player.UserId, now);
		const coins = player.FindFirstChild("leaderstats")?.FindFirstChild("Coins") as IntValue | undefined;
		if (coins) coins.Value += COIN_PAD_REWARD;
	});
}

Players.PlayerAdded.Connect(leaderstats);
for (const player of Players.GetPlayers()) leaderstats(player);
coinPad();
"#,
    )
    .map_err(|e| e.to_string())?;

    fs::write(
        dir.join("src/client/main.client.ts"),
        r#"import { GAME_NAME } from "shared/config";

const player = game.GetService("Players").LocalPlayer;
if (!player) {
	throw "LocalPlayer introuvable";
}

const gui = new Instance("ScreenGui");
gui.Name = "LumenHud";
gui.ResetOnSpawn = false;
gui.IgnoreGuiInset = true;
gui.Parent = player.WaitForChild("PlayerGui");

const title = new Instance("TextLabel");
title.BackgroundTransparency = 1;
title.Position = new UDim2(0, 24, 0, 18);
title.Size = new UDim2(1, -48, 0, 36);
title.Font = Enum.Font.GothamMedium;
title.Text = GAME_NAME;
title.TextColor3 = Color3.fromRGB(184, 92, 56);
title.TextSize = 22;
title.TextXAlignment = Enum.TextXAlignment.Left;
title.Parent = gui;

const hint = new Instance("TextLabel");
hint.BackgroundTransparency = 1;
hint.Position = new UDim2(0, 24, 0, 50);
hint.Size = new UDim2(1, -48, 0, 24);
hint.Font = Enum.Font.Gotham;
hint.Text = "Marche sur le plot doré pour gagner des pièces.";
hint.TextColor3 = Color3.fromRGB(107, 97, 86);
hint.TextSize = 16;
hint.TextXAlignment = Enum.TextXAlignment.Left;
hint.Parent = gui;

print(`[Lumen] client prêt — ${GAME_NAME}`);
"#,
    )
    .map_err(|e| e.to_string())?;

    fs::write(
        dir.join("AGENTS.md"),
        format!(
            r#"# {name}

Jeu Roblox créé avec Lumen.

## Stack
- Écris uniquement dans `src/` (TypeScript).
- `rbxtsc` compile vers `out/` (Luau). Ne pas éditer `out/` ni `include/`.
- Rojo synchronise `out/` + `include/` vers Studio.

## Consigne pour l'agent
Tu construis un jeu Roblox jouable. Écris le code dans `src/server`, `src/client` et `src/shared`.
Les fichiers serveur se nomment `*.server.ts`, les clients `*.client.ts`, le reste devient des ModuleScripts.
Ne demande pas de confirmer les étapes évidentes. Livre une première version jouable, puis itère.

{hud_section}
{asset_section}"#,
            name = project.name,
            hud_section = HUD_SECTION.trim_start(),
            asset_section = ASSET_SECTION.trim_start(),
        ),
    )
    .map_err(|e| e.to_string())?;

    write_agent_bridge(&dir)?;
    Ok(project)
}

pub fn write_rbxts_layout(dir: &Path) -> Result<(), String> {
    let name = parse_json_file::<Project>(&meta_path(dir))
        .map(|p| p.name)
        .unwrap_or_else(|_| "lumen-game".into());
    let safe = name.replace('"', "");

    let pkg_path = dir.join("package.json");
    let need_pkg = fs::read_to_string(&pkg_path)
        .map(|s| !s.contains("roblox-ts"))
        .unwrap_or(true);
    if need_pkg {
        fs::write(
            pkg_path,
            format!(
                r#"{{
  "name": "{slug}",
  "private": true,
  "version": "0.1.0",
  "scripts": {{
    "build": "rbxtsc",
    "watch": "rbxtsc -w"
  }},
  "devDependencies": {{
    "@rbxts/compiler-types": "latest",
    "@rbxts/types": "latest",
    "roblox-ts": "latest",
    "typescript": "^5.7.0"
  }}
}}
"#,
                slug = slugify(&safe)
            ),
        )
        .map_err(|e| e.to_string())?;
    }

    fs::write(
        dir.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "allowSyntheticDefaultImports": true,
    "downlevelIteration": true,
    "jsx": "react",
    "jsxFactory": "Roact.createElement",
    "jsxFragmentFactory": "Roact.Fragment",
    "module": "commonjs",
    "moduleDetection": "force",
    "moduleResolution": "Node",
    "noLib": true,
    "resolveJsonModule": true,
    "forceConsistentCasingInFileNames": true,
    "strict": true,
    "target": "ESNext",
    "typeRoots": ["node_modules/@rbxts"],
    "rootDir": "src",
    "baseUrl": "src",
    "outDir": "out",
    "incremental": true,
    "tsBuildInfoFile": "out/tsconfig.tsbuildinfo",
    "experimentalDecorators": true,
    "isolatedModules": true
  },
  "include": ["src/**/*"]
}
"#,
    )
    .map_err(|e| e.to_string())?;

    fs::write(
        dir.join("default.project.json"),
        format!(
            r#"{{
  "name": "{safe}",
  "globIgnorePaths": ["**/package.json", "**/tsconfig.json"],
  "tree": {{
    "$className": "DataModel",
    "ReplicatedStorage": {{
      "$className": "ReplicatedStorage",
      "rbxts_include": {{
        "$path": "include",
        "node_modules": {{
          "$className": "Folder",
          "@rbxts": {{
            "$path": "node_modules/@rbxts"
          }}
        }}
      }},
      "TS": {{
        "$path": "out/shared"
      }}
    }},
    "ServerScriptService": {{
      "$className": "ServerScriptService",
      "TS": {{
        "$path": "out/server"
      }}
    }},
    "StarterPlayer": {{
      "$className": "StarterPlayer",
      "StarterPlayerScripts": {{
        "$className": "StarterPlayerScripts",
        "TS": {{
          "$path": "out/client"
        }}
      }}
    }},
    "Workspace": {{
      "$className": "Workspace",
      "Baseplate": {{
        "$className": "Part",
        "$properties": {{
          "Anchored": true,
          "Locked": true,
          "Size": [512, 20, 512],
          "Position": [0, -10, 0],
          "Color": [0.55, 0.5, 0.42],
          "Material": "Grass"
        }}
      }},
      "Spawn": {{
        "$className": "SpawnLocation",
        "$properties": {{
          "Anchored": true,
          "Duration": 0,
          "Neutral": true,
          "Size": [12, 1, 12],
          "Position": [0, 0.5, 0],
          "Color": [0.72, 0.45, 0.28],
          "Material": "SmoothPlastic"
        }}
      }}
    }},
    "Lighting": {{
      "$className": "Lighting",
      "$properties": {{
        "Ambient": [0.35, 0.32, 0.28],
        "Brightness": 2,
        "ClockTime": 14.5,
        "OutdoorAmbient": [0.45, 0.42, 0.38]
      }}
    }},
    "SoundService": {{
      "$className": "SoundService",
      "$properties": {{
        "RespectFilteringEnabled": true
      }}
    }},
    "HttpService": {{
      "$className": "HttpService",
      "$properties": {{
        "HttpEnabled": true
      }}
    }}
  }}
}}
"#
        ),
    )
    .map_err(|e| e.to_string())?;

    fs::write(
        dir.join(".gitignore"),
        "node_modules\nout\ninclude\n.lumen-swarm.json\n",
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn open_project_dir(path: String) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err("Dossier projet introuvable".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir l'explorateur: {e}"))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn read_project_meta(path: &str) -> Result<Project, String> {
    let dir = PathBuf::from(path);
    let mut project: Project = parse_json_file(&meta_path(&dir))?;
    project.path = dir.to_string_lossy().into();
    merge_legacy_refs(&mut project);
    Ok(project)
}

fn merge_legacy_refs(project: &mut Project) {
    if let Some(legacy) = project.reference_path.take() {
        let trimmed = legacy.trim().to_string();
        if !trimmed.is_empty()
            && !project
                .reference_paths
                .iter()
                .any(|p| paths_equal(Path::new(p), Path::new(&trimmed)))
        {
            project.reference_paths.insert(0, trimmed);
        }
    }
}

fn save_project(project: &Project) -> Result<(), String> {
    fs::write(
        meta_path(Path::new(&project.path)),
        serde_json::to_string_pretty(project).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        p.canonicalize()
            .unwrap_or_else(|_| p.to_path_buf())
            .to_string_lossy()
            .replace('/', "\\")
            .trim_start_matches(r"\\?\")
            .to_ascii_lowercase()
    };
    norm(a) == norm(b)
}

fn under_projects_root(path: &Path) -> Result<bool, String> {
    let root = projects_root()?;
    let root = root.canonicalize().unwrap_or(root);
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let a = canon.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    let b = root.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    Ok(a.starts_with(&b))
}

pub fn references_of(project_path: &str) -> Result<Vec<Project>, String> {
    let project = read_project_meta(project_path)?;
    let mut out = Vec::new();
    for raw in &project.reference_paths {
        let Ok(other) = read_project_meta(raw) else {
            continue;
        };
        if paths_equal(Path::new(&project.path), Path::new(&other.path)) {
            continue;
        }
        if out
            .iter()
            .any(|p: &Project| paths_equal(Path::new(&p.path), Path::new(&other.path)))
        {
            continue;
        }
        out.push(other);
    }
    Ok(out)
}

pub fn pick_reference(from_project: &str, needle: &str) -> Result<Project, String> {
    let refs = references_of(from_project)?;
    if refs.is_empty() {
        return Err("Aucun projet référence. Studio → S'inspirer de.".into());
    }
    let want = needle.trim();
    if want.is_empty() {
        if refs.len() == 1 {
            return Ok(refs.into_iter().next().unwrap());
        }
        let names: Vec<String> = refs.iter().map(|p| p.name.clone()).collect();
        return Err(format!(
            "Plusieurs projets liés. Précise le nom : {}",
            names.join(", ")
        ));
    }
    let slug = slugify(want);
    refs.into_iter()
        .find(|p| {
            p.name.eq_ignore_ascii_case(want)
                || slugify(&p.name) == slug
                || paths_equal(Path::new(&p.path), Path::new(want))
        })
        .ok_or_else(|| format!("Projet référence inconnu : {want}"))
}

#[tauri::command]
pub fn set_reference_projects(
    project_path: String,
    reference_paths: Vec<String>,
) -> Result<Project, String> {
    let mut project = read_project_meta(&project_path)?;
    let mut cleaned = Vec::new();
    for raw in reference_paths {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if !under_projects_root(Path::new(&trimmed))? {
            return Err("Le projet référence doit être un projet Lumen".into());
        }
        let other = read_project_meta(&trimmed)?;
        if paths_equal(Path::new(&project.path), Path::new(&other.path)) {
            return Err("Choisis un autre projet, pas celui-ci".into());
        }
        if cleaned
            .iter()
            .any(|p: &String| paths_equal(Path::new(p), Path::new(&other.path)))
        {
            continue;
        }
        cleaned.push(other.path);
    }
    project.reference_paths = cleaned;
    project.reference_path = project.reference_paths.first().cloned();
    save_project(&project)?;
    let _ = write_agent_bridge(Path::new(&project.path));
    Ok(project)
}

pub fn bind_project_place(path: &str, place_name: &str, place_id: i64) -> Result<Project, String> {
    let mut project = read_project_meta(path)?;
    project.bound_place_name = Some(place_name.to_string());
    project.bound_place_id = Some(place_id);
    save_project(&project)?;
    Ok(project)
}

fn chrono_like_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

fn slugify(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

const HUD_SECTION: &str = r#"
## HUD / UI (style cartoon Roblox)
Tous les agents suivent le même langage visuel : contours noirs épais, coins ronds, couleurs saturées, FredokaOne, boutons 64 px.
Rail gauche (Shop / Lucky / Collect / Rebirth), barre bas-gauche (Lvl, or, jauge), pastille multiplicateur, boutique carte blanche + bandeau rouge SHOP.
Code HUD dans `src/client/` (`ui.ts`, `hud.ts`, `shop.ts`). Enrichir, ne jamais aplatir en gris.
Si l'utilisateur demande de s'inspirer d'une capture : `node tools/lumen-asset.mjs search inspiration hud` puis `get INS-xxxx`, Read l'image dans `assets/inspiration/`, et recrée une UI originale dans le même esprit.
"#;

const ASSET_SECTION: &str = r#"
## Assets (banque Lumen + VibeStarter)
Les clés API sont dans Lumen. Pour la map : **cherche d'abord**, propose, puis place.

```
node tools/lumen-asset.mjs search palm
node tools/lumen-asset.mjs search mesh totem --for "totem décoratif au spawn"
node tools/lumen-asset.mjs search image coin --for "icône pièce HUD"
node tools/lumen-asset.mjs search inspiration hud --for "layout boutique"
node tools/lumen-asset.mjs search texture bouton --for "fond bouton shop"
node tools/lumen-asset.mjs get VS-0124
node tools/lumen-asset.mjs get TEX-0001
node tools/lumen-asset.mjs get INS-0001
node tools/lumen-asset.mjs publish VS-0124
node tools/lumen-asset.mjs image "icône pièce d'or, style Roblox, PNG fond transparent"
node tools/lumen-asset.mjs mesh "coffre low poly pour tycoon Roblox"
node tools/lumen-asset.mjs blender assets/blender/crate.py Crate
```

1. `search` dans la banque (codes `LUM-xxxx`, `VS-xxxx`, `TEX-xxxx`) avant de générer. Toujours `--for "à quoi ça sert sur la map"` : Lumen l’affiche sur le toast. Lumen montre un **menu d’environ 10** assets : l’utilisateur en choisit **un**.
2. `search` / `propose` attendent le choix. Ensuite **un seul** `get` sur le code choisi. Ne get jamais les autres. La recherche ne contient pas d’ID Roblox.
3. Pour l’utiliser : `get CODE` — Lumen notifie, l’utilisateur prévisualise et valide. Ensuite Image = `rbxassetid://…`. Mesh = `InsertService.LoadAsset` côté serveur (Model, pas MeshId).
4. `image` / `mesh` seulement si la recherche est vide ou si l’utilisateur a cliqué Aucune. Si `status` dit `meshProvider: blender`, écris un script bpy dans `assets/blender/` puis `node tools/lumen-asset.mjs blender assets/blender/nom.py Titre` — pas Meshy. Les `image` d’icônes / props 2D : **PNG fond transparent**, jamais un fond uni.
5. Inspiration UI (`INS-xxxx`) : `search inspiration` (3 propositions, 1 choix), puis `get` — copie dans `assets/inspiration/`. Read l'image, reproduis l'esprit (layout/couleurs), jamais publish vers Roblox.
6. Textures (`TEX-xxxx`) : `search texture brick --for "sol de la rampe"`. `get` puis colle `rbxassetid://…` (ou `rbxasset://…`) sur ImageLabel / Texture / Decal / MeshPart.TextureID. Si le JSON a `scaleType` (textures Studio importées) : `ImageLabel.ScaleType` + `TileSize = UDim2.new(...)`. Les IDs Studio importés dans la banque sont déjà publiés.
"#;

fn ref_section_for(dir: &Path) -> String {
    let refs = references_of(&dir.to_string_lossy()).unwrap_or_default();
    if refs.is_empty() {
        return r#"
## Projets référence (lecture seule)
Aucun projet lié. Dans Studio, **S'inspirer de** pour en ajouter un ou plusieurs (lecture seule).
À n'utiliser que si l'utilisateur le demande, et seulement le projet qu'il nomme.
"#
        .into();
    }
    let lines: String = refs
        .iter()
        .map(|p| format!("- **{}** (`{}`)", p.name, p.path))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"
## Projets référence (lecture seule)
Jeux Lumen **disponibles** (lecture seule). Ne t'en sers **que si l'utilisateur le demande**, et seulement le(s) projet(s) qu'il nomme. Ignore les autres. Ne les modifie jamais.

{lines}

```
node tools/lumen-ref.mjs status
node tools/lumen-ref.mjs files "Nom du projet" src/client
node tools/lumen-ref.mjs cat "Nom du projet" src/client/ui.ts
```
"#
    )
}

pub fn write_agent_bridge(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir.join("tools")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("assets").join("images")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("assets").join("meshes")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("assets").join("inspiration")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("assets").join("inbox")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("assets").join("blender")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join(".cursor").join("skills").join("lumen-assets")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join(".claude").join("skills").join("lumen-assets")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join(".codex").join("skills").join("lumen-assets")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join(".agents").join("skills").join("lumen-assets")).map_err(|e| e.to_string())?;

    fs::write(
        dir.join("tools").join("lumen-asset.mjs"),
        include_str!("../resources/lumen-asset.mjs"),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        dir.join("tools").join("lumen-blender-run.py"),
        include_str!("../resources/lumen-blender-run.py"),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        dir.join("tools").join("lumen-ref.mjs"),
        include_str!("../resources/lumen-ref.mjs"),
    )
    .map_err(|e| e.to_string())?;

    let skill = include_str!("../resources/lumen-assets.SKILL.md");
    fs::write(
        dir.join(".cursor").join("skills").join("lumen-assets").join("SKILL.md"),
        skill,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        dir.join(".claude").join("skills").join("lumen-assets").join("SKILL.md"),
        skill,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        dir.join(".codex").join("skills").join("lumen-assets").join("SKILL.md"),
        skill,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        dir.join(".agents").join("skills").join("lumen-assets").join("SKILL.md"),
        skill,
    )
    .map_err(|e| e.to_string())?;

    let agents_path = dir.join("AGENTS.md");
    if agents_path.exists() {
        let current = fs::read_to_string(&agents_path).unwrap_or_default();
        let next = upsert_ref_section(
            &upsert_hud_section(&upsert_asset_section(&current)),
            &ref_section_for(dir),
        );
        if next != current {
            fs::write(&agents_path, next).map_err(|e| e.to_string())?;
        }
    } else {
        fs::write(
            &agents_path,
            format!(
                "# Projet Lumen\n{}{}{}",
                HUD_SECTION,
                ref_section_for(dir),
                ASSET_SECTION
            ),
        )
        .map_err(|e| e.to_string())?;
    }

    write_claude_ref_settings(dir)?;

    let claude_path = dir.join("CLAUDE.md");
    if !claude_path.exists() {
        fs::write(
            claude_path,
            "# Lumen\n\nLis `AGENTS.md`. Banque : `node tools/lumen-asset.mjs search …`. Projet référence : `node tools/lumen-ref.mjs files src/client`.\n",
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn write_claude_ref_settings(dir: &Path) -> Result<(), String> {
    let extra: Vec<String> = references_of(&dir.to_string_lossy())?
        .into_iter()
        .map(|other| other.path)
        .collect();
    fs::create_dir_all(dir.join(".claude")).map_err(|e| e.to_string())?;
    let json = serde_json::json!({
        "permissions": {
            "additionalDirectories": extra
        }
    });
    fs::write(
        dir.join(".claude").join("settings.local.json"),
        serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn upsert_ref_section(current: &str, section: &str) -> String {
    let start = ["## Projets référence", "## Projet référence"]
        .iter()
        .filter_map(|head| current.find(head))
        .min();
    let section = section.trim_start();
    if let Some(start) = start {
        let after = &current[start..];
        let end = after
            .find('\n')
            .and_then(|nl| after[nl + 1..].find("\n## ").map(|i| start + nl + 1 + i))
            .unwrap_or(current.len());
        let before = current[..start].trim_end();
        let rest = current[end..].trim_start();
        if rest.is_empty() {
            format!("{before}\n\n{section}")
        } else {
            format!("{before}\n\n{section}\n{rest}")
        }
    } else if let Some(start) = current.find("## Assets") {
        let before = current[..start].trim_end();
        let after = &current[start..];
        format!("{before}\n\n{section}\n{after}")
    } else {
        let mut next = current.to_string();
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(section);
        next
    }
}

fn upsert_hud_section(current: &str) -> String {
    if current.contains("## HUD / UI") {
        return current.to_string();
    }
    if let Some(start) = current.find("## Assets") {
        let before = current[..start].trim_end();
        let after = &current[start..];
        return format!("{before}\n{HUD_SECTION}\n{after}");
    }
    let mut next = current.to_string();
    if !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(HUD_SECTION);
    next
}

fn upsert_asset_section(current: &str) -> String {
    let start = ["## Assets (banque", "## Assets (Gemini"]
        .iter()
        .filter_map(|head| current.find(head))
        .min();
    let section = ASSET_SECTION.trim_start();
    if let Some(start) = start {
        let before = current[..start].trim_end();
        format!("{before}\n\n{section}")
    } else if current.contains("lumen-asset") {
        current.to_string()
    } else {
        let mut next = current.to_string();
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(ASSET_SECTION);
        next
    }
}

pub fn ensure_agent_bridge(project_path: &str) -> Result<(), String> {
    write_agent_bridge(Path::new(project_path))
}

const REF_SKIP: &[&str] = &[
    "node_modules",
    "out",
    "include",
    ".git",
    "target",
    "dist",
    ".cursor",
    ".claude",
    ".codex",
    ".agents",
    ".gemini",
];

fn safe_ref_rel(rel: &str) -> Result<PathBuf, String> {
    let trimmed = rel.trim().trim_start_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Err("chemin manquant".into());
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("chemin invalide".into());
    }
    Ok(path)
}

pub fn list_reference_files(
    from_project: &str,
    needle: &str,
    prefix: &str,
) -> Result<(Project, Vec<String>), String> {
    let other = pick_reference(from_project, needle)?;
    let root = PathBuf::from(&other.path);
    let start = if prefix.trim().is_empty() {
        root.clone()
    } else {
        root.join(safe_ref_rel(prefix)?)
    };
    if !start.exists() {
        return Err(format!("Dossier introuvable dans {0}: {prefix}", other.name));
    }
    let mut files = Vec::new();
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        if out.len() >= 250 {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            if out.len() >= 250 {
                return;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || REF_SKIP.iter().any(|s| *s == name) {
                continue;
            }
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    walk(&start, &root, &mut files);
    files.sort();
    Ok((other, files))
}

pub fn read_reference_file(
    from_project: &str,
    needle: &str,
    rel: &str,
) -> Result<(Project, String), String> {
    let other = pick_reference(from_project, needle)?;
    let root = PathBuf::from(&other.path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let full = root.join(safe_ref_rel(rel)?);
    let canon = full.canonicalize().map_err(|_| format!("Fichier introuvable : {rel}"))?;
    let root_s = root.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    let file_s = canon.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    if !file_s.starts_with(&root_s) {
        return Err("Fichier hors du projet référence".into());
    }
    let meta = fs::metadata(&canon).map_err(|e| e.to_string())?;
    if meta.len() > 200_000 {
        return Err("Fichier trop volumineux (max 200 Ko)".into());
    }
    let text = fs::read_to_string(&canon)
        .map_err(|_| "Ce fichier n’est pas du texte".to_string())?;
    Ok((other, text))
}

