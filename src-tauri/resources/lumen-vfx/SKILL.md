---
name: lumen-vfx
description: Effets visuels et sonores d'un jeu Roblox (roblox-ts) - particules, beams, trails, impacts, slashes, projectiles, auras, avertissements au sol d'une attaque, effets de collecte, météo, éclairage et ambiance, sons de feedback, « juice ». À utiliser dès qu'il faut créer, corriger ou optimiser un effet.
---

# VFX, éclairage et son (Roblox, roblox-ts)

La connaissance de référence est dans `tools/knowledge/lihfolk/` : 40 notes VFX / éclairage / audio de **LihFolk Vault V2** (CC BY 4.0, voir `CREDITS.md`). Elles sont en anglais, en Luau ; tu écris en TypeScript (roblox-ts).

## Méthode

1. **Cherche** dans `tools/knowledge/lihfolk/INDEX.md` (Grep sur un mot-clé : `slash`, `telegraph`, `aura`, `pickup`, `rain`, `lighting`, `budget`…). Chaque note y a son résumé : symptôme -> correction, puis checklist.
2. **Lis seulement les sections utiles** de 1 à 3 notes : `grep -n "^## " "<note>"` puis la plage voulue. Toujours `## Common mistakes` et `## Checklist` ; jamais une note entière.
3. **Écris la table des couches avant le code** : pour chaque couche, rôle (encre sombre, corps, cœur blanc, accent), texture, couleur, taille, durée, nombre de particules.
4. **Code en TypeScript**, toutes les propriétés posées explicitement (les valeurs par défaut de `new Instance` diffèrent de celles de Studio).
5. **Vérifie** contre la checklist de la note, puis en jeu (Play) sur un sol clair et un sol sombre.

## Règles qui reviennent partout

- **Valeurs** : chaque image a une encre sombre **saturée** (bleu-noir, encre de l'élément, jamais gris), le corps de l'élément, un **cœur blanc fin**, au plus une teinte d'accent. Une couche sombre par pile lumineuse ; le blanc seulement en cœur fin.
- **`LightInfluence = 0`** sur presque toutes les couches (sinon noir dans l'ombre). Luminosité choisie selon la surface à l'écran ; `LightEmission` 1 = additif (ne lit pas le monde).
- **Rythme** : l'image la plus chaude dure moins de 0,1 s ; coupe chaque phase. Étincelles dans la direction du coup, pas en étoile, sur un coup directionnel.
- **Rafales** avec `emitter.Emit(n)` (et ton propre facteur de qualité), flux plafonnés. Limites moteur : 400 particules/s par émetteur (100 sur mobile), durée de vie max 20 s, flipbooks en texture 1024 × 1024, 30 i/s max.
- **Avertissement au sol** d'une attaque : 4 couches (contour sombre, fond, remplissage de progression avec bord chaud, liseré clair) ; durée = 0,25 s de réaction + le temps de sortir en marchant ; dégâts **sur l'image du « pop »**, même table de forme pour le dessin et le test serveur.
- **Feedback** : à chaque action du joueur, un pop + un son en moins de 100 ms ; sur un coup, le feedback part sur le marqueur d'impact, pas au clic.
- **Réplication** : le lanceur joue l'effet tout de suite chez lui ; le serveur valide (forme, liste autorisée, valeurs finies, vivant, portée, recharge, seau de jetons) puis relaie **des ids et des nombres** aux autres clients, jamais au lanceur. Les clients construisent l'effet.
- **Coût** : pools plafonnés (pas de clone/destroy par coup), budget par lancer, par lanceur et par client, spectateurs à moitié ou moins, un « gouverneur » global. Les effets de gameplay ne sont jamais supprimés, seulement simplifiés.
- **Éléments** : la couleur ne suffit pas à distinguer (feu/sang, glace/foudre) ; chaque élément a sa signature de mouvement lisible en gris (voir « Elemental VFX Kits »). La glace n'apparaît pas en fondu.

## Écrire un effet : format Lumen (aperçu dans l'app)

Chaque effet de particules est un fichier **`assets/vfx/<Nom>.vfx.json`** : l'utilisateur le voit joué en 3D dans l'onglet **VFX** de Lumen (sol clair/sombre, mannequin de 5 studs, ralenti), mis à jour à chaque modification. Exemple complet : `assets/vfx/ExempleImpactGlace.vfx.json`.

```json
{ "name": "IcePunch", "duration": 1.2, "loop": true, "layers": [
  { "name": "Coeur", "texture": "star", "color": [[0, "#FFFFFF"], [1, "#BFF6FF"]], "size": [[0, 4], [1, 1]],
    "transparency": [[0, 0], [1, 1]], "lifetime": [0.08, 0.1], "speed": [0, 0], "spread": [0, 0],
    "acceleration": [0, 0, 0], "drag": 0, "lightEmission": 1, "rotation": [0, 360], "rotSpeed": [0, 0],
    "zOffset": 0.5, "offset": [0, 2, 0], "emits": [[0, 1]], "rate": 0, "rateFrom": 0, "rateTo": 0 } ] }
```

- Propriétés = celles d'un `ParticleEmitter` (séquences `[temps 0..1, valeur]`, premier temps 0, dernier 1 ; plages `[min, max]`). `spread` = SpreadAngle X, Z ; émission vers le haut de l'attachment placé à `offset`. `emits` = rafales `[seconde, nombre]` ; `rate` entre `rateFrom` et `rateTo` = flux.
- `texture` : une texture de base (`glow`, `spark`, `ring`, `smoke`, `shard`, `star`, dans `assets/vfx/textures/`), ou le chemin d'un PNG du projet avec `textureId` (`rbxassetid://…`).
- L'aperçu est une approximation (particules toujours face caméra, pas d'éclairage) : règle couleurs, rythme et lisibilité dedans, valide en Play dans Studio.

Pour le jeu :
1. `node tools/lumen-vfx.mjs textures` (une fois par projet) : publie les textures de base, garde les ids dans `assets/vfx/textures/ids.json`.
2. `node tools/lumen-vfx.mjs sync` après chaque modification : écrit `src/shared/vfx/runtime.ts` (`buildVfx`, `playVfxAt`) et un module par effet. Ne modifie jamais ces fichiers générés : modifie le JSON.
3. Dans le code : `import { playVfxAt } from "shared/vfx/runtime"; import { IcePunch } from "shared/vfx/IcePunch"; playVfxAt(IcePunch, hitCFrame);` ou `buildVfx(IcePunch, part).play()` pour un effet attaché à une pièce.

Beams, trails, Highlights, lumières et tweens restent écrits directement en TypeScript (patron ci-dessous).

## Patron roblox-ts

```ts
import { Debris, TweenService } from "@rbxts/services";

/** Émetteur entièrement défini (aucune valeur par défaut implicite). */
export function makeEmitter(parent: Attachment | BasePart, o: {
	texture: string; color: ColorSequence; size: NumberSequence; transparency: NumberSequence;
	lifetime: NumberRange; speed: NumberRange; spread?: Vector2; lightEmission?: number;
	acceleration?: Vector3; drag?: number; zOffset?: number; orientation?: Enum.ParticleOrientation;
}) {
	const e = new Instance("ParticleEmitter");
	e.Texture = o.texture;
	e.Color = o.color;
	e.Size = o.size;
	e.Transparency = o.transparency;
	e.Lifetime = o.lifetime;
	e.Speed = o.speed;
	e.SpreadAngle = o.spread ?? new Vector2(0, 0);
	e.LightEmission = o.lightEmission ?? 0;
	e.LightInfluence = 0;
	e.Acceleration = o.acceleration ?? Vector3.zero;
	e.Drag = o.drag ?? 0;
	e.ZOffset = o.zOffset ?? 0;
	e.Orientation = o.orientation ?? Enum.ParticleOrientation.FacingCamera;
	e.Rate = 0; // rafales seulement : e.Emit(n)
	e.Enabled = true;
	e.Parent = parent;
	return e;
}

/** Flash bref d'une Part (pop d'impact), puis nettoyage. */
export function flash(part: BasePart, to: Partial<WritableInstanceProperties<BasePart>>, seconds = 0.22) {
	TweenService.Create(part, new TweenInfo(seconds, Enum.EasingStyle.Quad, Enum.EasingDirection.Out), to).Play();
	Debris.AddItem(part, seconds + 0.05);
}
```

Textures : utilise celles de la banque Lumen (étagère Textures, `search texture …`) ou génère un PNG fond transparent avec `node tools/lumen-asset.mjs image "…"` puis `upload` pour l'id. N'invente jamais d'id.

## Notes à connaître

`VFX Design Playbook`, `Particle Emitters In Depth`, `Beams and Trails`, `Slash VFX Recipes - Crescent and Ring`, `Projectile VFX - Muzzle, Travel and Impact`, `Impact Decals and Material-Aware Hit Sparks`, `AoE Telegraphs - Readable Ground Warnings`, `Auras, Shields and Status Effects`, `Collectible Pickup FX - Idle Glint, Magnet Pull and Collect Pop`, `Elemental VFX Kits - Fire, Ice, Earth, Water, Wind and Lightning`, `Game Feel and Juice`, `Screen Damage Effects - Hit Flash, Low-Health Vignette and Blur`, `Replicating VFX - Server Validates, Clients Render`, `VFX Performance Budgets and Culling`, `VFX Readability and Value - Lessons from Blind Reviews`, `Stylized Lighting Recipes`, `Lighting Mood and Time of Day`, `Snow, Sandstorm, Blizzard and Ash Recipes`, `Combat Sound Design and Feedback`.

Synchroniser un effet avec une animation (marqueurs `Tele` / `Hit`) : voir le skill `lumen-animation`.
