---
name: lumen-3d
description: Créer un modèle 3D sur mesure (monstre, boss, personnage, arme, décor, objet emblématique d'un niveau, props d'une map) en low poly stylisé avec Blender, à partir d'une planche de concept, puis l'importer dans Roblox, UEFN ou Unreal. À utiliser dès qu'un modèle 3D n'existe pas dans la banque Lumen.
---

# Modèles 3D sur mesure (Blender, low poly)

Tu construis le modèle **en code** (script Blender + `lumen3d`), tu **regardes les rendus**, tu corriges, puis tu exportes. C'est la même méthode que « Claude Design → GLB », en mieux : Blender gère contours, pivots, hiérarchie et animations.

## 0. Avant tout

1. Cherche d'abord dans la banque (skill `lumen-assets`, `search mesh …`). Ne construis que si rien ne convient.
2. `node tools/lumen-asset.mjs status` : il faut Blender (`blender` détecté). Sinon, dis à l'utilisateur d'installer Blender 4.x.

## 1. Planche de concept

- Si l'utilisateur fournit des images (planche, captures), lis-les avec Read : c'est la référence.
- Sinon, génère une planche : `node tools/lumen-asset.mjs image "<prompt>"` avec le modèle de [references/concept-monstre.md](references/concept-monstre.md) (monstre, personnage, objet) ou [references/concept-map.md](references/concept-map.md) (niveau). Remplis tous les crochets (thème, couleurs #HEX, parties). Montre la planche à l'utilisateur et attends son accord avant de modéliser.
- Note la liste des parties, les couleurs exactes et les proportions : tu vas les recopier.

## 2. Script Blender

Écris `assets/blender/<nom>.py`. Commence par `from lumen3d import *`. Aides disponibles :

| Aide | Usage |
|---|---|
| `box(name, (x,y,z), loc, "#hex", bevel=0.2, light="#hex")` | blocs, plaques, piliers, armures |
| `prism(name, rayon, hauteur, sides=6, loc, "#hex", top_scale=0.7)` | membres, troncs, colonnes (jamais de cylindre lisse) |
| `cone(name, rayon, hauteur, sides=5, …)` | cornes, pics, griffes, dents |
| `rock(name, rayon, loc, "#hex", seed=, squash=(x,y,z))` | toute forme ronde : tête, corps, poing, rocher (facettes, jamais de sphère) |
| `crystal(name, rayon, hauteur, loc, "#hex", tilt=(rx, ry))` | cristaux, éclats |
| `wedge(name, (x,y,z), loc, "#hex")` | mâchoire, griffe plate, plaque inclinée |
| `paint(obj, "#hex", light="#hex")`, `paint_faces(obj, "#hex", lambda p: p.normal.z > 0.5)` | couleurs de sommets (faces du haut plus claires = facettes lisibles) |
| `set_pivot(obj, (x,y,z))` | pivot sur l'articulation (cou, épaule, genou, base de queue) |
| `parent(enfant, parent)` | hiérarchie (tête → corps, mâchoire → tête, bas de patte → haut de patte) |
| `outline(obj, 0.15)` | contour noir cartoon en géométrie (inverted hull) → `<Nom>_Outline`, enfant de la pièce |
| `glow(obj)` | pièce lumineuse (yeux, fissures, runes) : mesh séparé, sans contour |
| `mirror_x(obj, "NomR")` | copie symétrique gauche/droite |
| `join([objs], "Rocks")` | fusionner les décors d'un même type en un mesh nommé |
| `empty("LevelCenter", (0,0,0))` | repère vide (centre, entrée, point d'attache) |

Tu peux aussi utiliser `bpy` et `bmesh` directement. Pas de caméra, de lumière ni de sol dans le script : Lumen les gère.

## 3. Brouillon → regarder → corriger

```
node tools/lumen-asset.mjs draft assets/blender/<nom>.py <Nom>
```

- Rien n'est publié. Sortie : `assets/drafts/<nom>/` avec le modèle, `report.json` et 6 rendus (`view_front`, `back`, `left`, `right`, `threequarter`, `top`). `--views front,threequarter` pour aller plus vite.
- **Lis les rendus avec Read** et compare-les à la planche : silhouette, proportions, couleurs, parties manquantes, pièces qui flottent ou se traversent, contour présent partout sauf sur les glow.
- Lis le rapport affiché : triangles par mesh, pivots, hiérarchie, alertes (`ATTENTION`). Corrige toutes les alertes.
- Recommence jusqu'à ce que ce soit fidèle (en général 3 à 6 tours). Ne montre pas chaque tour à l'utilisateur : montre-lui le rendu `threequarter` final et demande validation.

## 4. Règles de style (par défaut, sauf autre demande)

- Low poly stylisé à facettes : plans plats, arêtes nettes, ombrage plat. **Pas de sphère ni de forme molle** (les rondeurs deviennent des cristaux ou des rochers à facettes). Pas de voxel / empilement de cubes, sauf blocs voulus. Pas de picots LEGO, pas de texte.
- Couleurs de sommets uniquement, pas de textures. Couleurs vives (le moteur les assombrit). Une teinte plus claire sur les faces du haut (`light=`).
- Contour noir : `outline()` sur chaque pièce visible, épaisseur 0.12 à 0.18 (en studs), jamais sur les pièces `glow`.
- Pièces lumineuses : meshes séparés nommés (Eyes, Glow, Runes…), posés 0.05 devant la surface.
- Aucune face superposée ou coplanaire entre deux pièces qui se touchent (scintillement) : décale de 0.05.

## 5. Structure (indispensable pour animer et coder)

- **Meshes séparés et nommés** en PascalCase (`Body`, `Head`, `Jaw`, `Eyes`, `LegFL_Upper`, `LegFL_Lower`, `Tail1`…), pas un mesh fusionné. Pour une map : un mesh par type de décor (`Rocks`, `Crystals`, `Landmark`…) + `Glow`.
- Hiérarchie réelle (racine = `Body`), pivot de chaque pièce sur son articulation.
- Moins de 10 000 triangles par mesh (Roblox refuse au-delà). Total conseillé : 5 000 à 15 000 pour un monstre, 30 000 à 80 000 pour une map entière, contours compris.
- Le modèle repose sur le sol (Z = 0), centré en X/Y, l'avant vers **-Y** dans Blender.

## 6. Par moteur

### Roblox
- 1 unité Blender = 1 stud. Un joueur mesure environ 5 studs.
- `draft` exporte en GLB et repasse les couleurs de sommets en sRGB (sinon elles sont trop sombres dans Roblox).
- Publication : `node tools/lumen-asset.mjs upload assets/drafts/<nom>/<nom>.glb <Titre>` → `rbxassetid`. Côté serveur : `InsertService:LoadAsset(id)` puis récupère le Model.
- Vérifié dans Studio : couleurs, facettes, contour (inverted hull) et orientation arrivent comme sur les rendus `draft`. Le modèle arrive en pièces séparées sous un Model.
- Pour le mettre dans la place ouverte (mode édition, sauvegardé avec la place) : `node tools/lumen-studio.mjs insert <assetId> --name Nom --replace --neon Eyes,Glow,Cracks`. Ne demande jamais à l'utilisateur de coller une commande dans Studio.
- Pièces lumineuses : `Material = Neon` (ou SurfaceAppearance/éclairage) sur les MeshParts nommés Eyes / Glow…
- Animations : **expérimental**. Le plus fiable : modèle statique bien découpé, puis Motor6D posés aux pivots et animation dans Roblox (Animation Editor ou code). Préviens l'utilisateur si tu tentes des animations Blender dans le GLB : leur import n'est pas garanti.
- **Maps** : murs, sol en damier, piliers, portes = blocs → construis-les **en Parts dans le jeu** (collisions et couleurs exactes, modifiables). Seuls les décors low poly (rochers, cristaux, obélisques, objet emblématique) passent par Blender.

### UEFN / Unreal
- 1 unité Blender = 1 mètre (export FBX, Unreal convertit en cm). Un personnage Fortnite mesure environ 1,9 m ; grille de construction 5,12 m × 5,12 m, étages 3,84 m.
- `draft` exporte en FBX par défaut dans ces projets (UEFN refuse souvent le GLB). `--glb` pour forcer.
- Import : UEFN → outil MCP `import_file` ; Unreal → outil `LumenTools.import_file` du MCP. Puis pose-le dans le niveau.
- Matériaux : couleurs de sommets ; dans UEFN, un matériau qui lit la Vertex Color est nécessaire pour les voir.

## 7. Ce qu'il ne faut pas faire

- Ne pas inventer d'ID Roblox, ne pas publier un brouillon non validé par l'utilisateur.
- Ne pas laisser de pièce sans couleur ni de mesh au-dessus de 10 000 triangles.
- Ne pas viser le réalisme sculpté : ce procédé est fait pour le low poly stylisé.
