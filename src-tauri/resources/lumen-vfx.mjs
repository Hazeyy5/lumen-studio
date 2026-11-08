// Effets visuels Lumen : assets/vfx/<Nom>.vfx.json (aperçu dans l'onglet VFX de Lumen) -> modules roblox-ts.
//
//   node tools/lumen-vfx.mjs textures   publie les textures de assets/vfx/textures/ (une fois) -> ids.json
//   node tools/lumen-vfx.mjs sync       écrit src/shared/vfx/runtime.ts et un module par effet
//
// Dans le jeu : import { buildVfx, playVfxAt } from "shared/vfx/runtime"; import { MonEffet } from "shared/vfx/MonEffet";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = process.env.LUMEN_PROJECT || process.cwd();
const vfxDir = join(root, "assets", "vfx");
const texDir = join(vfxDir, "textures");
const idsFile = join(texDir, "ids.json");
const outDir = join(root, "src", "shared", "vfx");
const cmd = process.argv[2];

const readIds = () => (existsSync(idsFile) ? JSON.parse(readFileSync(idsFile, "utf8")) : {});

if (cmd === "textures") {
  const ids = readIds();
  for (const file of readdirSync(texDir).filter((f) => f.endsWith(".png"))) {
    const name = file.replace(/\.png$/, "");
    if (ids[name]) continue;
    const out = execFileSync("node", [join(root, "tools", "lumen-asset.mjs"), "upload", join("assets", "vfx", "textures", file), `VFX ${name}`], {
      cwd: root,
      encoding: "utf8",
    });
    const m = out.match(/rbxassetid:\/\/(\d+)/);
    if (!m) {
      console.error(`Pas d'id pour ${file} :\n${out}`);
      process.exit(1);
    }
    ids[name] = `rbxassetid://${m[1]}`;
    writeFileSync(idsFile, JSON.stringify(ids, null, 2));
    console.log(`${name} -> ${ids[name]}`);
  }
  console.log("Textures publiées. Lance maintenant : node tools/lumen-vfx.mjs sync");
  process.exit(0);
}

if (cmd !== "sync") {
  console.log("Usage : node tools/lumen-vfx.mjs textures | sync");
  process.exit(cmd ? 1 : 0);
}

const RUNTIME = `// Généré par tools/lumen-vfx.mjs : ne pas modifier (relance \`node tools/lumen-vfx.mjs sync\`).
export interface VfxLayer {
	name: string;
	textureId: string;
	color: [number, string][];
	size: [number, number][];
	transparency: [number, number][];
	lifetime: [number, number];
	speed: [number, number];
	spread?: [number, number];
	acceleration?: [number, number, number];
	drag?: number;
	lightEmission?: number;
	rotation?: [number, number];
	rotSpeed?: [number, number];
	zOffset?: number;
	lockedToPart?: boolean;
	offset?: [number, number, number];
	emits?: [number, number][];
	rate?: number;
	rateFrom?: number;
	rateTo?: number;
}

export interface VfxSpec {
	name: string;
	duration: number;
	loop?: boolean;
	layers: VfxLayer[];
}

export interface VfxHandle {
	attachments: Attachment[];
	play(): void;
	destroy(): void;
}

const colors = (k: [number, string][]) =>
	new ColorSequence(k.map(([t, c]) => new ColorSequenceKeypoint(t, Color3.fromHex(c))));
const numbers = (k: [number, number][]) => new NumberSequence(k.map(([t, v]) => new NumberSequenceKeypoint(t, v)));

/** Construit l'effet sous \`parent\` (une Part) ; play() lance les rafales et flux prévus. */
export function buildVfx(spec: VfxSpec, parent: BasePart): VfxHandle {
	const emitters: [VfxLayer, ParticleEmitter][] = [];
	const attachments: Attachment[] = [];
	for (const layer of spec.layers) {
		const att = new Instance("Attachment");
		att.Name = \`Vfx_\${spec.name}_\${layer.name}\`;
		const o = layer.offset ?? [0, 0, 0];
		att.Position = new Vector3(o[0], o[1], o[2]);
		att.Parent = parent;
		const e = new Instance("ParticleEmitter");
		e.Name = layer.name;
		e.Texture = layer.textureId;
		e.Color = colors(layer.color);
		e.Size = numbers(layer.size);
		e.Transparency = numbers(layer.transparency);
		e.Lifetime = new NumberRange(layer.lifetime[0], layer.lifetime[1]);
		e.Speed = new NumberRange(layer.speed[0], layer.speed[1]);
		const s = layer.spread ?? [0, 0];
		e.SpreadAngle = new Vector2(s[0], s[1]);
		const a = layer.acceleration ?? [0, 0, 0];
		e.Acceleration = new Vector3(a[0], a[1], a[2]);
		e.Drag = layer.drag ?? 0;
		e.LightEmission = layer.lightEmission ?? 0;
		e.LightInfluence = 0;
		const r = layer.rotation ?? [0, 0];
		e.Rotation = new NumberRange(r[0], r[1]);
		const rs = layer.rotSpeed ?? [0, 0];
		e.RotSpeed = new NumberRange(rs[0], rs[1]);
		e.ZOffset = layer.zOffset ?? 0;
		e.LockedToPart = layer.lockedToPart ?? false;
		e.Rate = 0;
		e.Enabled = true;
		e.Parent = att;
		emitters.push([layer, e]);
		attachments.push(att);
	}
	let alive = true;
	return {
		attachments,
		play() {
			for (const [layer, e] of emitters) {
				for (const [t, n] of layer.emits ?? []) {
					task.delay(t, () => {
						if (alive) e.Emit(n);
					});
				}
				const rate = layer.rate ?? 0;
				if (rate > 0) {
					task.delay(layer.rateFrom ?? 0, () => {
						if (alive) e.Rate = rate;
					});
					task.delay(layer.rateTo ?? spec.duration, () => {
						if (alive) e.Rate = 0;
					});
				}
			}
		},
		destroy() {
			alive = false;
			for (const att of attachments) att.Destroy();
		},
	};
}

/** Joue l'effet une fois à une position du monde, puis nettoie. */
export function playVfxAt(spec: VfxSpec, cf: CFrame, parent: Instance = game.Workspace) {
	const p = new Instance("Part");
	p.Anchored = true;
	p.CanCollide = false;
	p.CanQuery = false;
	p.CanTouch = false;
	p.Transparency = 1;
	p.Size = new Vector3(0.2, 0.2, 0.2);
	p.CFrame = cf;
	p.Parent = parent;
	const handle = buildVfx(spec, p);
	handle.play();
	let longest = 0;
	for (const layer of spec.layers) longest = math.max(longest, layer.lifetime[1]);
	task.delay(spec.duration + longest + 0.1, () => p.Destroy());
	return handle;
}
`;

const ids = readIds();
const files = existsSync(vfxDir) ? readdirSync(vfxDir).filter((f) => f.endsWith(".vfx.json")) : [];
mkdirSync(outDir, { recursive: true });
writeFileSync(join(outDir, "runtime.ts"), RUNTIME);
const missing = new Set();
for (const file of files) {
  const spec = JSON.parse(readFileSync(join(vfxDir, file), "utf8"));
  const ident = String(spec.name || file.replace(/\.vfx\.json$/, "")).replace(/[^A-Za-z0-9_]/g, "_").replace(/^(\d)/, "_$1");
  const layers = (spec.layers || []).map((l) => {
    let textureId = l.textureId;
    if (!textureId) {
      textureId = ids[l.texture] || "rbxasset://textures/particles/sparkles_main.dds";
      if (!ids[l.texture]) missing.add(l.texture);
    }
    const out = { name: l.name, textureId, color: l.color, size: l.size, transparency: l.transparency, lifetime: l.lifetime, speed: l.speed };
    for (const k of ["spread", "acceleration", "drag", "lightEmission", "rotation", "rotSpeed", "zOffset", "lockedToPart", "offset", "emits", "rate", "rateFrom", "rateTo"]) {
      if (l[k] !== undefined) out[k] = l[k];
    }
    return out;
  });
  const body = { name: spec.name || ident, duration: spec.duration ?? 1, loop: Boolean(spec.loop), layers };
  writeFileSync(
    join(outDir, `${ident}.ts`),
    `// Généré depuis assets/vfx/${file} par tools/lumen-vfx.mjs : modifie le JSON, pas ce fichier.\nimport type { VfxSpec } from "./runtime";\n\nexport const ${ident}: VfxSpec = ${JSON.stringify(body, null, "\t")};\n`,
  );
  console.log(`${file} -> src/shared/vfx/${ident}.ts`);
}
if (missing.size) {
  console.log(`ATTENTION : textures sans id Roblox (${[...missing].join(", ")}) : lance \`node tools/lumen-vfx.mjs textures\` puis \`sync\`.`);
}
