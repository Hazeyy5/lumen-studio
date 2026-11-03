---
name: lumen-assets
description: Cherche la banque Lumen/VibeStarter/Textures pour un jeu Unreal Engine, récupère le fichier choisi dans assets/ puis l'importe dans l'éditeur par le MCP, génère image/mesh si besoin. À utiliser aussi pour s'inspirer d'une capture d'UI (codes INS-xxxx) ou pour des textures (TEX-xxxx).
---

# Assets Lumen (Unreal)

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

## Du fichier à Unreal

Ici, rien n'est publié sur Roblox. Ignore tout `rbxassetid`.

1. `get CODE` : l'utilisateur valide dans Lumen, puis Lumen copie le fichier dans `assets/images/`, `assets/meshes/` ou `assets/textures/`. La commande affiche le chemin. `image`, `mesh`, `icon` et `blender` affichent aussi leur fichier.
2. Importe-le toi-même avec le MCP de l'éditeur (`unreal-mcp`, éditeur ouvert) : `call_tool` avec `toolset_name` = `lumen_toolset.LumenTools`, `tool_name` = `import_file`, `arguments` = `{"source_file": "assets/meshes/SM_Coffre_VS_0124.glb", "folder_path": "/Game/Lumen/Meshes", "asset_name": "SM_Coffre_VS_0124"}`. Range dans `/Game/Lumen/Meshes`, `/Game/Lumen/Textures`, `/Game/Lumen/UI`.
   - `.png` : devient une texture (pour l'UI, réglages de compression UI / sans mipmaps).
   - `.glb` : devient un Static Mesh, avec ses matériaux et textures. N'utilise pas `StaticMeshTools.import_file`, qui refuse les `.glb`.
   - Éditeur fermé : dis à l'utilisateur d'ouvrir le projet (bouton Ouvrir dans Unreal de Lumen), ou de glisser le fichier dans le Content Browser.
3. En C++, ne mets pas de chemin en dur : expose une propriété (`UPROPERTY(EditAnywhere) TObjectPtr<UStaticMesh> Mesh;`, `TObjectPtr<UTexture2D> Icon;`) et règle-la sur le Blueprint par le MCP.

## Inspiration (captures d'UI)

`search inspiration …` (3 propositions, 1 choix), puis `get INS-xxxx` : l'image est copiée dans `assets/inspiration/`. Lis-la et reproduis l'esprit (layout, couleurs, rythme) en UMG. Ne l'importe pas.
