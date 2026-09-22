---
name: lumen-assets
description: Cherche la banque Lumen/VibeStarter/Textures, propose des assets pour la map, génère image/mesh si besoin, publie sur Roblox. À utiliser aussi pour s'inspirer d'une capture d'UI (banque Inspiration, codes INS-xxxx) ou pour des textures HUD (TEX-xxxx).
---

# Assets Lumen

Ne demande jamais les clés API. Elles sont dans Lumen (Réglages). Pont :

```bash
node tools/lumen-asset.mjs status
node tools/lumen-asset.mjs search palm
node tools/lumen-asset.mjs search mesh totem --for "totem décoratif au spawn, le joueur passe à côté"
node tools/lumen-asset.mjs search image coin --for "icône pièce dans le HUD"
node tools/lumen-asset.mjs search inspiration hud --for "layout boutique cartoon"
node tools/lumen-asset.mjs search texture bouton --for "fond de bouton shop"
node tools/lumen-asset.mjs propose VS-2608 VS-8911 VS-5385 --for "pavillon asiatique au bord de la rampe"
node tools/lumen-asset.mjs get VS-0124
node tools/lumen-asset.mjs get TEX-0001
node tools/lumen-asset.mjs get INS-0001
node tools/lumen-asset.mjs image "icône pièce d'or, style Roblox, PNG fond transparent"
node tools/lumen-asset.mjs mesh "coffre low poly texturé pour tycoon Roblox"
node tools/lumen-asset.mjs blender assets/blender/crate.py Crate
node tools/lumen-asset.mjs publish VS-0124
```

## Map : bibliothèque d'abord

Avant de créer un prop, une icône ou un mesh, **cherche dans la banque** (Lumen + pack VibeStarter, codes `VS-xxxx`).

1. `search mesh <mots> --for "où ça va / à quoi ça sert"` (anglais + français, 2–4 mots de recherche). **`--for` est obligatoire** : une phrase FR courte que Lumen affiche sur le toast (ex. `pavillon décoratif au bord de la rampe, le joueur passe à côté`). Jamais `search mesh prop` tout seul.
2. `search` **attend** : Lumen ouvre un **menu déroulant d’environ 10 assets**, avec l’usage. L’utilisateur en **choisit un** (ou Aucune / ×).
3. La commande affiche `Choisi : VS-xxxx`. Ensuite **un seul** `get` sur ce code. **Ne get jamais les autres.**
4. Si `Aucune sélection` : ne get rien, propose d’autres mots ou génère.
5. Si l’utilisateur dit « montre / renvoie / je veux voir les propositions » : **ne reliste pas les IDs**. Relance `search` **ou** `propose` (jusqu’à 10 codes). Attends le choix, puis `get` uniquement le choisi.

Format terminal après choix :

```
Choisi : VS-0124
Ensuite uniquement : get VS-0124
```

6. Pour **utiliser** l’asset choisi : `get CODE`. Lumen affiche une notification : l’utilisateur prévisualise et **Valider**. La commande attend ce clic.
7. **Ne génère** (`image` / `mesh` / `blender`) **que si la recherche ne donne rien de pertinent, ou si l’utilisateur a cliqué Aucune.**

Avant `mesh` : `status`. Si `meshProvider` est `blender`, **n’envoie pas un prompt Meshy**. Écris un script bpy (voir plus bas).

Lumen doit être ouvert. `search` / `get` / `publish` passent par Lumen (`127.0.0.1:17422`).

- Image : `rbxassetid://<robloxAssetId>` sur Decal / ImageLabel / Texture. Pour une icône / un prop 2D, génère un **PNG fond transparent** (pas de fond uni, pas de décor). Les textures plein cadre (herbe, brique, ciel) peuvent garder un fond.
- Mesh : **serveur uniquement**, `InsertService.LoadAsset(tonumber(robloxAssetId))`, puis sors le MeshPart/Model. Ce n'est pas un `MeshId`.
- N'invente jamais d'ID. La recherche ne donne **pas** d’ID Roblox : seulement après `get` validé.
- `get` / `image` / `mesh` : toast en haut à droite de Lumen. Attends la validation. Ne continue pas sans `robloxAssetId`.

## Inspiration (captures d'UI)

La banque a une étagère **Inspiration** : captures d'HUD, boutiques, menus de maps existantes. Codes `INS-xxxx`. Ce ne sont **pas** des assets Roblox : jamais `publish`, jamais `rbxassetid`.

Si l'utilisateur demande de s'inspirer d'une image / d'un HUD / d'une UI :

1. `search inspiration <mots>` (ex. shop, hud, boutique, boutons).
2. Lumen montre un **menu d’environ 10** captures : l’utilisateur en choisit **une**.
3. `get` uniquement le `INS-xxxx` choisi — validation Lumen, puis copie dans `assets/inspiration/INS-0001.png`.
4. **Read** ce fichier image (vision) et reproduis le **layout, la hiérarchie, les couleurs, le rythme** dans une UI **originale**. Pas de copie pixel-perfect ni de logos / marques.
5. N'utilise pas ces images comme Decal in-game.

Si l'utilisateur **colle une capture** dans le terminal ou le brief Lumen, le fichier est dans `assets/inbox/paste-….png`. Le message contient le chemin : **Read** cette image tout de suite.

## Textures HUD / UI (`TEX-xxxx`)

Étagère **Textures** : PNG/JPG locaux (`Images/textures`) **et** IDs déjà sur Roblox (`rbxassetid://…` importés depuis Studio). Utilisables in-game (ImageLabel / ImageButton / `Texture` / `Decal` / `MeshPart.TextureID`).

Si l’utilisateur demande une texture, un fond d’UI, un motif, de l’herbe, de la brique, de l’asphalte :

1. `search texture <mots> --for "où ça va"` (ex. brick, grass, wood, bouton).
2. Lumen montre un **menu d’environ 10** textures : l’utilisateur en choisit **une**.
3. `get` uniquement le `TEX-xxxx` choisi. La commande affiche `rbxassetid://…` **ou** `rbxasset://…`.
4. Colle cette valeur telle quelle :
   - UI : `ImageLabel.Image` / `ImageButton.Image`
   - monde 3D : `Texture.Texture`, `Decal.Texture`, `MeshPart.TextureID`
5. Si le JSON `get` contient `scaleType` (textures **importées de Studio** uniquement) :
   - ImageLabel / ImageButton : `ScaleType = Enum.ScaleType.{scaleType}`
   - Si `scaleType` est `Tile` et `tileSize` est présent : `TileSize = UDim2.new(tileSize.xScale, tileSize.xOffset, tileSize.yScale, tileSize.yOffset)` (ex. `{0, 100},{0, 100}` → `UDim2.new(0, 100, 0, 100)`).
   - Les instances 3D (`Texture` / `Decal` / `MeshPart`) n’ont pas `ScaleType` : ne les copie pas.
6. N’invente jamais un ID. Ne génère une image que si la recherche texture est vide.

## 3D Blender (`meshProvider: blender`)

Si `status` indique `"meshProvider":"blender"` (Réglages Lumen), **ne passe pas par Meshy/Tripo**.

1. Écris un script Python **bpy** dans `assets/blender/nom.py`.
2. Primitives : cube, uv_sphere, ico_sphere, cylinder, cone, torus. Matériaux Principled (Base Color, Roughness, Metallic). Origine `(0,0,0)`, taille ~1 m, low-poly (budget 20k triangles).
3. `node tools/lumen-asset.mjs blender assets/blender/nom.py "Titre du prop"`
4. Lumen lance Blender en local, exporte un GLB, toast de validation, puis `rbxassetid`.
5. Si le toast refuse : corrige le script, relance. Pas de `subprocess`, pas de réseau dans le script.

Exemple minimal :

```python
import bpy
for obj in list(bpy.data.objects):
    bpy.data.objects.remove(obj, do_unlink=True)
mat = bpy.data.materials.new("Wood")
mat.use_nodes = True
bsdf = mat.node_tree.nodes.get("Principled BSDF")
bsdf.inputs["Base Color"].default_value = (0.55, 0.32, 0.14, 1)
bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, 0, 0.5))
bpy.context.active_object.data.materials.append(mat)
```
