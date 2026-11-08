---
name: lumen-animation
description: Animer dans un jeu Roblox (roblox-ts) - monstres et boss (y compris ceux construits avec Blender via lumen-3d), personnages, armes, objets, interface ; attaques avec avertissement, combos, réactions aux coups, hit-stop, secousses de caméra, IK, animation procédurale, réplication des animations. À utiliser dès qu'une chose doit bouger.
---

# Animation (Roblox, roblox-ts)

Référence : `tools/knowledge/lihfolk/animation/` (environ 115 notes de **LihFolk Vault V2**, CC BY 4.0, voir `CREDITS.md`), en anglais et en Luau. Point d'entrée : `animation/Animation Start Here.md` ; panne : `animation/07 Gotchas/Troubleshooting Table.md`. Cherche d'abord dans `tools/knowledge/lihfolk/INDEX.md`, puis lis seulement les sections utiles (`grep -n "^## "`), toujours `Common mistakes` et `Checklist`.

## Choisir la technique

| Besoin | Technique |
|---|---|
| Monstre ou objet fait avec Blender (`lumen-3d`, pièces séparées) | **Rig Motor6D par code + animation par code** (ci-dessous). Rien à publier. |
| Idle, respiration, flottement, rotation, bobbing, regard vers le joueur | Procédural : `Motor6D.Transform` écrit chaque image. |
| Objet ou décor (porte, plateforme, pièce qui tourne), UI | `TweenService` sur `CFrame` / propriétés (note `Tween and CFrame Animation of Objects`, `UI and GUI Animation`). |
| Personnage joueur R15 | Animations publiées jouées par l'`Animator` (`Humanoid.Animator.LoadAnimation`). Lumen ne publie pas d'animations : demande les ids à l'utilisateur, ou garde les animations par défaut. Jamais d'id temporaire (`RegisterKeyframeSequence`) dans le code livré. |

## Monstre Blender : rig par code

1. Dans le script Blender (`lumen-3d`), garde la hiérarchie et les pivots aux articulations, et ajoute un mini mesh `Root` (0.2) à l'origine. Le rapport `assets/drafts/<nom>/report.json` donne chaque `pivot` en coordonnées Blender : en Roblox, relatif au repère `Root`, c'est `(x, z, -y)`.
2. Côté jeu : une pièce racine (`Body`) ancrée ou déplacée par le serveur ; chaque autre pièce reliée à son parent par un **Motor6D nommé comme la pièce**, `Anchored = false`, `Massless = true`, `CanCollide = false`. Les `<Pièce>_Outline` et pièces lumineuses sont soudées (`WeldConstraint`) à leur pièce.

```ts
/** joints : { Head: { parent: "Body", pivot: new Vector3(0, 5.2, -0.4) } } (pivot relatif au repère Root). */
export function rigModel(model: Model, root: BasePart, joints: Record<string, { parent: string; pivot: Vector3 }>) {
	const motors = new Map<string, Motor6D>();
	for (const [name, j] of pairs(joints)) {
		const part = model.FindFirstChild(name, true) as BasePart | undefined;
		const parentPart = model.FindFirstChild(j.parent, true) as BasePart | undefined;
		if (!part || !parentPart) continue;
		const pivot = root.CFrame.mul(new CFrame(j.pivot));
		const m = new Instance("Motor6D");
		m.Name = name;
		m.Part0 = parentPart;
		m.Part1 = part;
		m.C0 = parentPart.CFrame.Inverse().mul(pivot);
		m.C1 = part.CFrame.Inverse().mul(pivot);
		m.Parent = parentPart;
		part.Anchored = false;
		part.Massless = true;
		part.CanCollide = false;
		motors.set(name, m);
	}
	return motors;
}
```

3. **Animation par code** : des clips écrits comme données (temps, rotations en degrés, position optionnelle) et des marqueurs, joués en écrivant `Motor6D.Transform` dans `RunService.PreSimulation`.

```ts
import { RunService } from "@rbxts/services";

type Key = [t: number, rx: number, ry: number, rz: number, px?: number, py?: number, pz?: number];
export interface Clip { length: number; loop: boolean; joints: Record<string, Key[]>; markers?: Record<string, number> }

const smooth = (k: number) => k * k * (3 - 2 * k);

function sample(keys: Key[], t: number) {
	let a = keys[0];
	let b = keys[keys.size() - 1];
	for (let i = 0; i < keys.size() - 1; i++) {
		if (t >= keys[i][0] && t <= keys[i + 1][0]) {
			a = keys[i];
			b = keys[i + 1];
			break;
		}
	}
	const span = b[0] - a[0];
	const k = span > 0 ? smooth(math.clamp((t - a[0]) / span, 0, 1)) : 0;
	const l = (x: number, y: number) => x + (y - x) * k;
	const pos = new Vector3(l(a[4] ?? 0, b[4] ?? 0), l(a[5] ?? 0, b[5] ?? 0), l(a[6] ?? 0, b[6] ?? 0));
	return new CFrame(pos).mul(CFrame.Angles(math.rad(l(a[1], b[1])), math.rad(l(a[2], b[2])), math.rad(l(a[3], b[3]))));
}

/** Joue un clip ; onMarker("Tele" | "Hit" | …) au passage de chaque marqueur. Renvoie la connexion (Disconnect pour arrêter). */
export function playClip(motors: Map<string, Motor6D>, clip: Clip, onMarker?: (name: string) => void, speed = 1) {
	let t = 0;
	const fired = new Set<string>();
	const conn = RunService.PreSimulation.Connect((dt) => {
		t += dt * speed;
		if (t > clip.length) {
			if (!clip.loop) {
				conn.Disconnect();
				return;
			}
			t %= clip.length;
			fired.clear();
		}
		for (const [name, keys] of pairs(clip.joints)) {
			const m = motors.get(name);
			if (m) m.Transform = sample(keys, t);
		}
		if (clip.markers && onMarker) {
			for (const [name, at] of pairs(clip.markers)) {
				if (t >= at && !fired.has(name)) {
					fired.add(name);
					onMarker(name);
				}
			}
		}
	});
	return conn;
}
```

Les angles sont interpolés en Euler : découpe les grandes rotations (jamais exactement 180° entre deux clés). Pour un rendu plus fin (easing par clé, mélange de clips), lis `Lua Keyframe Player` et `Keyframes, Easing and Interpolation`.

## Règles des créatures et du combat

- Le poids vient de l'**anticipation**, de l'**impact** et du **follow-through**, pas d'une lecture ralentie : écris lent, ne joue jamais sous 1×. Vitesse de cycle ≈ 1 / √(échelle) pour une copie agrandie.
- Chaque attaque a un **avertissement unique d'au moins 0,45 s** (0,5 s pour un boss), avec les marqueurs `Tele` (début de l'avertissement : effet au sol, lueur) et `Hit` (dégâts, effet d'impact, son, secousse). Les dégâts serveur partent sur `Hit`, avec la même forme que l'avertissement dessiné.
- Hit-stop et réactions : voir `Hit Reactions Stagger and Hit-Stop` ; secousse de caméra courte et décroissante : `Camera Shake and Camera FX`.
- **Procédural** : écris les articulations animées dans `PreSimulation` ; compose avec la valeur animée (`R.mul(A)`), sans accumuler (`Transform = Transform * R` dérive) ; accepte `Motor6D`, `AnimationConstraint` et `Bone` quand tu cherches une articulation.
- **Réplication** : un `Animator` sous le personnage d'un joueur réplique ce que ce client joue ; ailleurs, seul le serveur réplique. Pour les ennemis animés par code : le serveur écrit l'état en attributs (`Anim`, `AnimSpeed`, puis `AnimT` en dernier) et chaque client anime ; un joueur qui arrive en retard reprend à `(now - AnimT) * speed` (note `Enemy Animation State Protocol`).
- Effets synchronisés : déclenche les VFX sur les marqueurs (skill `lumen-vfx`, note `VFX Timing with Animation`).

## Notes à connaître

`Animating Creatures and Monsters`, `Boss Animation and Phases`, `Enemy and NPC Animation`, `Enemy Animation State Protocol`, `Combat Combo Systems`, `Hit Reactions Stagger and Hit-Stop`, `Weapon Swing Design`, `Timing Spacing and Frame Charts`, `Principles of Animation for Games`, `Idle Breathing and Secondary Motion`, `Procedural Animation`, `Springs Damping and Smoothing`, `Look-At Aim and Head Tracking`, `Motor6D Math and CFrame Composition`, `Playback, Priorities and Blending`, `Markers and Events`, `Replication and Ownership`, `Publishing Animations`, `Camera Shake and Camera FX`, `Troubleshooting Table`.
