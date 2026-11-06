# Planche de concept : niveau / arène

Prompt pour `node tools/lumen-asset.mjs image "…"`, en anglais. Remplace **tous** les crochets. Si l'utilisateur a des captures de ses niveaux précédents, lis-les et décris leur style dans le prompt pour garder une cohérence.

```
Create a concept art sheet of LEVEL [NUMBER] "[LEVEL NAME]" of a [Roblox / Fortnite] [GAME GENRE, e.g. monster-catching simulator]. [If previous levels exist: It must look like it belongs to the same game as the previous levels: same building style, only the colors and the theme change.] Theme: [THEME IN ONE SENTENCE, e.g. a frozen glacier cave / a jungle temple / a volcano], where the [MONSTER NAME] lives. It is a big rectangular fighting arena with details along the walls and a few low details towards the middle, but always with clear open space to fight. Show only the environment: no characters, no monsters, no text, no logos, no UI.

Style: walls built from clean tall rectangular pillars side by side (no gaps) with flat caps on top and drips hanging from the caps, flat faces and sharp edges; props in simple LOW POLY (faceted, flat shaded, a few big shapes), bright and readable for a game. No LEGO studs, no realistic details, no textures, no letters.

Colors ([MOOD: e.g. cold and bright / warm and dangerous], but bright and readable):
- Ground: checkerboard of 5x5 stud tiles in two [THEME] colors ([#HEX] and [#HEX]), [optional: a few thin glowing cracks / snow patches / moss lines (#HEX)].
- Wall: [WALL MATERIAL] pillars (main [#HEX], dark sides [#HEX], light top edges [#HEX]) [optional: with a glowing band (#HEX) near the top].
- Caps: [CAP MATERIAL] [#HEX] flat caps (slightly wider than the pillar, darker edges [#HEX]), with [DRIPS: moss / icicles / lava / molten gold / vines] hanging down the faces ([#HEX] and [#HEX]).
- Props: [MAIN PROP MATERIAL #HEX], [SECOND MATERIAL #HEX], [GLOW / FIRE / WATER #HEX].
- Sky: [SKY: e.g. bright blue with clouds / dark night with aurora #HEX to #HEX].

Layout (big rectangular arena, deeper than wide; a player is tiny, about 1/[40] of the width):
- GROUND: FLAT checkerboard, no hills, no holes, no deep water.
- SIDE WALLS: one continuous solid wall of tall rectangular pillars side by side, touching with NO gaps, different widths and heights (alternating taller and shorter like battlements), about 7-8 times the height of a player, each with its cap and drips.
- FRONT WALL: the same pillars, with ONE entrance opening in the middle, framed by a big gate: [GATE DESIGN].
- BACK WALL: the same pillars, with ONE EXIT opening in the middle, framed by the same gate. On the [left / right] of the exit, against the back wall, [LANDMARK: the big signature object of the level].
- DETAILS ALONG THE WALLS: [4-6 KINDS OF PROPS with numbers, e.g. 4 giant crystal clusters in the corners; 6 braziers along the side walls; 4 obelisks; a few big rocks].
- DETAILS TOWARDS THE CENTER (low or high up, so the fight stays free): [e.g. a big FLAT symbol on the floor; 4 small waist-high plinths; one big object FLOATING HIGH above the center].

Keep at least 60% of the ground free for the fight: the center area and wide lanes between the details stay open.

NOT allowed: houses, castles, fences, signs with letters, stairs, swirling portals, LEGO studs, lasers, robots, modern sci-fi machines, any other opening in the walls.

Sheet layout: 3 images of the SAME level on one sheet: 1) big isometric 3/4 overview from above, seen from the entrance side, 2) top view (plan), 3) ground level view from the entrance looking towards the exit gate and the landmark. Keep every part clearly readable so it can be rebuilt in 3D.
```

## Ensuite, construire le niveau

Repère : origine (0, 0, 0) = centre de l'arène au niveau du sol ; -Y (Blender) / -Z (Roblox) = côté entrée ; +X = est. 1 unité = 1 stud. Fixe d'abord les dimensions (largeur W, profondeur D) et écris-les.

**Roblox : la structure en Parts dans le jeu, pas en mesh.**
- Sol : x de -W/2 à W/2, z de -D/2 à D/2, dessus à Y = 0, 4 studs d'épaisseur, damier de cases 5 × 5 (coins sur des multiples de 5).
- Zone jouable : 5 studs à l'intérieur des murs.
- Murs latéraux : face intérieure à x = ±(W/2 - 5), 10 d'épaisseur. Piliers rectangulaires séparés mais collés (aucun trou), largeur 8 à 15, hauteur 32 à 44 (la plupart ~38), alternés comme des créneaux, certains en retrait de 1 à 2 ou en saillie de 1. Chaque pilier : chapeau plat de 1 d'épaisseur qui dépasse de 0.6 de chaque côté, coulures (bandes de 0.5 à 1 de large, 2 à 8 de long) à 0.05 des faces.
- Mur avant : même principe, face intérieure à z = -(D/2 - 5), 5 d'épaisseur, UNE ouverture x -25..25, Y 0..30. Porte : 2 piliers 6 × 6 de 44 de haut centrés à x = ±29, z = -(D/2 - 8), poutre de x -32 à 32, face basse à Y = 31. Rien devant l'ouverture sur 20 studs.
- Mur du fond : identique, ouverture de sortie et porte identique côté +z.
- Coins fermés.
- Zones libres : rien de plus haut que 0.2 dans le cercle central (rayon ~30) ni dans les couloirs x -15..15 et z -15..15 ; rien de haut à moins de 20 studs de l'entrée et de la sortie.
- Noms (utilisés par le code du jeu) : `Floor`, `Wall_West`, `Wall_East`, `Wall_North` (fond), `Wall_South` (avant), `WallCaps_*`, `Gate`, `ExitGate`. Repères : `LevelCenter` (0,0,0), `EntranceCenter`, `ExitCenter`, `LandmarkCenter`.

**Décors low poly dans Blender** (un mesh par type : `Landmark`, `CrystalClusters`, `Braziers`, `Obelisks`, `Rocks`, `Plinths`, `FloatingRocks`, `Glow`) : tailles exactes en studs, couleurs #HEX de la planche, grands et lisibles de loin. Exporte-les avec `draft`, place-les ensuite dans le jeu aux positions prévues.

**UEFN** : même méthode, mais la structure se fait avec des pièces de construction ou des meshes Fortnite calés sur la grille (5,12 m / 3,84 m), et les décors en FBX.
