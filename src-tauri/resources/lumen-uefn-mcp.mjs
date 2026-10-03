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
