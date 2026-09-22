#!/usr/bin/env node
/**
 * Pont Lumen : génère une image (Gemini) ou un mesh (Meshy/Tripo)
 * via Lumen. Les clés API restent dans Lumen, jamais dans le projet.
 *
 * Usage:
 *   node tools/lumen-asset.mjs status
 *   node tools/lumen-asset.mjs image "icône pièce d'or, style Roblox, PNG fond transparent"
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

const BASE = process.env.LUMEN_ASSET_URL || "http://127.0.0.1:17422";

function fail(message) {
  console.error(message);
  process.exit(1);
}

function usage() {
  fail(
    "Usage:\n  node tools/lumen-asset.mjs status\n  node tools/lumen-asset.mjs search [mesh|image|inspiration|texture] <mots> --for \"à quoi ça sert sur la map\"\n  node tools/lumen-asset.mjs propose VS-0001 VS-0002 --for \"décor au spawn\"\n  node tools/lumen-asset.mjs image <prompt>\n  node tools/lumen-asset.mjs mesh <prompt>\n  node tools/lumen-asset.mjs blender assets/blender/nom.py Titre\n  node tools/lumen-asset.mjs publish LUM-0001\n  node tools/lumen-asset.mjs get LUM-0001\n  node tools/lumen-asset.mjs get TEX-0001\n  node tools/lumen-asset.mjs get INS-0001",
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
      if (json.code && !json.robloxAssetId) {
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
      if (!json.robloxAssetId) {
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
    if (json.code && !json.robloxAssetId) {
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
    if (!json.robloxAssetId) {
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
    if (json.code && !json.robloxAssetId) {
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
    if (!json.robloxAssetId) {
      fail(
        json.publishError ||
          "Asset créé dans Lumen mais pas publié sur Roblox. Ajoute une clé Open Cloud (asset:read + asset:write) dans Réglages.",
      );
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
