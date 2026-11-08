// Bruitages Lumen : assets/sfx/<Nom>.sfx.json (+ <Nom>.wav), écoutables dans l'onglet Sons de Lumen.
//
//   node tools/lumen-sfx.mjs presets                         liste des préréglages
//   node tools/lumen-sfx.mjs make <preset> <Nom> [--seed 7] [--pitch 1.2] [--length 0.8]
//   node tools/lumen-sfx.mjs variants <preset> <Nom> [4]      Nom_v1..v4 à écouter dans Lumen
//   node tools/lumen-sfx.mjs keep <Nom_v3> <Nom>             garde une variante, supprime les autres
//   node tools/lumen-sfx.mjs render [Nom]                    JSON modifié à la main -> WAV
//   node tools/lumen-sfx.mjs upload [Nom]                    publie les WAV nouveaux ou modifiés -> ids.json
//   node tools/lumen-sfx.mjs sync                            écrit src/shared/sfx/index.ts (SfxIds, playSfx)
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { encodeWav, makePreset, PRESETS, renderSfx } from "./lumen-sfx-core.mjs";

const root = process.env.LUMEN_PROJECT || join(dirname(fileURLToPath(import.meta.url)), "..");
const dir = join(root, "assets", "sfx");
const idsFile = join(dir, "ids.json");
const outDir = join(root, "src", "shared", "sfx");
mkdirSync(dir, { recursive: true });

const args = process.argv.slice(2);
const cmd = args[0];
const flag = (name, def) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] !== undefined ? Number(args[i + 1]) : def;
};
const positional = args.filter((a, i) => !a.startsWith("--") && !(i > 0 && args[i - 1].startsWith("--")));
const safe = (n) => {
  if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(n || "")) {
    console.error(`Nom invalide « ${n} » : lettres, chiffres et _ (commence par une lettre), ex. CoinPickup.`);
    process.exit(1);
  }
  return n;
};
const readIds = () => (existsSync(idsFile) ? JSON.parse(readFileSync(idsFile, "utf8")) : {});
const names = () => readdirSync(dir).filter((f) => f.endsWith(".sfx.json")).map((f) => f.replace(/\.sfx\.json$/, "")).sort();
const hash = (buf) => createHash("sha256").update(buf).digest("hex").slice(0, 16);

function save(name, spec) {
  spec.name = name;
  writeFileSync(join(dir, `${name}.sfx.json`), JSON.stringify(spec, null, 2));
  const wav = encodeWav(renderSfx(spec));
  writeFileSync(join(dir, `${name}.wav`), wav);
  const seconds = (wav.length - 44) / 2 / 44100;
  console.log(`${name} : assets/sfx/${name}.wav (${seconds.toFixed(2)} s)`);
}

if (!cmd || cmd === "help") {
  console.log(readFileSync(fileURLToPath(import.meta.url), "utf8").split("\n").filter((l) => l.startsWith("//")).join("\n"));
  process.exit(0);
}

if (cmd === "presets") {
  for (const [k, p] of Object.entries(PRESETS)) console.log(`${k.padEnd(10)} ${p.label}`);
  process.exit(0);
}

if (cmd === "make" || cmd === "variants") {
  const [, preset, rawName, countArg] = positional;
  const name = safe(rawName);
  if (!PRESETS[preset]) {
    console.error(`Préréglage inconnu « ${preset} ». Liste : node tools/lumen-sfx.mjs presets`);
    process.exit(1);
  }
  const pitch = flag("pitch", 1);
  const length = flag("length", 1);
  if (cmd === "make") {
    save(name, makePreset(preset, { seed: flag("seed", Math.floor(Math.random() * 1e6)), pitch, length }));
  } else {
    const n = Math.max(2, Math.min(8, Number(countArg) || 4));
    const base = Math.floor(Math.random() * 1e6);
    for (let i = 1; i <= n; i++) save(`${name}_v${i}`, makePreset(preset, { seed: base + i, pitch, length }));
    console.log(`Fais écouter les variantes à l'utilisateur dans l'onglet Sons de Lumen, puis : node tools/lumen-sfx.mjs keep ${name}_vN ${name}`);
  }
  process.exit(0);
}

if (cmd === "keep") {
  const [, from, rawTo] = positional;
  const to = safe(rawTo);
  if (!existsSync(join(dir, `${from}.sfx.json`))) {
    console.error(`Introuvable : assets/sfx/${from}.sfx.json`);
    process.exit(1);
  }
  const spec = JSON.parse(readFileSync(join(dir, `${from}.sfx.json`), "utf8"));
  save(to, spec);
  const prefix = from.replace(/_v\d+$/, "_v");
  for (const n of names()) {
    if (n.startsWith(prefix) && /_v\d+$/.test(n)) {
      rmSync(join(dir, `${n}.sfx.json`), { force: true });
      rmSync(join(dir, `${n}.wav`), { force: true });
    }
  }
  console.log(`Gardé : ${to}. Variantes supprimées.`);
  process.exit(0);
}

if (cmd === "render") {
  const list = positional[1] ? [positional[1]] : names();
  for (const n of list) save(n, JSON.parse(readFileSync(join(dir, `${n}.sfx.json`), "utf8")));
  process.exit(0);
}

if (cmd === "upload") {
  const ids = readIds();
  const list = (positional[1] ? [positional[1]] : names()).filter((n) => !/_v\d+$/.test(n));
  for (const n of list) {
    const file = join(dir, `${n}.wav`);
    if (!existsSync(file)) {
      console.error(`${n} : pas de WAV, lance render.`);
      continue;
    }
    const h = hash(readFileSync(file));
    if (ids[n]?.id && ids[n].hash === h) {
      console.log(`${n} : déjà publié (${ids[n].id})`);
      continue;
    }
    const out = execFileSync("node", [join(root, "tools", "lumen-asset.mjs"), "upload", join("assets", "sfx", `${n}.wav`), `SFX ${n}`], {
      cwd: root,
      encoding: "utf8",
    });
    const m = out.match(/rbxassetid:\/\/(\d+)/);
    if (!m) {
      console.error(`${n} : pas d'id Roblox.\n${out}`);
      continue;
    }
    ids[n] = { id: `rbxassetid://${m[1]}`, hash: h };
    writeFileSync(idsFile, JSON.stringify(ids, null, 2));
    console.log(`${n} -> ${ids[n].id}`);
  }
  console.log("Lance maintenant : node tools/lumen-sfx.mjs sync");
  process.exit(0);
}

if (cmd === "sync") {
  const ids = readIds();
  const list = names().filter((n) => !/_v\d+$/.test(n));
  const entries = list.map((n) => `\t${n}: "${ids[n]?.id ?? ""}",`).join("\n");
  const missing = list.filter((n) => !ids[n]?.id);
  mkdirSync(outDir, { recursive: true });
  writeFileSync(
    join(outDir, "index.ts"),
    `// Généré par tools/lumen-sfx.mjs : ne pas modifier (relance \`node tools/lumen-sfx.mjs sync\`).
import { SoundService } from "@rbxts/services";

export const SfxIds = {
${entries}
} as const;

export type SfxName = keyof typeof SfxIds;

function sfxGroup(): SoundGroup {
	const found = SoundService.FindFirstChild("Sfx");
	if (found && found.IsA("SoundGroup")) return found;
	const group = new Instance("SoundGroup");
	group.Name = "Sfx";
	group.Volume = 0.6;
	group.Parent = SoundService;
	return group;
}

export interface SfxOptions {
	/** Volume du son (0 à 1, défaut 0.8). */
	volume?: number;
	/** Hauteur (1 = normale). */
	pitch?: number;
	/** Variation aléatoire de hauteur, ±6 % par défaut (évite l'effet mitraillette). */
	vary?: number;
	/** Pièce ou Attachment : son 3D positionné. Sans parent : son d'interface (2D). */
	parent?: Instance;
	/** Distance max d'un son 3D (défaut 80 studs). */
	range?: number;
}

/** Joue un bruitage une fois puis le nettoie. */
export function playSfx(name: SfxName, options: SfxOptions = {}): Sound | undefined {
	const id = SfxIds[name];
	if (id === "") {
		warn(\`[sfx] \${name} n'est pas publié : node tools/lumen-sfx.mjs upload puis sync\`);
		return undefined;
	}
	const sound = new Instance("Sound");
	sound.Name = name;
	sound.SoundId = id;
	sound.Volume = options.volume ?? 0.8;
	const vary = options.vary ?? 0.06;
	sound.PlaybackSpeed = (options.pitch ?? 1) * (1 + (math.random() * 2 - 1) * vary);
	sound.SoundGroup = sfxGroup();
	sound.RollOffMode = Enum.RollOffMode.InverseTapered;
	sound.RollOffMaxDistance = options.range ?? 80;
	sound.Parent = options.parent ?? SoundService;
	sound.Play();
	sound.Ended.Once(() => sound.Destroy());
	task.delay(10, () => sound.Parent !== undefined && sound.Destroy());
	return sound;
}
`,
  );
  console.log(`src/shared/sfx/index.ts écrit (${list.length} sons).`);
  if (missing.length) console.log(`ATTENTION : pas encore publiés (${missing.join(", ")}) : node tools/lumen-sfx.mjs upload puis sync.`);
  process.exit(0);
}

console.error(`Commande inconnue « ${cmd} ». Aide : node tools/lumen-sfx.mjs help`);
process.exit(1);
