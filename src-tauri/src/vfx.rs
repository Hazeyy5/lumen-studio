//! Effets visuels décrits en JSON (`assets/vfx/<Nom>.vfx.json`) : lus par l'onglet VFX de Lumen,
//! convertis pour le jeu par `tools/lumen-vfx.mjs sync`.

use serde::Serialize;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

const TEXTURES: &[(&str, &[u8])] = &[
    ("glow.png", include_bytes!("../resources/vfx-textures/glow.png")),
    ("spark.png", include_bytes!("../resources/vfx-textures/spark.png")),
    ("ring.png", include_bytes!("../resources/vfx-textures/ring.png")),
    ("smoke.png", include_bytes!("../resources/vfx-textures/smoke.png")),
    ("shard.png", include_bytes!("../resources/vfx-textures/shard.png")),
    ("star.png", include_bytes!("../resources/vfx-textures/star.png")),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VfxFile {
    pub name: String,
    pub path: String,
    pub modified: u64,
    pub text: String,
}

/// Textures de base, outil de conversion et un exemple (seulement s'il n'y a encore aucun effet).
pub fn write_vfx_kit(dir: &Path) -> Result<(), String> {
    let vfx = dir.join("assets").join("vfx");
    let tex = vfx.join("textures");
    fs::create_dir_all(&tex).map_err(|e| e.to_string())?;
    for (name, bytes) in TEXTURES {
        fs::write(tex.join(name), bytes).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(dir.join("tools")).map_err(|e| e.to_string())?;
    fs::write(dir.join("tools").join("lumen-vfx.mjs"), include_str!("../resources/lumen-vfx.mjs"))
        .map_err(|e| e.to_string())?;
    let has_effect = fs::read_dir(&vfx)
        .map(|it| it.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".vfx.json")))
        .unwrap_or(false);
    if !has_effect {
        fs::write(vfx.join("ExempleImpactGlace.vfx.json"), include_str!("../resources/vfx-example.json"))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Effets du projet, avec leur date de modification (l'onglet recharge quand l'agent modifie un fichier).
#[tauri::command(async)]
pub fn list_vfx(project_path: String) -> Result<Vec<VfxFile>, String> {
    let dir = Path::new(&project_path).join("assets").join("vfx");
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(out);
    };
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        let Some(name) = file.strip_suffix(".vfx.json") else {
            continue;
        };
        let path = entry.path();
        let modified = fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        out.push(VfxFile {
            name: name.to_string(),
            path: path.to_string_lossy().into(),
            modified,
            text: fs::read_to_string(&path).unwrap_or_default(),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
