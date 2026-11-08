// Actions dans la place ouverte dans Roblox Studio, faites par le plugin Lumen (mode édition, Ctrl+Z possible).
//
//   node tools/lumen-studio.mjs drop-pack
//   node tools/lumen-studio.mjs insert <assetId> [--name Nom] [--parent Workspace] [--replace]
//        [--align Modele --marker LevelCenter --offset 0,-0.25,0] [--at x,y,z] [--neon Glow,Eyes] [--no-anchor]
//   node tools/lumen-studio.mjs build assets/maps/Niveau.json [--name Nom] [--parent Workspace] [--replace] [--at x,y,z]
//
// insert : modèle publié (rbxassetid) ajouté à la place, sauvegardé avec elle. --align cale son repère
// (--marker, LevelCenter par défaut, rendu invisible) sur le même repère d'un modèle déjà présent.
// build : JSON { "root": { "class": "Model", "name": "...", "primary": "LevelCenter", "children": [
//   { "class": "Part", "name": "...", "size": [x,y,z], "pos": [x,y,z], "rot": [rx,ry,rz], "color": "#RRGGBB",
//     "material": "SmoothPlastic", "anchored": true, "canCollide": true, "transparency": 0, "castShadow": true } ] } }
import { existsSync, mkdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const [cmd, ...args] = process.argv.slice(2);
const root = process.env.LUMEN_PROJECT || process.cwd();

function usage(code = 1) {
  console.log(
    "Usage :\n  node tools/lumen-studio.mjs drop-pack\n  node tools/lumen-studio.mjs insert <assetId> [--name Nom] [--replace] [--align Modele] [--marker LevelCenter] [--offset x,y,z] [--at x,y,z] [--neon Glow,Eyes]\n  node tools/lumen-studio.mjs build assets/maps/Niveau.json [--name Nom] [--replace] [--at x,y,z]",
  );
  process.exit(code);
}

if (!cmd) usage(0);

if (cmd === "drop-pack") {
  const dir = join(root, "assets", "ui");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "drop-pack"), "1\n");
  console.log(
    "Demande envoyée. Avec Lumen ouvert et Studio rouvert, le plugin retire le pack de la place. L'UI du jeu dans src/client reste.",
  );
  process.exit(0);
}

const flags = {};
const positional = [];
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a.startsWith("--")) {
    const key = a.slice(2);
    const next = args[i + 1];
    if (next === undefined || next.startsWith("--")) flags[key] = true;
    else {
      flags[key] = next;
      i++;
    }
  } else positional.push(a);
}
const triple = (s) => {
  const v = String(s).split(",").map(Number);
  if (v.length !== 3 || v.some(Number.isNaN)) usage();
  return v;
};

const job = { parent: flags.parent || "Workspace", replace: Boolean(flags.replace) };
if (typeof flags.name === "string") job.name = flags.name;
if (flags.at) job.at = triple(flags.at);

if (cmd === "insert") {
  const id = positional[0]?.replace(/^rbxassetid:\/\//, "");
  if (!id || !/^\d+$/.test(id)) usage();
  Object.assign(job, { kind: "insert", assetId: id, anchor: !flags["no-anchor"] });
  if (flags.neon) job.neon = String(flags.neon).split(",").map((s) => s.trim()).filter(Boolean);
  if (flags.align) {
    job.marker = typeof flags.marker === "string" ? flags.marker : "LevelCenter";
    job.alignTo = {
      model: flags.align.includes(".") ? flags.align : `Workspace.${flags.align}`,
      marker: job.marker,
      offset: flags.offset ? triple(flags.offset) : [0, 0, 0],
    };
  } else if (typeof flags.marker === "string") job.marker = flags.marker;
} else if (cmd === "build") {
  const file = positional[0];
  if (!file) usage();
  const spec = JSON.parse(readFileSync(resolve(root, file), "utf8"));
  Object.assign(job, { kind: "build", root: spec.root || spec });
} else usage();

const dir = join(root, "assets", "studio-jobs");
mkdirSync(dir, { recursive: true });
const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
const jobFile = join(dir, `${id}.json`);
const resultFile = join(dir, `${id}.result.json`);
writeFileSync(jobFile, JSON.stringify(job));
console.error("En attente du plugin Lumen dans Studio (place ouverte en mode édition, Lumen ouvert)…");

const deadline = Date.now() + 180_000;
while (Date.now() < deadline) {
  if (existsSync(resultFile)) {
    const r = JSON.parse(readFileSync(resultFile, "utf8"));
    unlinkSync(resultFile);
    if (r.ok) {
      console.log(`Fait dans Studio : ${r.name} (${r.descendants} objets). Annulable par Ctrl+Z ; sauvegardé avec la place.`);
      process.exit(0);
    }
    console.error(`Le plugin a refusé : ${r.message}`);
    process.exit(1);
  }
  await new Promise((r) => setTimeout(r, 500));
}
try {
  unlinkSync(jobFile);
} catch {
  /* déjà pris */
}
console.error(
  "Pas de réponse de Studio en 3 minutes. Vérifie que Studio est ouvert sur la place liée au projet, en mode édition (pas en Play), avec le plugin Lumen, et que ce projet est ouvert dans Lumen.",
);
process.exit(1);
