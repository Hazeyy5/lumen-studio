#!/usr/bin/env node
/**
 * Pont Lumen : génère une image (Gemini) ou un mesh (Meshy/Tripo)
 * via Lumen. Les clés API restent dans Lumen, jamais dans le projet.
 *
 * Usage:
 *   node tools/lumen-asset.mjs status
 *   node tools/lumen-asset.mjs image "icône pièce d'or, style Roblox, PNG fond transparent"
 *   node tools/lumen-asset.mjs icon VS-0001
 *   node tools/lumen-asset.mjs icon assets/meshes/coffre.glb
 *   node tools/lumen-asset.mjs mesh "coffre low poly pour tycoon Roblox"
 *   node tools/lumen-asset.mjs publish LUM-0001
 *   node tools/lumen-asset.mjs search palm
 *   node tools/lumen-asset.mjs search mesh totem
 *   node tools/lumen-asset.mjs search inspiration hud
 *   node tools/lumen-asset.mjs search texture bouton
 *   node tools/lumen-asset.mjs get VS-0001
 *   node tools/lumen-asset.mjs get TEX-0001
 *   node tools/lumen-asset.mjs get INS-0001
 */

import { readdirSync } from "node:fs";

const BASE = process.env.LUMEN_ASSET_URL || "http://127.0.0.1:17422";

// Projet Fortnite ou Unreal : pas de publication Roblox, le fichier reste dans assets/ pour l'import dans l'éditeur.
const ENGINE = (() => {
  try {
    const files = readdirSync(process.env.LUMEN_PROJECT || process.cwd()).map((f) => f.toLowerCase());
    if (files.some((f) => f.endsWith(".uefnproject"))) return "uefn";
    if (files.some((f) => f.endsWith(".uproject"))) return "unreal";
  } catch {
    /* Roblox */
  }
  return "";
})();
const UEFN = ENGINE !== "";

function relativeFile(path) {
  const root = (process.env.LUMEN_PROJECT || process.cwd()).replace(/\\/g, "/").replace(/\/$/, "");
  const file = String(path || "").replace(/\\/g, "/");
  return file.toLowerCase().startsWith(root.toLowerCase() + "/") ? file.slice(root.length + 1) : file;
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

function usage() {
  fail(
    "Usage:\n  node tools/lumen-asset.mjs status\n  node tools/lumen-asset.mjs search [mesh|image|inspiration|texture] <mots> --for \"à quoi ça sert sur la map\"\n  node tools/lumen-asset.mjs propose VS-0001 VS-0002 --for \"décor au spawn\"\n  node tools/lumen-asset.mjs image <prompt>\n  node tools/lumen-asset.mjs icon <VS-0001 | fichier.glb>\n  node tools/lumen-asset.mjs mesh <prompt>\n  node tools/lumen-asset.mjs blender assets/blender/nom.py Titre\n  node tools/lumen-asset.mjs draft assets/blender/nom.py [Nom] [--views front,back,left,right,threequarter,top] [--fbx]\n  node tools/lumen-asset.mjs upload assets/images/icone.png [Titre]\n  node tools/lumen-asset.mjs publish LUM-0001\n  node tools/lumen-asset.mjs get LUM-0001\n  node tools/lumen-asset.mjs get TEX-0001\n  node tools/lumen-asset.mjs get INS-0001",
  );
}

async function readJson(res) {
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    return { error: text || `HTTP ${res.status}` };
  }
}

async function request(path, options) {
  let res;
  try {
    res = await fetch(`${BASE}${path}`, options);
  } catch {
    fail("Lumen n'est pas joignable. Ouvre l'app Lumen, puis réessaie.");
  }
  const json = await readJson(res);
  if (!res.ok || json.error) {
    throw new Error(json.error || JSON.stringify(json));
  }
  return json;
}

async function call(path, options) {
  try {
    return await request(path, options);
  } catch (err) {
    fail(String(err.message || err));
  }
}

function printAsset(json) {
  if (json.code) console.log(`ID ${json.code}`);
  if (UEFN && !json.inspiration) {
    const file = json.relativePath || relativeFile(json.localPath || json.path);
    if (file) {
      if (ENGINE === "unreal") {
        console.log(`Fichier pour Unreal : ${file}`);
        console.log(
          "Importe-le toi-même avec le MCP de l'éditeur (import d'asset vers /Game/Lumen/...), éditeur ouvert. Sinon, demande à l'utilisateur de le glisser dans le Content Browser.",
        );
      } else {
        console.log(`Fichier pour UEFN : ${file}`);
        console.log(
          "Demande à l'utilisateur de le glisser dans le Content Browser de UEFN (dossier à préciser), puis Build Verse Code. Le nom Verse est dans le digest *-Assets.digest.verse.",
        );
      }
    }
    if (json.copyError) console.log(`Copie : ${json.copyError}`);
    console.log(JSON.stringify(json, null, 2));
    return;
  }
  if (json.inspiration || json.publishSkipped) {
    if (json.relativePath) console.log(`Inspiration copiée : ${json.relativePath}`);
    else if (json.localPath) console.log(`Inspiration copiée : ${json.localPath}`);
    if (json.copyError) console.log(`Copie : ${json.copyError}`);
    console.log(
      "Lis ce fichier image (Read) et reproduis l'esprit de l'UI (layout, couleurs, rythme). Ne le publie pas sur Roblox.",
    );
  } else if (json.robloxAssetId || json.assetId) {
    const id = String(json.robloxAssetId || json.assetId);
    console.log(/^rbxasset/i.test(id) ? id : `rbxassetid://${id}`);
  } else if (json.publishError) {
    console.log(`Pas encore sur Roblox : ${json.publishError}`);
  }
  if (json.scaleType) {
    console.log(`ScaleType Enum.ScaleType.${json.scaleType}`);
    if (json.scaleType === "Tile" && json.tileSize) {
      const t = json.tileSize;
      console.log(
        `TileSize UDim2.new(${t.xScale ?? 0}, ${t.xOffset ?? 0}, ${t.yScale ?? 0}, ${t.yOffset ?? 0})`,
      );
    }
  }
  console.log(JSON.stringify(json, null, 2));
}

function splitPurpose(parts) {
  const flags = new Set(["--for", "--pour", "--use", "--as", "--why", "--purpose"]);
  const kept = [];
  let purpose = "";
  for (let i = 0; i < parts.length; i++) {
    if (flags.has(String(parts[i]).toLowerCase())) {
      purpose = parts.slice(i + 1).join(" ").trim();
      break;
    }
    kept.push(parts[i]);
  }
  return { parts: kept, purpose };
}

async function main() {
  const [cmd, ...rest] = process.argv.slice(2);
  if (!cmd) usage();

  if (cmd === "status" || cmd === "health") {
    printAsset(await call("/status"));
    return;
  }

  if (cmd === "inspire" && rest.length === 1 && /^(ins|lum|vs)-/i.test(rest[0] || "")) {
    console.error("En attente de validation dans Lumen (notification / toast en haut à droite)…");
    const project = process.env.LUMEN_PROJECT || process.cwd();
    printAsset(
      await call(
        `/asset?code=${encodeURIComponent(rest[0])}&project=${encodeURIComponent(project)}`,
      ),
    );
    return;
  }

  if (cmd === "search" || cmd === "library" || cmd === "inspire") {
    const kinds = new Set([
      "mesh",
      "image",
      "icon",
      "icons",
      "all",
      "inspiration",
      "inspire",
      "texture",
      "textures",
      "tex",
    ]);
    let kind = cmd === "inspire" ? "inspiration" : "";
    const { parts: rawParts, purpose } = splitPurpose(rest);
    const parts = [...rawParts];
    if (parts[0] && kinds.has(parts[0].toLowerCase())) {
      kind = parts.shift().toLowerCase();
    }
    const q = parts.join(" ").trim();
    const params = new URLSearchParams();
    if (q) params.set("q", q);
    if (purpose) params.set("for", purpose);
    if (kind && kind !== "all") {
      params.set(
        "kind",
        kind === "icon" || kind === "icons"
          ? "image"
          : kind === "inspire"
            ? "inspiration"
            : kind === "textures" || kind === "tex"
              ? "texture"
              : kind,
      );
    }
    params.set("limit", "10");
    if (!purpose) {
      console.error(
        'Ajoute --for "où ça va / à quoi ça sert" : Lumen l’affiche sur le toast.',
      );
    }
    console.error("Menu d’assets dans Lumen — choisis-en un (toast haut à droite)…");
    const json = await call(`/library?${params.toString()}`);
    const items = json.items || [];
    if (!items.length) {
      console.log("Aucun asset. Élargis les mots-clés ou génère avec `image` / `mesh`.");
      console.log(JSON.stringify(json, null, 2));
      return;
    }
    for (const item of items) {
      const label =
        item.inspiration || item.source === "inspiration"
          ? "inspiration"
          : item.source === "texture" || item.kind === "texture"
            ? "texture"
            : item.kind;
      console.log(`${item.code}\t${label}\t${item.name}`);
    }
    if (json.chosen) {
      console.log(`Choisi : ${json.chosen}`);
      console.log(`Ensuite uniquement : get ${json.chosen}`);
    } else {
      console.log("Aucune sélection. Ne fais pas de get.");
    }
    console.log(JSON.stringify(json, null, 2));
    return;
  }

  if (cmd === "propose" || cmd === "show") {
    const { parts: rawParts, purpose } = splitPurpose(rest);
    const codes = rawParts
      .join(" ")
      .split(/[\s,;]+/)
      .map((s) => s.trim())
      .filter((s) => /^(vs|lum|ins|tex)-/i.test(s))
      .slice(0, 10);
    if (!codes.length) usage();
    console.error("Menu d’assets dans Lumen — choisis-en un (toast haut à droite)…");
    const json = await call("/propose", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        codes,
        query: purpose || "proposition",
        purpose,
      }),
    });
    for (const item of json.items || []) {
      console.log(`${item.code}\t${item.kind}\t${item.name}`);
    }
    if (json.chosen) {
      console.log(`Choisi : ${json.chosen}`);
      console.log(`Ensuite uniquement : get ${json.chosen}`);
    } else {
      console.log("Aucune sélection. Ne fais pas de get.");
    }
    return;
  }

  if (cmd === "icon" || cmd === "icone" || cmd === "icône") {
    const flags = new Set([
      "--zoom",
      "--vertical",
      "--horizontal",
      "--thickness",
      "--color",
      "--resolution",
      "--opacity",
      "--blur",
      "--offset-y",
    ]);
    const modelParts = [];
    const options = {
      zoom: 1,
      vertical: 0,
      horizontal: 0,
      thickness: 4,
      color: "#111111",
      resolution: 512,
      opacity: 0.35,
      blur: 16,
      offsetY: 12,
      outline: true,
      shadow: true,
    };
    for (let i = 0; i < rest.length; i += 1) {
      const token = rest[i];
      const lower = String(token).toLowerCase();
      if (lower === "--no-outline") {
        options.outline = false;
        continue;
      }
      if (lower === "--no-shadow") {
        options.shadow = false;
        continue;
      }
      if (flags.has(lower)) {
        const value = rest[i + 1];
        i += 1;
        if (value == null) usage();
        if (lower === "--color") options.color = value;
        else if (lower === "--resolution") options.resolution = Number(value);
        else if (lower === "--offset-y") options.offsetY = Number(value);
        else options[lower.slice(2)] = Number(value);
        continue;
      }
      modelParts.push(token);
    }
    const model = modelParts.join(" ").trim();
    if (!model) usage();
    console.error("Capture de l’icône dans Lumen — laisse la fenêtre ouverte…");
    const json = await call("/icon", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        projectPath: process.env.LUMEN_PROJECT || process.cwd(),
        model,
        ...options,
      }),
    });
    printAsset(json);
    if (!UEFN && !json.robloxAssetId) {
      fail(
        json.publishError ||
          "Icône créée dans Lumen mais pas publiée sur Roblox. Ajoute une clé Open Cloud (asset:read + asset:write) dans Réglages.",
      );
    }
    return;
  }

  if (cmd === "image" || cmd === "mesh") {
    const prompt = rest.join(" ").trim();
    if (!prompt) usage();
    if (cmd === "mesh" && /\.py$/i.test(rest[0] || "")) {
      const scriptPath = rest[0];
      const title = rest.slice(1).join(" ").trim() || "mesh";
      console.error("Blender dans Lumen — toast en haut à droite…");
      const json = await call("/blender", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          projectPath: process.cwd(),
          scriptPath,
          title,
        }),
      });
      if (!UEFN && json.code && !json.robloxAssetId) {
        try {
          const published = await request("/publish", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ code: json.code }),
          });
          json.robloxAssetId = published.assetId;
          json.publishError = undefined;
        } catch (err) {
          json.publishError = String(err.message || err);
        }
      }
      printAsset(json);
      if (!UEFN && !json.robloxAssetId) {
        fail(
          json.publishError ||
            "Asset créé dans Lumen mais pas publié sur Roblox. Ajoute une clé Open Cloud (asset:read + asset:write) dans Réglages.",
        );
      }
      return;
    }
    const json = await call(`/${cmd}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        prompt,
        projectPath: process.cwd(),
      }),
    });
    if (!UEFN && json.code && !json.robloxAssetId) {
      try {
        const published = await request("/publish", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ code: json.code }),
        });
        json.robloxAssetId = published.assetId;
        json.publishError = undefined;
      } catch (err) {
        json.publishError = String(err.message || err);
      }
    }
    printAsset(json);
    if (!UEFN && !json.robloxAssetId) {
      fail(
        json.publishError ||
          "Asset créé dans Lumen mais pas publié sur Roblox. Ajoute une clé Open Cloud (asset:read + asset:write) dans Réglages.",
      );
    }
    return;
  }

  if (cmd === "blender") {
    const scriptPath = rest[0];
    const title = rest.slice(1).join(" ").trim() || "mesh";
    if (!scriptPath) usage();
    console.error("Blender dans Lumen — toast en haut à droite…");
    const json = await call("/blender", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        projectPath: process.cwd(),
        scriptPath,
        title,
      }),
    });
    if (!UEFN && json.code && !json.robloxAssetId) {
      try {
        const published = await request("/publish", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ code: json.code }),
        });
        json.robloxAssetId = published.assetId;
        json.publishError = undefined;
      } catch (err) {
        json.publishError = String(err.message || err);
      }
    }
    printAsset(json);
    if (!UEFN && !json.robloxAssetId) {
      fail(
        json.publishError ||
          "Asset créé dans Lumen mais pas publié sur Roblox. Ajoute une clé Open Cloud (asset:read + asset:write) dans Réglages.",
      );
    }
    return;
  }

  if (cmd === "draft") {
    // draft assets/blender/nom.py [Nom] [--views front,top] [--fbx]
    const args = [...rest];
    const take = (flag) => {
      const i = args.indexOf(flag);
      if (i < 0) return undefined;
      const value = args[i + 1];
      args.splice(i, 2);
      return value;
    };
    const views = take("--views") || "";
    const fbxIndex = args.indexOf("--fbx");
    const fbx = fbxIndex >= 0 || (UEFN && args.indexOf("--glb") < 0);
    if (fbxIndex >= 0) args.splice(fbxIndex, 1);
    const glbIndex = args.indexOf("--glb");
    if (glbIndex >= 0) args.splice(glbIndex, 1);
    const scriptPath = args[0];
    if (!scriptPath) usage();
    const name = args.slice(1).join(" ").trim();
    console.error("Blender en arrière-plan (brouillon, rien n'est publié)…");
    const r = await call("/blender/draft", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        projectPath: process.cwd(),
        scriptPath,
        name,
        views,
        format: fbx ? "fbx" : "glb",
        srgb: !UEFN, // Roblox affiche les couleurs de sommets en sRGB
      }),
    });
    const size = r.bbox_blender?.size_xyz || [];
    console.log(`Fichier : ${relativeFile(r.file)}`);
    console.log(`Triangles : ${r.triangles} sur ${r.meshes} meshes. Taille X×Y×Z (Blender, Z en haut) : ${size.join(" × ")}`);
    for (const o of r.objects || []) {
      if (/_Outline$/.test(o.name)) continue;
      console.log(`  ${o.name}${o.parent ? ` (enfant de ${o.parent})` : ""} : ${o.tris} tris, pivot ${o.pivot.join(", ")}${o.colors ? "" : ", SANS couleur"}`);
    }
    const outlines = (r.objects || []).filter((o) => /_Outline$/.test(o.name)).length;
    if (outlines) console.log(`  + ${outlines} contours (_Outline)`);
    for (const e of r.empties || []) console.log(`  repère ${e.name} : ${e.pos.join(", ")}`);
    for (const a of r.actions || []) console.log(`  animation ${a.name} : images ${a.frames.join("-")}`);
    for (const w of r.warnings || []) console.log(`ATTENTION : ${w}`);
    console.log("Rendus (lis-les avec Read et compare à la planche de concept) :");
    for (const p of r.renders || []) console.log(`  ${relativeFile(p)}`);
    return;
  }

  if (cmd === "upload") {
    const file = rest[0];
    const title = rest.slice(1).join(" ").trim();
    if (!file) usage();
    if (UEFN) fail("Projet UEFN/Unreal : rien à publier sur Roblox. Importe le fichier dans l'éditeur (import_file).");
    const json = await call("/upload", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ projectPath: process.cwd(), file, title }),
    });
    printAsset(json);
    if (!json.robloxAssetId) {
      fail(json.publishError || "Fichier ajouté à la banque mais pas publié sur Roblox. Ajoute une clé Open Cloud (asset:read + asset:write) dans Réglages.");
    }
    return;
  }

  if (cmd === "publish" || cmd === "get") {
    const code = rest.join(" ").trim();
    if (!code) usage();
    if (cmd === "get") {
      console.error("En attente de validation dans Lumen (notification / toast en haut à droite)…");
      const project = process.env.LUMEN_PROJECT || process.cwd();
      printAsset(
        await call(
          `/asset?code=${encodeURIComponent(code)}&project=${encodeURIComponent(project)}`,
        ),
      );
      return;
    }
    if (UEFN) fail("Projet UEFN/Unreal : rien à publier sur Roblox. Utilise `get CODE` pour récupérer le fichier.");
    printAsset(
      await call("/publish", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code }),
      }),
    );
    return;
  }

  usage();
}

main().catch((err) => fail(String(err)));
