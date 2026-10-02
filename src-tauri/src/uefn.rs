use crate::bank::{is_texture, BankItem};
use crate::projects::{
    chrono_like_now, fortnite_projects_root, meta_path, parse_json_file, read_project_meta,
    ref_section_for, save_project, upsert_ref_section, write_claude_settings, Project,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UefnProject {
    pub name: String,
    pub path: String,
    pub linked: bool,
}

fn uefnproject_file(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("uefnproject"))
    })
}

fn uefn_title(dir: &Path) -> String {
    let folder = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Projet UEFN".into());
    uefnproject_file(dir)
        .and_then(|file| parse_json_file::<serde_json::Value>(&file).ok())
        .and_then(|json| json.get("title").and_then(|t| t.as_str()).map(str::to_string))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or(folder)
}

/// Dossier (créé par UEFN) qui contient les digests Verse : l'API Fortnite et les assets du projet.
fn verse_project_dir(dir: &Path) -> Option<PathBuf> {
    let folder = dir.file_name()?;
    let path = dirs::data_local_dir()?
        .join("UnrealEditorFortnite")
        .join("Saved")
        .join("VerseProject")
        .join(folder);
    path.is_dir().then_some(path)
}

fn inside_fortnite_root(dir: &Path) -> Result<PathBuf, String> {
    let root = fortnite_projects_root().ok_or("Dossier Documents/Fortnite Projects introuvable")?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let canon = dir.canonicalize().map_err(|_| "Projet UEFN introuvable".to_string())?;
    if canon.parent() != Some(root.as_path()) {
        return Err("Le projet doit être dans Documents/Fortnite Projects".into());
    }
    Ok(canon)
}

#[tauri::command(async)]
pub fn list_uefn_projects() -> Result<Vec<UefnProject>, String> {
    let Some(root) = fortnite_projects_root() else {
        return Ok(Vec::new());
    };
    let mut out: Vec<UefnProject> = fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && uefnproject_file(p).is_some())
        .map(|p| UefnProject {
            name: uefn_title(&p),
            linked: meta_path(&p).is_file(),
            path: p.to_string_lossy().into(),
        })
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    Ok(out)
}

#[tauri::command(async)]
pub fn link_uefn_project(path: String) -> Result<Project, String> {
    let dir = PathBuf::from(path.trim());
    inside_fortnite_root(&dir)?;
    if uefnproject_file(&dir).is_none() {
        return Err("Ce dossier n’est pas un projet UEFN (.uefnproject absent)".into());
    }
    let mut project = if meta_path(&dir).is_file() {
        read_project_meta(&dir.to_string_lossy())?
    } else {
        Project {
            name: uefn_title(&dir),
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
    project.engine = "uefn".into();
    save_project(&project)?;
    write_uefn_bridge(&dir)?;
    Ok(project)
}

#[tauri::command]
pub fn open_in_uefn(path: String) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    let file = uefnproject_file(&dir).ok_or("Fichier .uefnproject introuvable")?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(file)
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir UEFN : {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Err("UEFN n'existe que sur Windows".into())
    }
}

fn safe_file_part(text: &str) -> String {
    let out: String = text
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    out.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_")
}

/// Pour un projet UEFN, `get` copie le fichier dans `assets/` au lieu de publier sur Roblox.
/// Le nom commence par `T_` / `SM_` pour suivre la convention Unreal ; c'est aussi le nom Verse après import.
pub fn copy_asset_for_uefn(item: &BankItem, project_path: &str) -> Result<(String, String), String> {
    let project = crate::assets::assert_lumen_project(project_path)?;
    let local = crate::catalog::materialize(item)?;
    let source = Path::new(&local.path);
    if !source.is_file() {
        return Err("Fichier de l’asset introuvable sur ce PC".into());
    }
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    let (folder, prefix) = if is_texture(item) {
        ("textures", "T")
    } else if item.kind == "mesh" {
        ("meshes", "SM")
    } else {
        ("images", "T")
    };
    let mut name = safe_file_part(&item.name);
    name.truncate(40);
    let code = safe_file_part(&item.code);
    let stem = if name.is_empty() {
        format!("{prefix}_{code}")
    } else {
        format!("{prefix}_{name}_{code}")
    };
    let dir = project.join("assets").join(folder);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(format!("{stem}.{ext}"));
    fs::copy(source, &dest).map_err(|e| e.to_string())?;
    Ok((
        dest.to_string_lossy().into_owned(),
        format!("assets/{folder}/{stem}.{ext}"),
    ))
}

pub fn is_uefn_project(project_path: &str) -> bool {
    !project_path.trim().is_empty()
        && read_project_meta(project_path).is_ok_and(|p| p.is_uefn())
}

const UEFN_INTRO: &str = r#"
Jeu Fortnite (UEFN) relié à Lumen.

## Stack
- Le code est en **Verse**, dans `Content/` (fichiers `*.verse`). Range les nouveaux fichiers dans `Content/`, ou un sous-dossier de `Content/`.
- Ne touche jamais aux `.uasset`, `.umap`, `__ExternalActors__`, `__ExternalObjects__`, ni au `.uefnproject` / `.uplugin`. C'est UEFN qui les gère.
- Il n'y a ni Roblox, ni Rojo, ni TypeScript ici. Pas de `rbxassetid`.
"#;

const UEFN_CONSIGNE: &str = r#"
## Consigne pour l'agent
Tu construis une île Fortnite jouable avec Verse. La logique vit dans des classes `creative_device` : l'utilisateur pose le device Verse dans la map, puis relie les champs `@editable` aux devices de la scène (dans le panneau Détails de UEFN). Dis-lui toujours quels devices poser et quoi relier.

**Compile toi-même** après chaque modification, avec l'UEFN ouvert sur ce projet :

```
node tools/lumen-verse.mjs build
node tools/lumen-verse.mjs push
node tools/lumen-verse.mjs status
```

- `build` lance la compilation Verse dans UEFN et affiche les erreurs (`fichier(ligne,colonne) : message`). Corrige, puis relance `build` jusqu'à **0 erreur**, avant de rendre la main. Ne dis jamais que c'est fini sans un build à 0 erreur.
- Si UEFN n'est pas joignable, ou s'il a un autre projet ouvert, demande à l'utilisateur d'ouvrir ce projet dans UEFN. Ne prétends pas que le code compile.
- `push` envoie le Verse compilé dans la session de test en cours (Push Verse Changes). Sans session lancée, dis à l'utilisateur de la lancer depuis UEFN.
- Les erreurs déjà présentes dans des fichiers que tu n'as pas touchés : signale-les, et ne les corrige que si l'utilisateur est d'accord.

Livre une première version qui compile, puis itère. Ne demande pas de confirmer les étapes évidentes.
"#;

const UEFN_API_SECTION: &str = r#"
## API Verse (référence)
N'invente pas de fonction. Vérifie dans les digests générés par UEFN (lecture seule) :
{digests}
`Fortnite.digest.verse` (devices, joueurs, UI), `Verse.digest.verse` (langage, Simulation), `UnrealEngine.digest.verse` (UI, maths spatiales). Cherche dedans avec grep plutôt que de tout lire : ils sont gros.

Les assets déjà importés dans UEFN sont dans `*-Assets.digest.verse` : un dossier de `Content/` devient un module (`Icons.UI : texture` pour `Content/Icons/UI`). Après un import, ce fichier n'est mis à jour qu'au prochain Build Verse Code.
"#;

const UEFN_ASSET_SECTION: &str = r#"
## Assets (banque Lumen + VibeStarter)
Même banque que pour Roblox. Les clés API sont dans Lumen. **Cherche d'abord**, propose, puis récupère.

```
node tools/lumen-asset.mjs search mesh totem --for "totem décoratif au spawn"
node tools/lumen-asset.mjs search image coin --for "icône pièce dans le HUD"
node tools/lumen-asset.mjs search texture bouton --for "fond de bouton shop"
node tools/lumen-asset.mjs search inspiration hud --for "layout boutique"
node tools/lumen-asset.mjs get VS-0124
node tools/lumen-asset.mjs image "icône pièce d'or, PNG fond transparent"
node tools/lumen-asset.mjs icon VS-0124
node tools/lumen-asset.mjs mesh "coffre low poly stylisé"
node tools/lumen-asset.mjs blender assets/blender/crate.py Crate
```

1. `search … --for "à quoi ça sert"` : Lumen montre un menu d'environ 10 assets, l'utilisateur en choisit **un**. Ensuite **un seul** `get` sur le code choisi.
2. `get CODE` : l'utilisateur valide dans Lumen, puis Lumen copie le fichier dans `assets/images/`, `assets/meshes/` ou `assets/textures/` (chemin affiché par la commande). Rien n'est publié sur Roblox.
3. Le fichier n'est **pas encore dans UEFN**. Demande à l'utilisateur de le glisser dans le Content Browser de UEFN, dans un dossier précis que tu choisis (ex. `Content/Lumen/Icons`). Les `.png` deviennent des textures. Les `.glb` s'importent comme Static Mesh ; si UEFN refuse le fichier, convertis-le en `.fbx` avec un script Blender.
4. Après import et `node tools/lumen-verse.mjs build`, l'asset est utilisable en Verse par son module : `Lumen.Icons.T_coin_VS_0124` (vérifie le nom exact dans `*-Assets.digest.verse`). Une texture va dans un `texture_block{DefaultImage := …}`. Un mesh se pose dans la map ou via un device (Prop Mover, Prop-o-Matic…), pas en Verse brut.
5. `image` / `mesh` / `blender` seulement si la recherche est vide ou si l'utilisateur a cliqué Aucune. Icônes et props 2D : **PNG fond transparent**.
6. Inspiration UI (`INS-xxxx`) : `get` copie l'image dans `assets/inspiration/`. Lis-la et reproduis l'esprit (layout, couleurs) en UI Verse. Ne l'importe pas dans UEFN.
"#;

fn digests_lines(dir: &Path) -> (String, Vec<String>) {
    let Some(vp) = verse_project_dir(dir) else {
        return (
            "- Pas encore générés. Ouvre le projet dans UEFN puis lance une fois **Verse → Build Verse Code** : ils apparaîtront dans `%LOCALAPPDATA%/UnrealEditorFortnite/Saved/VerseProject/<projet>/Digests/`.".into(),
            Vec::new(),
        );
    };
    let digests = vp.join("Digests");
    let path = digests.to_string_lossy().replace('\\', "/");
    (
        format!("- `{path}/BuiltIn/` (API) et `{path}/` (assets du projet)"),
        vec![digests.to_string_lossy().into_owned()],
    )
}

fn section_bounds(text: &str, heading: &str) -> Option<(usize, usize)> {
    let start = text.find(heading)?;
    let after = &text[start..];
    let end = after
        .find('\n')
        .and_then(|nl| after[nl + 1..].find("\n## ").map(|i| start + nl + 1 + i))
        .unwrap_or(text.len());
    Some((start, end))
}

fn upsert_section(current: &str, heading: &str, section: &str) -> String {
    let section = section.trim();
    match section_bounds(current, heading) {
        Some((start, end)) => {
            let before = current[..start].trim_end();
            let rest = current[end..].trim_start();
            if rest.is_empty() {
                format!("{before}\n\n{section}\n")
            } else {
                format!("{before}\n\n{section}\n\n{rest}")
            }
        }
        None => format!("{}\n\n{section}\n", current.trim_end()),
    }
}

pub fn write_uefn_bridge(dir: &Path) -> Result<(), String> {
    let project: Project = parse_json_file(&meta_path(dir))?;
    for sub in ["images", "meshes", "textures", "inspiration", "inbox", "blender"] {
        fs::create_dir_all(dir.join("assets").join(sub)).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(dir.join("tools")).map_err(|e| e.to_string())?;
    for (name, body) in [
        ("lumen-asset.mjs", include_str!("../resources/lumen-asset.mjs")),
        ("lumen-blender-run.py", include_str!("../resources/lumen-blender-run.py")),
        ("lumen-ref.mjs", include_str!("../resources/lumen-ref.mjs")),
        ("lumen-verse.mjs", include_str!("../resources/lumen-verse.mjs")),
    ] {
        fs::write(dir.join("tools").join(name), body).map_err(|e| e.to_string())?;
    }
    let skill = include_str!("../resources/lumen-assets-uefn.SKILL.md");
    for agent in [".cursor", ".claude", ".codex", ".agents"] {
        let skill_dir = dir.join(agent).join("skills").join("lumen-assets");
        fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
        fs::write(skill_dir.join("SKILL.md"), skill).map_err(|e| e.to_string())?;
    }

    let (digests, extra_dirs) = digests_lines(dir);
    let api = UEFN_API_SECTION.replace("{digests}", &digests);
    let agents_path = dir.join("AGENTS.md");
    let current = fs::read_to_string(&agents_path).unwrap_or_default();
    let base = if current.trim().is_empty() {
        format!("# {}\n{}", project.name, UEFN_INTRO)
    } else {
        current.clone()
    };
    let next = upsert_section(&base, "## Consigne pour l'agent", UEFN_CONSIGNE);
    let next = upsert_section(&next, "## API Verse", &api);
    let next = upsert_section(&next, "## Assets", UEFN_ASSET_SECTION);
    let next = upsert_ref_section(&next, &ref_section_for(dir));
    if next != current {
        fs::write(&agents_path, next).map_err(|e| e.to_string())?;
    }

    write_claude_settings(dir, extra_dirs)?;

    let claude_path = dir.join("CLAUDE.md");
    if !claude_path.exists() {
        fs::write(
            claude_path,
            "# Lumen (UEFN)\n\nLis `AGENTS.md`. Code Verse dans `Content/`. Banque : `node tools/lumen-asset.mjs search …`.\n",
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
