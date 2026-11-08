"""Importe les notes LihFolk Vault V2 (CC BY 4.0) utiles aux agents : VFX/éclairage/audio et animation.

Usage : python scripts/import_lihfolk_knowledge.py <chemin du dépôt v2-vault cloné>
Écrit src-tauri/resources/knowledge/lihfolk/ (notes + INDEX.md + CREDITS.md) et src-tauri/src/knowledge_files.rs.
"""
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "src-tauri" / "resources" / "knowledge" / "lihfolk"
vault = Path(sys.argv[1]) / "LihFolk Vault V2"

SECTIONS = {
    "vfx": [vault / "07 Lighting VFX and Audio"],
    "animation": [vault / "08 Animation" / d for d in (
        "01 Fundamentals", "02 Rigging and Importing", "03 Authoring", "04 Procedural",
        "05 Systems", "07 Gotchas", "08 Snippets",
    )],
}
# Outils internes de LihFolk : seuls ceux qui servent à nos monstres importés sont gardés.
EXTRA_ANIMATION = [vault / "08 Animation" / "06 Pipelines and Tooling" / n for n in (
    "Pipeline - Importing Animated Enemies.md", "Enemy Import Axis Fix.md",
    "Lua Keyframe Player.md", "Lua Keyframe Player Internals.md",
)] + [vault / "08 Animation" / n for n in ("Animation Start Here.md", "Animation Glossary.md")]


def tldr(text):
    m = re.search(r"tldr:start -->(.*?)<!-- tldr:end", text, re.S)
    if not m:
        return []
    lines = [l.strip().lstrip(">").strip() for l in m.group(1).splitlines()]
    return [l for l in lines if l.startswith("- ")]


if DEST.exists():
    shutil.rmtree(DEST)
files = []
index = ["# Index des notes LihFolk (VFX, éclairage, audio, animation)", "",
         "Une ligne par note : chemin, puis son résumé (symptôme -> correction, checklist).",
         "Cherche un mot-clé ici (Grep), puis lis seulement la section utile de la note (`grep -n \"^## \"`).", ""]
for section, dirs in SECTIONS.items():
    sources = []
    for d in dirs:
        sources += sorted(d.glob("*.md"))
    if section == "animation":
        sources += [p for p in EXTRA_ANIMATION if p.exists()]
    index.append(f"## {section}")
    index.append("")
    for src in sources:
        sub = src.parent.name if src.parent.name not in ("07 Lighting VFX and Audio", "08 Animation") else ""
        rel = Path(section) / sub / src.name if sub else Path(section) / src.name
        target = DEST / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        text = src.read_text(encoding="utf-8")
        target.write_text(text, encoding="utf-8")
        files.append(rel.as_posix())
        bullets = tldr(text)
        index.append(f"### {src.stem}")
        index.append(f"`tools/knowledge/lihfolk/{rel.as_posix()}`")
        index += bullets[:6]
        index.append("")

(DEST / "INDEX.md").write_text("\n".join(index), encoding="utf-8")
files.append("INDEX.md")
(DEST / "CREDITS.md").write_text(
    "# Crédits\n\n"
    "Ces notes viennent de **LihFolk Vault V2** (https://github.com/lihfolk-cy/v2-vault), (c) 2026 LihFolk,\n"
    "sous licence **Creative Commons Attribution 4.0 International (CC BY 4.0)** :\n"
    "https://creativecommons.org/licenses/by/4.0/\n\n"
    "Copiées sans modification dans les projets Lumen pour les agents. Les citations de la documentation Roblox\n"
    "qu'elles contiennent restent sous CC BY 4.0, (c) Roblox Corporation.\n",
    encoding="utf-8",
)
files.append("CREDITS.md")

rs = ["// Généré par scripts/import_lihfolk_knowledge.py : ne pas modifier à la main.",
      "pub const LIHFOLK_FILES: &[(&str, &str)] = &["]
for rel in files:
    rs.append(f'    ("{rel}", include_str!("../resources/knowledge/lihfolk/{rel}")),')
rs.append("];")
(ROOT / "src-tauri" / "src" / "knowledge_files.rs").write_text("\n".join(rs) + "\n", encoding="utf-8")
print(len(files), "fichiers ; index", (DEST / "INDEX.md").stat().st_size // 1024, "Ko")
