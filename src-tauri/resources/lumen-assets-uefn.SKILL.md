---
name: lumen-assets
description: Cherche la banque Lumen/VibeStarter/Textures pour une île Fortnite (UEFN), récupère le fichier choisi dans assets/ pour l'importer dans UEFN, génère image/mesh si besoin. À utiliser aussi pour s'inspirer d'une capture d'UI (codes INS-xxxx) ou pour des textures (TEX-xxxx).
---

# Assets Lumen (UEFN)

Ne demande jamais les clés API. Elles sont dans Lumen (Réglages). Lumen doit être ouvert : tout passe par `127.0.0.1:17422`.

```bash
node tools/lumen-asset.mjs status
node tools/lumen-asset.mjs search mesh totem --for "totem décoratif au spawn, le joueur passe à côté"
node tools/lumen-asset.mjs search image coin --for "icône pièce dans le HUD"
node tools/lumen-asset.mjs search texture bouton --for "fond de bouton shop"
node tools/lumen-asset.mjs search inspiration hud --for "layout boutique cartoon"
node tools/lumen-asset.mjs propose VS-2608 VS-8911 --for "pavillon au bord de la rampe"
node tools/lumen-asset.mjs get VS-0124
node tools/lumen-asset.mjs image "icône pièce d'or, PNG fond transparent"
node tools/lumen-asset.mjs icon VS-0124
node tools/lumen-asset.mjs mesh "coffre low poly stylisé"
node tools/lumen-asset.mjs blender assets/blender/crate.py Crate
```

## Bibliothèque d'abord

1. `search <mesh|image|texture|inspiration> <2 à 4 mots> --for "où ça va / à quoi ça sert"`. `--for` est obligatoire : Lumen l'affiche sur le toast.
2. `search` attend : Lumen ouvre un menu d'environ 10 assets, l'utilisateur en choisit **un** (ou Aucune).
3. La commande affiche `Choisi : VS-xxxx`. Ensuite **un seul** `get` sur ce code. Ne get jamais les autres.
4. `Aucune sélection` : ne get rien. Propose d'autres mots ou génère.
5. Ne génère (`image` / `mesh` / `blender`) que si la recherche ne donne rien de pertinent, ou si l'utilisateur a cliqué Aucune. Avant `mesh`, lance `status` : si `meshProvider` vaut `blender`, écris un script bpy dans `assets/blender/` et lance `blender`.

## Du fichier à UEFN

Ici, rien n'est publié sur Roblox. Ignore tout `rbxassetid`.

1. `get CODE` : l'utilisateur valide dans Lumen, puis Lumen copie le fichier dans `assets/images/`, `assets/meshes/` ou `assets/textures/`. La commande affiche le chemin. `image`, `mesh`, `icon` et `blender` affichent aussi leur fichier.
2. Le fichier n'est pas encore dans UEFN. Dis à l'utilisateur de le glisser dans le Content Browser de UEFN, dans un dossier que tu nommes (ex. `Content/Lumen/Icons`).
   - `.png` : devient une texture.
   - `.glb` : s'importe comme Static Mesh. Si UEFN refuse, écris un script Blender qui exporte en `.fbx`.
3. Après l'import, lance `node tools/lumen-verse.mjs build` (UEFN ouvert). L'asset apparaît alors dans le digest `*-Assets.digest.verse` (chemin dans `AGENTS.md`) : un dossier de `Content/` devient un module. Exemple : `Lumen.Icons.T_coin_VS_0124 : texture`. Recopie le nom exact depuis le digest.
4. En Verse, une texture s'affiche avec `texture_block{DefaultImage := Lumen.Icons.T_coin_VS_0124}`. Un mesh se place dans la map, ou s'anime avec des devices (Prop Mover, etc.). Dis à l'utilisateur où le poser.

## Inspiration (captures d'UI)

`search inspiration …` (3 propositions, 1 choix), puis `get INS-xxxx` : l'image est copiée dans `assets/inspiration/`. Lis-la et reproduis l'esprit (layout, couleurs, rythme) en UI Verse. Ne l'importe pas dans UEFN.
