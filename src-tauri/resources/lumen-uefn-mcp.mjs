#!/usr/bin/env node
/**
 * Serveur MCP (stdio) de Lumen pour piloter l'éditeur UEFN.
 * Relaie chaque outil vers l'écouteur Python lancé dans UEFN (tools/uefn_listener.py,
 * KirChuvakov/uefn-mcp-server, licence MIT), qui écoute sur 127.0.0.1:8765-8770.
 * Pas de dépendance : Node seul, aucun Python côté PC.
 */

import { readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const FIRST_PORT = Number(process.env.UEFN_MCP_PORT || 8765);
const LAST_PORT = 8770;
const PROJECT = process.env.LUMEN_PROJECT || path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const LISTENER = path.join(PROJECT, "tools", "uefn_listener.py").replace(/\\/g, "/");

// Nom du module de contenu UEFN (= nom du .uplugin), ex. "magnet_incremental" pour /magnet_incremental/...
const CONTENT_MODULE = (() => {
  try {
    const file = readdirSync(PROJECT).find((f) => f.toLowerCase().endsWith(".uplugin"));
    return file ? file.slice(0, -".uplugin".length) : "";
  } catch {
    return "";
  }
})();

const NOT_RUNNING = `L'écouteur UEFN ne répond pas (ports ${FIRST_PORT}-${LAST_PORT}). Demande à l'utilisateur, dans UEFN : Outils → Exécuter un script Python → ${LISTENER}. Il faut aussi le plugin « Python Editor Script Plugin » coché dans les Paramètres du projet. Ne fais rien d'autre dans l'éditeur en attendant.`;

let port = null;

async function post(p, command, params, timeoutMs) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch(`http://127.0.0.1:${p}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ command, params: params || {} }),
      signal: controller.signal,
    });
    return await res.json();
  } finally {
    clearTimeout(timer);
  }
}

async function findPort() {
  if (port) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}`, { signal: AbortSignal.timeout(1500) });
      if (res.ok) return port;
    } catch {
      /* rescan */
    }
  }
  for (let p = FIRST_PORT; p <= LAST_PORT; p += 1) {
    try {
      const res = await fetch(`http://127.0.0.1:${p}`, { signal: AbortSignal.timeout(800) });
      const json = await res.json();
      if (json && json.status === "ok") {
        port = p;
        return p;
      }
    } catch {
      /* port suivant */
    }
  }
  port = null;
  return null;
}

async function send(command, params, timeoutMs = 60000) {
  const p = await findPort();
  if (!p) throw new Error(NOT_RUNNING);
  let body;
  try {
    body = await post(p, command, params, timeoutMs);
  } catch (error) {
    port = null;
    if (error.name === "AbortError") throw new Error(`UEFN n'a pas répondu à « ${command} » à temps.`);
    throw new Error(NOT_RUNNING);
  }
  if (!body.success) {
    throw new Error(`UEFN : « ${command} » a échoué : ${body.error || "erreur inconnue"}\n${body.traceback || ""}`.trim());
  }
  return body.result;
}

// UEFN pilote le projet ouvert à l'écran : on refuse d'agir si ce n'est pas celui de l'agent.
let checkedAt = 0;
async function assertProject() {
  if (!CONTENT_MODULE || Date.now() - checkedAt < 30000) return;
  const info = await send("get_project_info");
  const open = String(info.project_name || "");
  if (open && open.toLowerCase() !== CONTENT_MODULE.toLowerCase()) {
    throw new Error(
      `UEFN a le projet « ${open} » ouvert, pas « ${CONTENT_MODULE} ». Demande à l'utilisateur d'ouvrir le bon projet. Rien n'a été modifié.`,
    );
  }
  checkedAt = Date.now();
}

const json = (value) => JSON.stringify(value, null, 2);
const vec3 = (what) => ({ type: "array", items: { type: "number" }, minItems: 3, maxItems: 3, description: what });

function pyString(text) {
  return JSON.stringify(String(text));
}

// Champs @editable d'un device Verse. Python ne les voit que sous leur nom interne
// (« __verse_0xBF219067_Team1 ») : on les lit dans la copie texte du device (EDIT COPY),
// puis on rend le presse-papiers et la sélection de l'utilisateur.
const VERSE_FIELDS_PY = `
import re
def _lumen_actor(label):
    acts = actor_sub.get_all_level_actors()
    for a in acts:
        if a.get_actor_label() == label:
            return a
    close = [a.get_actor_label() for a in acts if label.lower() in a.get_actor_label().lower()][:10]
    raise ValueError(f"Aucun acteur nommé « {label} ». Proches : {close}")

def _lumen_verse_fields(dev):
    script = dev.get_editor_property("Script")
    root = get_tk_root()
    try:
        previous = root.clipboard_get()
    except Exception:
        previous = None
    selection = actor_sub.get_selected_level_actors()
    actor_sub.set_selected_level_actors([dev])
    try:
        unreal.SystemLibrary.execute_console_command(unreal.EditorLevelLibrary.get_editor_world(), "EDIT COPY")
        root.update()
        text = root.clipboard_get()
    finally:
        actor_sub.set_selected_level_actors(selection)
        if previous is not None:
            root.clipboard_clear()
            root.clipboard_append(previous)
            root.update()
    fields = {}
    for mangled, name in re.findall(r"^\\s*(__verse_0x[0-9A-F]+_(\\w+))(?:\\(\\d+\\))?=", text, re.M):
        if name in fields:
            continue
        try:
            fields[name] = (mangled, script.get_editor_property(mangled))
        except Exception:
            pass  # sous-objet d'un champ (événement…), pas un champ du device
    return script, fields

def _lumen_is_list(value):
    return isinstance(value, (unreal.Array, list))

def _lumen_wrapper_target(value):
    # Champ vers un device Fortnite : objet intermédiaire dont SavedActor est l'acteur.
    try:
        return True, value.get_editor_property("SavedActor")
    except Exception:
        return False, None

def _lumen_verse_owner(value):
    # Champ vers un autre device Verse : c'est l'objet Script de cet acteur.
    try:
        outer = value.get_outer()
    except Exception:
        return None
    return outer if isinstance(outer, unreal.Actor) else None

def _lumen_describe(value):
    if _lumen_is_list(value):
        return {"list": [_lumen_describe(v) for v in value]}
    if value is None:
        return {"device": None}
    ok, target = _lumen_wrapper_target(value)
    if ok:
        return {"type": value.get_class().get_name(), "device": target.get_actor_label() if target else None}
    owner = _lumen_verse_owner(value)
    if owner is not None:
        return {"type": value.get_class().get_name(), "device": owner.get_actor_label()}
    return {"value": str(value)}

def _lumen_link_value(script, sample, target):
    # Nouvelle valeur qui relie target, sur le modèle de sample (valeur actuelle du champ).
    ok, _ = _lumen_wrapper_target(sample)
    if ok:
        wrapper = unreal.new_object(sample.get_class(), outer=script)
        wrapper.set_editor_property("SavedActor", target)
        return wrapper
    if _lumen_verse_owner(sample) is not None:
        return target.get_editor_property("Script")
    raise ValueError("Ce champ n'est pas une référence vers un device (nombre, texte…). Règle-le dans le Verse ou le panneau Détails.")

def _lumen_device_schema(dev):
    # Outil interne d'Epic (DeviceToolset) : connaît tous les champs, même les listes vides.
    import json
    if not hasattr(unreal, "DeviceToolset"):
        return {}
    cdo = unreal.get_default_object(unreal.DeviceToolset)
    return json.loads(cdo.call_method("ListDeviceProperties", (dev,)))

def _lumen_empty_lists(dev, known):
    lowered = {k.lower() for k in known}
    return [k for k, s in _lumen_device_schema(dev).items() if s.get("type") == "array" and k.lower() not in lowered]

def _lumen_fill_empty_list(dev, script, name, targets):
    # Liste vide : son nom interne est inconnu, on passe par DeviceToolset avec des éléments neufs.
    import json
    schema = _lumen_device_schema(dev)
    key = next((k for k in schema if k.lower() == name.lower()), None)
    if key is None:
        raise ValueError(f"Champ « {name} » introuvable. Champs du device : {sorted(schema)}.")
    spec = schema[key]
    item = spec.get("items") or {}
    if spec.get("type") != "array" or "refPath" not in (item.get("properties") or {}):
        raise ValueError(f"« {name} » n'est pas une liste de devices Fortnite ; remplis-la dans le panneau Détails.")
    cls = unreal.load_object(None, item["title"])
    refs = []
    for t in targets:
        w = unreal.new_object(cls, outer=script)
        w.set_editor_property("SavedActor", t)
        refs.append({"refPath": w.get_path_name()})
    cdo = unreal.get_default_object(unreal.DeviceToolset)
    cdo.call_method("SetDeviceProperty", (dev, key, json.dumps(refs)))
`;

const TOOLS = [
  {
    name: "ping",
    description: "Vérifie que l'écouteur UEFN tourne et répond.",
    schema: {},
    safe: true,
    run: async () => json(await send("ping")),
  },
  {
    name: "get_project_info",
    description: "Nom du projet ouvert dans UEFN et racine du contenu. En UEFN la racine est '/<Projet>/', pas '/Game/'.",
    schema: {},
    safe: true,
    run: async () => json(await send("get_project_info")),
  },
  {
    name: "execute_python",
    description:
      "Exécute du Python dans l'éditeur UEFN (thread principal, module `unreal`). Variables prêtes : unreal, actor_sub, asset_sub, level_sub. Assigne `result` pour renvoyer une valeur, print() pour la sortie. À utiliser quand aucun outil dédié ne suffit. Ne crée jamais tk.Tk().",
    schema: { code: { type: "string", description: "Code Python à exécuter." } },
    required: ["code"],
    run: async ({ code }) => {
      const r = await send("execute_python", { code }, 120000);
      const parts = [];
      if (r.stdout) parts.push(`stdout:\n${r.stdout}`);
      if (r.stderr) parts.push(`stderr:\n${r.stderr}`);
      if (r.result !== null && r.result !== undefined) parts.push(`result: ${json(r.result)}`);
      return parts.join("\n") || "(aucune sortie)";
    },
  },
  {
    name: "import_file",
    description:
      "Importe un fichier du disque (png, glb, fbx, wav…) dans le Content Browser UEFN. Pour les assets de la banque Lumen récupérés avec `get` (dossier assets/ du projet). Renvoie les chemins UEFN créés.",
    schema: {
      file: { type: "string", description: "Chemin du fichier, absolu ou relatif au projet (ex. assets/meshes/SM_Autel_VS_0124.glb)." },
      destination: { type: "string", description: "Dossier UEFN de destination, ex. /magnet_incremental/Lumen/Meshes. Par défaut /<Projet>/Lumen." },
      replace: { type: "boolean", description: "Remplacer un asset existant du même nom (défaut : true)." },
    },
    required: ["file"],
    run: async ({ file, destination, replace }) => {
      const full = path.resolve(PROJECT, file).replace(/\\/g, "/");
      const dest = destination || (CONTENT_MODULE ? `/${CONTENT_MODULE}/Lumen` : "");
      if (!dest) throw new Error("Précise destination (ex. /MonProjet/Lumen/Meshes).");
      const code = `
import os
path = ${pyString(full)}
if not os.path.isfile(path):
    raise FileNotFoundError(path)
task = unreal.AssetImportTask()
task.set_editor_property("filename", path)
task.set_editor_property("destination_path", ${pyString(dest)})
task.set_editor_property("automated", True)
task.set_editor_property("save", True)
task.set_editor_property("replace_existing", ${replace === false ? "False" : "True"})
unreal.AssetToolsHelpers.get_asset_tools().import_asset_tasks([task])
result = [str(p) for p in task.get_editor_property("imported_object_paths")]
`;
      const r = await send("execute_python", { code }, 180000);
      if (r.stderr) throw new Error(`Import refusé par UEFN :\n${r.stderr}`);
      const paths = Array.isArray(r.result) ? r.result : [];
      if (!paths.length) {
        return "UEFN n'a rien importé (format refusé ?). Si c'est un .glb, convertis-le en .fbx avec Blender puis réessaie.";
      }
      return `Importé :\n${paths.join("\n")}\nPour le poser : spawn_actor avec asset_path = le chemin sans le suffixe « .Nom » (ex. /Projet/Lumen/SM_Autel).`;
    },
  },
  {
    name: "get_verse_fields",
    description:
      "Liste les champs @editable d'un device Verse posé dans le niveau (par son label), avec le device relié à chacun. Les champs vers un device ont un « type » ; device = null s'il n'est pas encore relié.",
    schema: { device: { type: "string", description: "Label du device Verse dans le niveau (ex. « Boxfight-boxfight round device »)." } },
    required: ["device"],
    safe: true,
    run: async ({ device }) => {
      const code = `${VERSE_FIELDS_PY}
dev = _lumen_actor(${pyString(device)})
script, fields = _lumen_verse_fields(dev)
result = {name: _lumen_describe(value) for name, (mangled, value) in fields.items()}
try:
    for name in _lumen_empty_lists(dev, fields):
        result[name] = {"list": []}  # nom renvoyé par Epic (initiale en minuscule)
except Exception as e:
    print("Listes vides non lisibles :", e)
`;
      const r = await send("execute_python", { code }, 60000);
      if (r.stderr) throw new Error(r.stderr);
      return json(r.result);
    },
  },
  {
    name: "link_verse_field",
    description:
      "Relie un champ @editable d'un device Verse à des devices du niveau, comme dans le panneau Détails : un champ simple (Team1 → BF_TeamSettings_Team1, Hud → un autre device Verse) ou une liste, même vide (Teams → [Team1, Team2], qui remplace toute la liste). Annulable par Ctrl+Z. Lance save_current_level après la série : UEFN n'enregistre qu'à la sauvegarde.",
    schema: {
      device: { type: "string", description: "Label du device Verse à régler." },
      field: { type: "string", description: "Nom du champ tel qu'écrit dans le Verse (ex. Team1, Teams)." },
      target: { type: "string", description: "Champ simple : label du device à relier (ex. BF_TeamSettings_Team1)." },
      targets: { type: "array", items: { type: "string" }, description: "Liste : labels de tous les devices, dans l'ordre. Remplace le contenu actuel." },
    },
    required: ["device", "field"],
    run: async ({ device, field, target, targets }) => {
      const isList = Array.isArray(targets);
      if (!isList && !target) throw new Error("Donne target (champ simple) ou targets (liste).");
      const wanted = isList ? targets : [target];
      const code = `${VERSE_FIELDS_PY}
dev = _lumen_actor(${pyString(device)})
targets = [_lumen_actor(t) for t in ${JSON.stringify(wanted)}]
name = ${pyString(field)}
script, fields = _lumen_verse_fields(dev)
name = next((k for k in fields if k.lower() == name.lower()), name)
want_list = ${isList ? "True" : "False"}
empty_list = name not in fields and want_list
if name not in fields and not want_list:
    raise ValueError(f"Champ introuvable. Champs connus : {sorted(fields)}.")
if not empty_list:
    mangled, current = fields[name]
    is_list = _lumen_is_list(current)
    if is_list != want_list:
        raise ValueError(f"« {name} » est " + ("une liste : passe targets." if is_list else "un champ simple : passe target."))
with unreal.ScopedEditorTransaction("Lumen : " + name):
    dev.modify()
    script.modify()
    if empty_list:
        _lumen_fill_empty_list(dev, script, name, targets)  # nom interne inconnu tant qu'elle est vide
    elif is_list:
        old = list(current)
        reuse = _lumen_wrapper_target(old[0])[0]
        values = []
        for i, t in enumerate(targets):
            if reuse and i < len(old):
                old[i].modify()
                old[i].set_editor_property("SavedActor", t)  # garde l'élément existant
                values.append(old[i])
            else:
                values.append(_lumen_link_value(script, old[0], t))
        script.set_editor_property(mangled, values)
    elif _lumen_wrapper_target(current)[0]:
        current.modify()
        current.set_editor_property("SavedActor", targets[0])
    else:
        script.set_editor_property(mangled, _lumen_link_value(script, current, targets[0]))
script, fields = _lumen_verse_fields(dev)
name = next((k for k in fields if k.lower() == name.lower()), name)
result =_lumen_describe(fields[name][1]) if name in fields else None
`;
      const r = await send("execute_python", { code }, 60000);
      if (r.stderr) throw new Error(r.stderr);
      const got = r.result?.list ? r.result.list.map((v) => v.device) : [r.result?.device];
      if (JSON.stringify(got) !== JSON.stringify(wanted)) throw new Error(`Le lien n'a pas pris (champ = ${json(r.result)}).`);
      return `${device}.${field} → ${isList ? `[${wanted.join(", ")}]` : target}. Pense à save_current_level quand la série est finie.`;
    },
  },
  {
    name: "get_all_actors",
    description: "Liste les acteurs du niveau. UEFN préfixe souvent les classes (FortStaticMeshActor, Device_*…).",
    schema: { class_filter: { type: "string", description: "Filtre optionnel sur le nom de classe." } },
    safe: true,
    run: async ({ class_filter = "" }) => json(await send("get_all_actors", { class_filter })),
  },
  {
    name: "get_selected_actors",
    description: "Acteurs sélectionnés dans la vue UEFN.",
    schema: {},
    safe: true,
    run: async () => json(await send("get_selected_actors")),
  },
  {
    name: "spawn_actor",
    description: "Pose un acteur dans le niveau, depuis un asset (asset_path) ou une classe (actor_class). Unités : centimètres.",
    schema: {
      asset_path: { type: "string", description: "Asset à poser, ex. /magnet_incremental/Lumen/SM_Autel." },
      actor_class: { type: "string", description: "Ou une classe Unreal, ex. PointLight." },
      location: vec3("[x, y, z] en cm."),
      rotation: vec3("[pitch, yaw, roll] en degrés."),
    },
    run: async (a) => json(await send("spawn_actor", a)),
  },
  {
    name: "delete_actors",
    description: "Supprime des acteurs (chemins ou labels). Ne supprime que ce que tu as posé, ou ce que l'utilisateur a demandé.",
    schema: { actor_paths: { type: "array", items: { type: "string" } } },
    required: ["actor_paths"],
    run: async (a) => json(await send("delete_actors", a)),
  },
  {
    name: "set_actor_transform",
    description: "Change la position, la rotation et/ou l'échelle d'un acteur.",
    schema: {
      actor_path: { type: "string", description: "Chemin ou label de l'acteur." },
      location: vec3("[x, y, z] en cm."),
      rotation: vec3("[pitch, yaw, roll] en degrés."),
      scale: vec3("[x, y, z]."),
    },
    required: ["actor_path"],
    run: async (a) => json(await send("set_actor_transform", a)),
  },
  {
    name: "get_actor_properties",
    description: "Lit des propriétés d'un acteur. Celles qui n'existent pas renvoient une erreur, une par une.",
    schema: { actor_path: { type: "string" }, properties: { type: "array", items: { type: "string" } } },
    required: ["actor_path", "properties"],
    safe: true,
    run: async (a) => json(await send("get_actor_properties", a)),
  },
  {
    name: "set_actor_properties",
    description: "Modifie des propriétés d'un acteur (set_editor_property). Chaque propriété renvoie ok ou une erreur.",
    schema: { actor_path: { type: "string" }, properties: { type: "object" } },
    required: ["actor_path", "properties"],
    run: async (a) => json(await send("set_actor_properties", a)),
  },
  {
    name: "select_actors",
    description: "Sélectionne des acteurs dans la vue (pour les montrer à l'utilisateur).",
    schema: { actor_paths: { type: "array", items: { type: "string" } }, add_to_selection: { type: "boolean" } },
    required: ["actor_paths"],
    safe: true,
    run: async (a) => json(await send("select_actors", a)),
  },
  {
    name: "focus_selected",
    description: "Cadre la caméra sur la sélection (comme la touche F).",
    schema: {},
    safe: true,
    run: async () => json(await send("focus_selected")),
  },
  {
    name: "get_editor_log",
    description: "Dernières lignes du Journal de sortie de UEFN.",
    schema: { last_n: { type: "integer" }, filter_str: { type: "string" } },
    safe: true,
    run: async ({ last_n = 100, filter_str = "" }) => {
      const r = await send("get_editor_log", { last_n, filter_str });
      return r.error ? `Erreur : ${r.error}` : (r.lines || []).join("\n");
    },
  },
  {
    name: "list_assets",
    description: "Liste les assets d'un dossier du contenu (ex. /magnet_incremental/Meshes/).",
    schema: { directory: { type: "string" }, recursive: { type: "boolean" }, class_filter: { type: "string" } },
    safe: true,
    run: async ({ directory = CONTENT_MODULE ? `/${CONTENT_MODULE}/` : "/Game/", recursive = true, class_filter = "" }) =>
      json(await send("list_assets", { directory, recursive, class_filter })),
  },
  {
    name: "search_assets",
    description: "Cherche des assets par classe (StaticMesh, Texture2D, Material…).",
    schema: { class_name: { type: "string" }, directory: { type: "string" }, recursive: { type: "boolean" } },
    safe: true,
    run: async ({ class_name = "", directory = CONTENT_MODULE ? `/${CONTENT_MODULE}/` : "/Game/", recursive = true }) =>
      json(await send("search_assets", { class_name, directory, recursive })),
  },
  {
    name: "get_asset_info",
    description: "Détails d'un asset.",
    schema: { asset_path: { type: "string" } },
    required: ["asset_path"],
    safe: true,
    run: async (a) => json(await send("get_asset_info", a)),
  },
  {
    name: "does_asset_exist",
    description: "Vérifie qu'un asset existe.",
    schema: { asset_path: { type: "string" } },
    required: ["asset_path"],
    safe: true,
    run: async (a) => json(await send("does_asset_exist", a)),
  },
  {
    name: "rename_asset",
    description: "Renomme ou déplace un asset.",
    schema: { old_path: { type: "string" }, new_path: { type: "string" } },
    required: ["old_path", "new_path"],
    run: async (a) => json(await send("rename_asset", a)),
  },
  {
    name: "duplicate_asset",
    description: "Duplique un asset.",
    schema: { source_path: { type: "string" }, dest_path: { type: "string" } },
    required: ["source_path", "dest_path"],
    run: async (a) => json(await send("duplicate_asset", a)),
  },
  {
    name: "delete_asset",
    description: "Supprime un asset. Seulement s'il vient de toi ou si l'utilisateur l'a demandé.",
    schema: { asset_path: { type: "string" } },
    required: ["asset_path"],
    run: async (a) => json(await send("delete_asset", a)),
  },
  {
    name: "save_asset",
    description: "Sauvegarde un asset modifié.",
    schema: { asset_path: { type: "string" } },
    required: ["asset_path"],
    run: async (a) => json(await send("save_asset", a)),
  },
  {
    name: "save_current_level",
    description: "Sauvegarde le niveau. À faire après une série de modifications réussies.",
    schema: {},
    run: async () => json(await send("save_current_level")),
  },
  {
    name: "get_level_info",
    description: "Nom du niveau et nombre d'acteurs.",
    schema: {},
    safe: true,
    run: async () => json(await send("get_level_info")),
  },
  {
    name: "get_viewport_camera",
    description: "Position et rotation de la caméra de la vue.",
    schema: {},
    safe: true,
    run: async () => json(await send("get_viewport_camera")),
  },
  {
    name: "set_viewport_camera",
    description: "Déplace la caméra de la vue.",
    schema: { location: vec3("[x, y, z] en cm."), rotation: vec3("[pitch, yaw, roll] en degrés.") },
    safe: true,
    run: async (a) => json(await send("set_viewport_camera", a)),
  },
];

const BY_NAME = new Map(TOOLS.map((t) => [t.name, t]));

function listTools() {
  return TOOLS.map((t) => ({
    name: t.name,
    description: t.description,
    inputSchema: { type: "object", properties: t.schema, ...(t.required ? { required: t.required } : {}) },
  }));
}

function reply(id, result) {
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, result }) + "\n");
}

function replyError(id, code, message) {
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, error: { code, message } }) + "\n");
}

async function handle(message) {
  const { id, method, params } = message;
  if (id === undefined || id === null) return; // notification
  if (method === "initialize") {
    reply(id, {
      protocolVersion: params?.protocolVersion || "2025-06-18",
      capabilities: { tools: {} },
      serverInfo: { name: "lumen-uefn", version: "1.0.0" },
      instructions:
        "Pilote l'éditeur UEFN ouvert : acteurs, assets, niveau, caméra, Python. Le projet ouvert dans UEFN doit être celui de l'agent. Sauvegarde le niveau après une série de modifications réussies.",
    });
    return;
  }
  if (method === "ping") return reply(id, {});
  if (method === "tools/list") return reply(id, { tools: listTools() });
  if (method === "tools/call") {
    const tool = BY_NAME.get(params?.name);
    if (!tool) return replyError(id, -32602, `Outil inconnu : ${params?.name}`);
    try {
      if (!tool.safe) await assertProject();
      const text = await tool.run(params?.arguments || {});
      reply(id, { content: [{ type: "text", text }] });
    } catch (error) {
      reply(id, { content: [{ type: "text", text: String(error.message || error) }], isError: true });
    }
    return;
  }
  replyError(id, -32601, `Méthode non prise en charge : ${method}`);
}

let buffer = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => {
  buffer += chunk;
  let nl;
  while ((nl = buffer.indexOf("\n")) >= 0) {
    const line = buffer.slice(0, nl).trim();
    buffer = buffer.slice(nl + 1);
    if (!line) continue;
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      replyError(null, -32700, "JSON illisible");
      continue;
    }
    handle(message).catch((error) => replyError(message.id ?? null, -32603, String(error)));
  }
});
process.stdin.on("end", () => process.exit(0));
