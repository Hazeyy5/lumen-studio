# Planche de concept : monstre / personnage / objet

Prompt pour `node tools/lumen-asset.mjs image "…"`. Écris-le en anglais (meilleurs résultats). Remplace **tous** les crochets ; supprime les lignes optionnelles inutiles. Garde la même liste de parties pour le script Blender ensuite.

```
Create a character turnaround sheet of an original fantasy monster for a [Roblox / Fortnite] game: "[MONSTER NAME]", the monster of level [NUMBER] ("[LEVEL NAME]").

Style: low poly 3D game asset, chunky stylized proportions, angular faceted shapes with flat planes, flat cartoon colors, thick black outline around every part, soft studio lighting, pure white background. Same style as a stylized low poly mobile game monster. No text, no logos, no ground, no base, no shadow, no aura ring. No round balls or blob shapes.

Design: [WHAT IT IS: animal / creature / living armor / elemental...] inspired by [LEVEL THEME]. [BODY SHAPE: e.g. huge heroic silhouette, wide shoulders, thick legs, long tail...]. It stands [POSE: tall and proud / low and ready to pounce], slightly leaning forward, ready to fight.
Body: [MAIN MATERIAL] ([COLOR #HEX], lighter facets [COLOR #HEX]) with [GLOWING DETAIL: glowing cracks / runes / veins] ([COLOR #HEX]), framed by [TRIM MATERIAL: gold / ice / bone trims] ([COLOR #HEX]).
Head: [HEAD: face, glowing eyes #HEX, horns / crests / helmet], [MOUTH: fangs, tongue...].
Special feature: [THE MOST RECOGNIZABLE ELEMENT: e.g. a burning star core in the chest / ice crystals on the back / a flaming mane].
Arms / legs: [LIMBS: claws, big fists, shoulder armor...].
Weapon or attack part (optional): [WEAPON OR ATTACKING PART: hammer, whip tail, horns...].
Back / tail: [BACK AND TAIL: spikes, cape, crystals, fins...].
Small details: [3-4 SMALL DETAILS: rivets, chains, gems...].

Sheet layout: 6 views of the SAME model, same size, evenly spaced on one image in 2 rows: front, back, left side, right side, 3/4 front, top. Neutral standing pose, limbs slightly away from the body, head straight. Keep every part clearly separated and readable ([PART LIST: body, head, eyes, horns, upper legs, lower legs, paws, tail, back crystals...]) so it can be rebuilt in 3D.
```

## Ensuite, pour le script Blender

Reprends de la planche, puis applique les règles du skill :

- **Échelle** : hauteur × largeur × longueur en studs (Roblox) ou en mètres (UEFN/Unreal). Un joueur Roblox ≈ 5 studs.
- **Parties nommées** et hiérarchie, par exemple : `Body` (racine) → `Head`, `LegFL_Upper`…, `Tail1` ; `Head` → `Jaw`, `Eyes`, `HornL`, `HornR` ; `LegFL_Upper` → `LegFL_Lower` → `PawFL` ; `Tail1` → `Tail2` → `Tail3`.
- **Pivots** aux articulations : cou, épaules, hanches, genoux, chevilles, base de chaque segment de queue, charnière de la mâchoire.
- **Pièces lumineuses** séparées (`glow`) : yeux, fissures, runes, cœur. Sans contour.
- **Contour** `outline()` sur toutes les autres pièces.
- **Pose neutre** de la planche ; budget triangles du skill.
- Si l'utilisateur veut des attaques ou des animations : liste-les (durée, image de l'impact, portée en studs, zone de touche vue de dessus, temps de recharge), mais anime de préférence dans le moteur (voir le skill, section Roblox).
