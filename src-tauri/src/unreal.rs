use crate::projects::{
    chrono_like_now, meta_path, parse_json_file, read_project_meta, ref_section_for, save_project,
    unreal_projects_root, upsert_ref_section, write_claude_settings, Project,
};
use crate::uefn::{read_json_object, slash_path, upsert_section, write_json, UefnProject};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Serveur MCP officiel d'Unreal 5.8 (plugin ModelContextProtocol) : port et chemin par défaut.
const MCP_URL: &str = "http://127.0.0.1:8000/mcp";
const MCP_NAME: &str = "unreal-mcp";
/// Plugins activés à la liaison : le serveur MCP, tous ses toolsets, Live Coding (absent d'AllToolsets),
/// et Python, sans lequel les outils d'édition (acteurs, assets, import, Blueprints) ne se chargent pas.
const MCP_PLUGINS: [&str; 4] = ["ModelContextProtocol", "AllToolsets", "LiveCodingToolset", "PythonScriptPlugin"];
const NETFX_COMPONENT: &str = "Microsoft.Net.Component.4.8.SDK";

fn uproject_file(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("uproject"))
    })
}

fn unreal_title(dir: &Path) -> String {
    uproject_file(dir)
        .and_then(|f| f.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Projet Unreal".into())
}

fn inside_unreal_root(dir: &Path) -> Result<PathBuf, String> {
    let root = unreal_projects_root().ok_or("Dossier Documents/Unreal Projects introuvable")?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let canon = dir.canonicalize().map_err(|_| "Projet Unreal introuvable".to_string())?;
    if canon.parent() != Some(root.as_path()) {
        return Err("Le projet doit être dans Documents/Unreal Projects".into());
    }
    Ok(canon)
}

pub fn is_unreal_project(project_path: &str) -> bool {
    !project_path.trim().is_empty()
        && read_project_meta(project_path).is_ok_and(|p| p.is_unreal())
}

#[tauri::command(async)]
pub fn list_unreal_projects() -> Result<Vec<UefnProject>, String> {
    let Some(root) = unreal_projects_root() else {
        return Ok(Vec::new());
    };
    let mut out: Vec<UefnProject> = fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && uproject_file(p).is_some())
        .map(|p| UefnProject {
            name: unreal_title(&p),
            linked: meta_path(&p).is_file(),
            path: p.to_string_lossy().into(),
        })
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    Ok(out)
}

#[tauri::command(async)]
pub fn link_unreal_project(path: String) -> Result<Project, String> {
    let dir = PathBuf::from(path.trim());
    inside_unreal_root(&dir)?;
    if uproject_file(&dir).is_none() {
        return Err("Ce dossier n’est pas un projet Unreal (.uproject absent)".into());
    }
    let mut project = if meta_path(&dir).is_file() {
        read_project_meta(&dir.to_string_lossy())?
    } else {
        Project {
            name: unreal_title(&dir),
            path: dir.to_string_lossy().into(),
            created_at: chrono_like_now(),
            bound_place_name: None,
            bound_place_id: None,
            reference_paths: Vec::new(),
            reference_path: None,
            engine: String::new(),
        }
    };
    project.path = dir.to_string_lossy().into();
    project.engine = "unreal".into();
    save_project(&project)?;
    write_unreal_bridge(&dir)?;
    Ok(project)
}

#[tauri::command]
pub fn open_in_unreal(path: String) -> Result<(), String> {
    let file = uproject_file(Path::new(&path)).ok_or("Fichier .uproject introuvable")?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(file)
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir Unreal : {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Err("Ouverture d'Unreal prise en charge sur Windows seulement".into())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnrealEditorStatus {
    pub running: bool,
    pub server: Option<String>,
}

/// Le MCP de l'éditeur répond-il ? Un `initialize` JSON-RPC suffit (réponse JSON ou flux SSE).
#[tauri::command(async)]
pub fn unreal_editor_status() -> UnrealEditorStatus {
    let off = UnrealEditorStatus { running: false, server: None };
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(1500))
        .build()
    else {
        return off;
    };
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "lumen-status", "version": "1.0" }
        }
    });
    let Ok(res) = client
        .post(MCP_URL)
        .header("accept", "application/json, text/event-stream")
        .json(&body)
        .send()
    else {
        return off;
    };
    let text = res.text().unwrap_or_default();
    let json = text
        .lines()
        .map(|l| l.trim().trim_start_matches("data:").trim())
        .find(|l| l.starts_with('{'))
        .and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok());
    match json {
        Some(j) if j.get("result").is_some() => UnrealEditorStatus {
            running: true,
            server: j
                .pointer("/result/serverInfo/name")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        },
        _ => off,
    }
}

fn engine_association(dir: &Path) -> String {
    uproject_file(dir)
        .and_then(|f| parse_json_file::<serde_json::Value>(&f).ok())
        .and_then(|j| j.get("EngineAssociation").and_then(|v| v.as_str()).map(str::to_string))
        .unwrap_or_default()
}

/// Dossier du moteur (version du launcher, ex. « 5.8 »). Les builds source (GUID) ne sont pas cherchés ici.
fn engine_dir(dir: &Path) -> Option<PathBuf> {
    let assoc = engine_association(dir);
    if assoc.is_empty() {
        return None;
    }
    let from_launcher = parse_json_file::<serde_json::Value>(Path::new(
        r"C:\ProgramData\Epic\UnrealEngineLauncher\LauncherInstalled.dat",
    ))
    .ok()
    .and_then(|j| {
        j.get("InstallationList")?.as_array()?.iter().find_map(|item| {
            if item.get("AppName")?.as_str()? != format!("UE_{assoc}") {
                return None;
            }
            Some(PathBuf::from(item.get("InstallLocation")?.as_str()?))
        })
    });
    from_launcher
        .into_iter()
        .chain([PathBuf::from(format!(r"C:\Program Files\Epic Games\UE_{assoc}"))])
        .find(|p| p.join("Engine").join("Build").join("BatchFiles").join("Build.bat").is_file())
}

fn vswhere() -> PathBuf {
    PathBuf::from(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe")
}

fn vs_install_path(require: Option<&str>) -> Option<String> {
    let mut cmd = std::process::Command::new(vswhere());
    cmd.args(["-latest", "-products", "*", "-property", "installationPath"]);
    if let Some(component) = require {
        cmd.args(["-requires", component]);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let out = cmd.output().ok()?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then_some(path)
}

fn netfx_sdk_installed() -> bool {
    fs::read_dir(r"C:\Program Files (x86)\Windows Kits\NETFXSDK")
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("4."))
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnrealToolchain {
    pub engine: Option<String>,
    pub association: String,
    pub vs_cpp: bool,
    pub netfx: bool,
}

/// Ce qu'il faut pour compiler le C++ du projet : le moteur, Visual Studio (outils C++) et le SDK .NET Framework.
#[tauri::command(async)]
pub fn unreal_toolchain(path: String) -> UnrealToolchain {
    let dir = PathBuf::from(&path);
    UnrealToolchain {
        engine: engine_dir(&dir).map(|p| p.to_string_lossy().into_owned()),
        association: engine_association(&dir),
        vs_cpp: vs_install_path(Some("Microsoft.VisualStudio.Component.VC.Tools.x86.x64")).is_some(),
        netfx: netfx_sdk_installed(),
    }
}

/// Ouvre Visual Studio Installer avec le SDK .NET Framework 4.8 déjà coché : l'utilisateur clique « Modifier ».
/// Pas de --passive : l'installeur le refuse (code 5007) s'il n'est pas lancé en administrateur dès le départ.
#[tauri::command]
pub fn install_netfx_sdk() -> Result<(), String> {
    let path = vs_install_path(None).ok_or("Visual Studio 2022 introuvable : installe-le avec « Développement Desktop en C++ ».")?;
    let installer = PathBuf::from(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vs_installer.exe");
    if !installer.is_file() {
        return Err("Visual Studio Installer introuvable".into());
    }
    std::process::Command::new(installer)
        .args(["modify", "--installPath", &path, "--add", NETFX_COMPONENT])
        .spawn()
        .map_err(|e| format!("Impossible de lancer Visual Studio Installer : {e}"))?;
    Ok(())
}

// ---------- Fichiers écrits dans le projet ----------

/// Active les plugins MCP dans le .uproject (éditeur seulement : rien n'est embarqué dans le jeu packagé).
fn enable_mcp_plugins(dir: &Path) -> Result<(), String> {
    let file = uproject_file(dir).ok_or("Fichier .uproject introuvable")?;
    let raw = fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let mut root = read_json_object(&file);
    let obj = root.as_object_mut().ok_or("Fichier .uproject illisible")?;
    if obj.is_empty() {
        return Err("Fichier .uproject illisible".into());
    }
    let plugins = obj.entry("Plugins").or_insert_with(|| serde_json::json!([]));
    if !plugins.is_array() {
        *plugins = serde_json::json!([]);
    }
    let list = plugins.as_array_mut().expect("tableau JSON");
    for name in MCP_PLUGINS {
        match list
            .iter_mut()
            .find(|p| p.get("Name").and_then(|n| n.as_str()) == Some(name))
        {
            Some(entry) => {
                entry["Enabled"] = serde_json::json!(true);
            }
            None => list.push(serde_json::json!({
                "Name": name,
                "Enabled": true,
                "TargetAllowList": ["Editor"],
            })),
        }
    }
    // Indentation par tabulations, comme les .uproject écrits par l'éditeur.
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, serde_json::ser::PrettyFormatter::with_indent(b"\t"));
    serde::Serialize::serialize(&root, &mut ser).map_err(|e| e.to_string())?;
    let next = String::from_utf8(buf).map_err(|e| e.to_string())?;
    if next.trim() != raw.trim() {
        fs::write(&file, next).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Démarre le serveur MCP avec l'éditeur. Couche « Default » de la config : les réglages faits
/// dans l'éditeur (Saved/Config) restent prioritaires.
fn enable_mcp_autostart(dir: &Path) -> Result<(), String> {
    const SECTION: &str = "[/Script/ModelContextProtocolEngine.ModelContextProtocolSettings]";
    let path = dir.join("Config").join("DefaultEditorPerProjectUserSettings.ini");
    let current = fs::read_to_string(&path).unwrap_or_default();
    let next = match current.find(SECTION) {
        Some(start) => {
            let body_start = start + SECTION.len();
            let body_end = current[body_start..]
                .find("\n[")
                .map(|i| body_start + i)
                .unwrap_or(current.len());
            let body = &current[body_start..body_end];
            if body.lines().any(|l| l.trim().eq_ignore_ascii_case("bAutoStartServer=True")) {
                return Ok(());
            }
            let kept: Vec<&str> = body
                .lines()
                .filter(|l| !l.trim().to_ascii_lowercase().starts_with("bautostartserver="))
                .filter(|l| !l.trim().is_empty())
                .collect();
            let mut section = format!("{SECTION}\nbAutoStartServer=True");
            for line in kept {
                section.push('\n');
                section.push_str(line);
            }
            format!("{}{}\n{}", &current[..start], section, &current[body_end..])
        }
        None => format!("{}\n{SECTION}\nbAutoStartServer=True\n", current.trim_end()).trim_start().to_string(),
    };
    fs::create_dir_all(dir.join("Config")).map_err(|e| e.to_string())?;
    fs::write(&path, next).map_err(|e| e.to_string())
}

/// Toolset MCP de Lumen (import glTF, Python libre), chargé par Unreal via Content/Python/init_unreal.py.
/// Un init_unreal.py écrit par quelqu'un d'autre n'est jamais remplacé : on n'y ajoute que l'import.
fn write_python_toolset(dir: &Path) -> Result<(), String> {
    const MARK: &str = "# Écrit par Lumen.";
    let py = dir.join("Content").join("Python");
    fs::create_dir_all(&py).map_err(|e| e.to_string())?;
    fs::write(py.join("lumen_toolset.py"), include_str!("../resources/unreal/lumen_toolset.py"))
        .map_err(|e| e.to_string())?;
    let init = py.join("init_unreal.py");
    let current = fs::read_to_string(&init).unwrap_or_default();
    let next = if current.trim().is_empty() || current.starts_with(MARK) {
        include_str!("../resources/unreal/init_unreal.py").to_string()
    } else if current.contains("lumen_toolset") {
        return Ok(());
    } else {
        format!("{}

try:
    import lumen_toolset  # noqa: F401 — ajouté par Lumen
except Exception:
    pass
", current.trim_end())
    };
    if next != current {
        fs::write(&init, next).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn merge_mcp_entry(path: &Path, root_key: &str, entry: serde_json::Value) -> Result<(), String> {
    let mut root = read_json_object(path);
    let servers = root
        .as_object_mut()
        .expect("objet JSON")
        .entry(root_key)
        .or_insert_with(|| serde_json::json!({}));
    if !servers.is_object() {
        *servers = serde_json::json!({});
    }
    servers.as_object_mut().expect("objet JSON").insert(MCP_NAME.into(), entry);
    write_json(path, &root)
}

/// Même format que la commande de l'éditeur `ModelContextProtocol.GenerateClientConfig`.
fn write_mcp_configs(dir: &Path) -> Result<(), String> {
    merge_mcp_entry(&dir.join(".mcp.json"), "mcpServers", serde_json::json!({ "type": "http", "url": MCP_URL }))?;
    merge_mcp_entry(&dir.join(".cursor").join("mcp.json"), "mcpServers", serde_json::json!({ "url": MCP_URL }))?;
    merge_mcp_entry(&dir.join(".gemini").join("settings.json"), "mcpServers", serde_json::json!({ "httpUrl": MCP_URL }))?;

    let settings_path = dir.join(".claude").join("settings.local.json");
    let mut settings = read_json_object(&settings_path);
    settings
        .as_object_mut()
        .expect("objet JSON")
        .insert("enabledMcpjsonServers".into(), serde_json::json!([MCP_NAME]));
    write_json(&settings_path, &settings)?;

    let codex_path = dir.join(".codex").join("config.toml");
    let current = fs::read_to_string(&codex_path).unwrap_or_default();
    if current.trim().is_empty() || current.starts_with("# lumen-unreal") {
        fs::create_dir_all(dir.join(".codex")).map_err(|e| e.to_string())?;
        fs::write(
            &codex_path,
            format!("# lumen-unreal : écrit par Lumen\n[mcp_servers.{MCP_NAME}]\nurl = \"{MCP_URL}\"\n"),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

const UNREAL_INTRO: &str = r#"
Jeu Unreal Engine relié à Lumen.

## Stack
- Gameplay en **C++** dans `Source/<Module>/` (`.h` / `.cpp`). Assets, niveaux et Blueprints dans `Content/`, modifiés **par le MCP de l'éditeur**.
- Ne modifie jamais à la main les `.uasset` / `.umap` (binaires), ni `Binaries/`, `Intermediate/`, `Saved/`, `DerivedDataCache/`.
- Il n'y a ni Roblox, ni Verse ici.
"#;

const UNREAL_CONSIGNE: &str = r#"
## Consigne pour l'agent
Tu construis un jeu Unreal jouable. La logique vit en C++ : des `UCLASS` (`AActor`, `UActorComponent`, `ACharacter`, `AGameModeBase`…) avec les réglages en `UPROPERTY(EditAnywhere, BlueprintReadWrite)` et les actions en `UFUNCTION(BlueprintCallable)`. Ensuite, dans l'éditeur (MCP), crée des Blueprints dérivés de ces classes, règle-les et place-les dans le niveau.

**Compile toi-même** après chaque modification C++ :

```
node tools/lumen-unreal.mjs status
node tools/lumen-unreal.mjs init-cpp
node tools/lumen-unreal.mjs build
```

- `init-cpp` : seulement si `status` dit qu'il n'y a pas de module C++ (projet Blueprint). Crée `Source/` et déclare le module.
- `build` : éditeur fermé → compilation complète (UnrealBuildTool). Éditeur ouvert → **Live Coding** par le MCP. Corrige et relance jusqu'à **0 erreur** avant de rendre la main. Ne dis jamais que c'est fini sans un build réussi.
- Live Coding ne recharge de façon fiable que le corps des fonctions. Nouvelle classe, nouveau `UPROPERTY` / `UFUNCTION`, `.h` modifié en profondeur, `.Build.cs` touché, ou premier build après `init-cpp` : demande à l'utilisateur de **fermer l'éditeur**, lance `build`, puis dis-lui de rouvrir le projet.
- Si `build` signale un outil manquant (Visual Studio, SDK .NET Framework 4.8) : dis-le à l'utilisateur, Lumen a un bouton pour l'installer. Ne contourne pas.
- Les erreurs déjà présentes dans des fichiers que tu n'as pas touchés : signale-les, et ne les corrige que si l'utilisateur est d'accord.

Livre une première version qui compile, puis itère. Ne demande pas de confirmer les étapes évidentes.
"#;

const UNREAL_API_SECTION: &str = r#"
## API Unreal (référence)
N'invente pas de fonction. Les en-têtes du moteur sont lisibles (lecture seule) :
{engine}
Cherche avec grep (`Runtime/Engine/Classes/GameFramework/Actor.h`, `Runtime/Engine/Classes/Components/`…) plutôt que de tout lire : c'est énorme.
"#;

const UNREAL_EDITOR_SECTION: &str = r#"
## Éditeur Unreal (MCP)
Tu as le serveur MCP `unreal-mcp` : celui d'Epic, intégré à l'éditeur, qui démarre avec le projet. Il pilote l'éditeur ouvert : acteurs, niveau, assets, Blueprints, matériaux, UMG, PCG, Niagara, Live Coding…

- Il n'expose que `list_toolsets`, `describe_toolset` (`toolset_name`) et `call_tool` (`toolset_name`, `tool_name` sans le préfixe, `arguments`). Commence par `list_toolsets`, puis `describe_toolset` sur ceux qu'il te faut, puis `call_tool`.
- Toolsets utiles : `editor_toolset.toolsets.actor.ActorTools`, `.scene.SceneTools`, `.asset.AssetTools`, `.blueprint.BlueprintTools`, `.material.MaterialTools`, `EditorToolset.EditorAppToolset` (sélection, caméra, Play in Editor), `EditorToolset.LogsToolset` (journal), `UMGToolSet.UMGToolSet`, `LiveCodingToolset.LiveCodingToolset`.
- **`lumen_toolset.LumenTools`** (ajouté par Lumen) : `import_file` importe n'importe quel fichier (`.glb`, `.gltf`, `.fbx`, `.obj`, `.png`, `.wav`…), chemin relatif au projet accepté ; `execute_python` exécute du Python avec le module `unreal` complet, pour ce qu'aucun outil ne couvre. Code court et ciblé, jamais de boucle de suppression large.
- **Fais-le toi-même** au lieu de le demander : importer un fichier de `assets/`, poser et régler des acteurs, créer un Blueprint enfant de ta classe C++, modifier ses propriétés. Ne demande à l'utilisateur que ce que les outils ne savent pas faire (tester en jeu, choix de design).
- Les chemins de contenu commencent par `/Game/` (= `Content/`).
- Avant de modifier le niveau, regarde ce qui existe. Ne supprime que ce que tu as posé, ou ce que l'utilisateur a demandé.
- Après une série de modifications réussies : sauvegarde le niveau et les assets touchés.
- Si le MCP ne répond pas : l'éditeur est fermé, ou le serveur n'a pas démarré. Demande à l'utilisateur d'ouvrir le projet (bouton **Ouvrir dans Unreal** de Lumen), ou de taper `ModelContextProtocol.StartServer` dans la console de l'éditeur. En attendant, ne prétends pas avoir modifié le niveau.
- Si `lumen_toolset.LumenTools` manque : le plugin Python est désactivé ou l'éditeur n'a pas été relancé depuis l'ajout à Lumen. Demande de relancer l'éditeur.
"#;

const UNREAL_ASSET_SECTION: &str = r#"
## Assets (banque Lumen + VibeStarter)
Même banque que pour Roblox. Les clés API sont dans Lumen. **Cherche d'abord**, propose, puis récupère.

```
node tools/lumen-asset.mjs search mesh totem --for "totem décoratif au spawn"
node tools/lumen-asset.mjs search image coin --for "icône pièce dans le HUD"
node tools/lumen-asset.mjs search texture pierre --for "sol du donjon"
node tools/lumen-asset.mjs search inspiration hud --for "layout boutique"
node tools/lumen-asset.mjs get VS-0124
node tools/lumen-asset.mjs image "icône pièce d'or, PNG fond transparent"
node tools/lumen-asset.mjs mesh "coffre low poly stylisé"
node tools/lumen-asset.mjs blender assets/blender/crate.py Crate
```

1. `search … --for "à quoi ça sert"` : Lumen montre un menu d'environ 10 assets, l'utilisateur en choisit **un**. Ensuite **un seul** `get` sur le code choisi.
2. `get CODE` : Lumen copie le fichier dans `assets/images/`, `assets/meshes/` ou `assets/textures/` (chemin affiché). Rien n'est publié sur Roblox.
3. Importe-le toi-même : `call_tool` → `lumen_toolset.LumenTools` / `import_file` avec `source_file` = le chemin affiché (ex. `assets/meshes/SM_Coffre_VS_0124.glb`), `folder_path` = `/Game/Lumen/Meshes` (ou `Textures`, `UI`, `Sounds`), `asset_name` = le nom du fichier sans extension. Pas `StaticMeshTools.import_file` : il refuse les `.glb`.
4. En C++, référence l'asset par une propriété (`UPROPERTY(EditAnywhere) TObjectPtr<UStaticMesh> Mesh;`) que tu règles sur le Blueprint, plutôt qu'un chemin en dur.
5. `image` / `mesh` / `blender` seulement si la recherche est vide ou si l'utilisateur a cliqué Aucune.
6. Inspiration UI (`INS-xxxx`) : `get` copie l'image dans `assets/inspiration/`. Lis-la et reproduis l'esprit en UMG. Ne l'importe pas.
"#;

pub fn write_unreal_bridge(dir: &Path) -> Result<(), String> {
    let project: Project = parse_json_file(&meta_path(dir))?;
    for sub in ["images", "meshes", "textures", "inspiration", "inbox", "blender"] {
        fs::create_dir_all(dir.join("assets").join(sub)).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(dir.join("tools")).map_err(|e| e.to_string())?;
    for (name, body) in [
        ("lumen-asset.mjs", include_str!("../resources/lumen-asset.mjs")),
        ("lumen-blender-run.py", include_str!("../resources/lumen-blender-run.py")),
        ("lumen-ref.mjs", include_str!("../resources/lumen-ref.mjs")),
        ("lumen-unreal.mjs", include_str!("../resources/lumen-unreal.mjs")),
    ] {
        fs::write(dir.join("tools").join(name), body).map_err(|e| e.to_string())?;
    }
    let skill = include_str!("../resources/lumen-assets-unreal.SKILL.md");
    for agent in [".cursor", ".claude", ".codex", ".agents"] {
        let skill_dir = dir.join(agent).join("skills").join("lumen-assets");
        fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
        fs::write(skill_dir.join("SKILL.md"), skill).map_err(|e| e.to_string())?;
    }

    enable_mcp_plugins(dir)?;
    enable_mcp_autostart(dir)?;
    write_python_toolset(dir)?;

    let engine_source = engine_dir(dir).map(|p| p.join("Engine").join("Source"));
    let engine_line = match &engine_source {
        Some(src) => format!("- `{}/`", slash_path(src)),
        None => format!(
            "- Moteur {} introuvable : installe-le depuis l'Epic Games Launcher.",
            engine_association(dir)
        ),
    };
    let api = UNREAL_API_SECTION.replace("{engine}", &engine_line);
    let agents_path = dir.join("AGENTS.md");
    let current = fs::read_to_string(&agents_path).unwrap_or_default();
    let base = if current.trim().is_empty() {
        format!("# {}\n{}", project.name, UNREAL_INTRO)
    } else {
        current.clone()
    };
    let next = upsert_section(&base, "## Consigne pour l'agent", UNREAL_CONSIGNE);
    let next = upsert_section(&next, "## API Unreal", &api);
    let next = upsert_section(&next, "## Éditeur Unreal", UNREAL_EDITOR_SECTION);
    let next = upsert_section(&next, "## Assets", UNREAL_ASSET_SECTION);
    let next = upsert_ref_section(&next, &ref_section_for(dir));
    if next != current {
        fs::write(&agents_path, next).map_err(|e| e.to_string())?;
    }

    let extra_dirs = engine_source
        .map(|p| vec![p.to_string_lossy().into_owned()])
        .unwrap_or_default();
    write_claude_settings(dir, extra_dirs)?;
    write_mcp_configs(dir)?;

    let claude_path = dir.join("CLAUDE.md");
    if !claude_path.exists() {
        fs::write(
            claude_path,
            "# Lumen (Unreal)\n\nLis `AGENTS.md`. C++ dans `Source/`, compilé avec `node tools/lumen-unreal.mjs build`. Éditeur : MCP `unreal-mcp`.\n",
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
