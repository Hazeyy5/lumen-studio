---
name: lumen-audio
description: Son d'un jeu Unreal Engine (C++ / Blueprints) - bruitages (pièce, saut, clic, achat, coup, explosion, laser, pas, power-up, erreur…), feedback sonore, ambiances, musique, mixage, son 3D. À utiliser dès qu'il faut ajouter, générer, régler ou jouer un son.
---

# Son et bruitages (Unreal Engine)

Les bruitages se **génèrent** avec Lumen (aucun fichier à trouver, aucun droit à vérifier) : l'utilisateur les écoute dans l'onglet **Sons** de Lumen. Principes de sound design : `tools/knowledge/lihfolk/` (notes LihFolk, CC BY 4.0, exemples Roblox mais règles valables partout) : `grep -i "sound\|audio\|music\|mixing" tools/knowledge/lihfolk/INDEX.md`.

## Méthode

1. **Liste les sons utiles** : chaque action du joueur a son son. Noms en PascalCase par rôle : `CoinPickup`, `UiClick`, `PlayerHurt`.
2. **Génère des variantes** et fais choisir l'utilisateur :
   ```
   node tools/lumen-sfx.mjs presets
   node tools/lumen-sfx.mjs variants coin CoinPickup 4
   node tools/lumen-sfx.mjs keep CoinPickup_v3 CoinPickup
   ```
   Il écoute `CoinPickup_v1..v4` dans l'onglet Sons. Réglages : `--pitch 1.3`, `--length 0.7`, ou le JSON `assets/sfx/<Nom>.sfx.json` puis `render <Nom>`.
3. **Importe** par le MCP : `call_tool` `lumen_toolset.LumenTools` / `import_file` avec `source_file = assets/sfx/CoinPickup.wav`, `folder_path = /Game/Lumen/Sfx`, `asset_name = SFX_CoinPickup` (Sound Wave). L'utilisateur peut aussi cliquer « Importer dans le projet » dans l'onglet Sons. Ne lance jamais `upload` / `sync` : c'est pour Roblox.
4. **Joue-le** depuis le C++ :
   ```cpp
   UPROPERTY(EditAnywhere, Category = "Sons") TObjectPtr<USoundBase> CoinSound;
   // 2D (interface, sons du joueur lui-même), hauteur variée de ±6 % :
   UGameplayStatics::PlaySound2D(this, CoinSound, 1.f, FMath::FRandRange(0.94f, 1.06f));
   // 3D, à la position de la source :
   UGameplayStatics::PlaySoundAtLocation(this, ExplosionSound, GetActorLocation(), 1.f, FMath::FRandRange(0.94f, 1.06f), 0.f, Attenuation);
   ```
   `#include "Kismet/GameplayStatics.h"`. Renseigne les `USoundBase*` dans le Blueprint enfant (par le MCP), jamais un chemin d'asset en dur dans le code.
5. **Mixage** : crée une **Sound Class** `SC_Sfx` (et `SC_Music`) et assigne-la aux Sound Waves ; le volume des effets dans les options passe par un **Sound Mix**. Atténuation : un asset **Sound Attenuation** par catégorie (petits impacts 1500-3000 unités, explosions 8000+).
6. **Compile** (`node tools/lumen-unreal.mjs build`) à 0 erreur.

## Préréglages

`coin`, `pickup`, `powerup`, `levelup`, `jump`, `bounce`, `laser`, `shoot`, `explosion`, `hit`, `hurt`, `click`, `hover`, `confirm`, `error`, `whoosh`, `blip`, `footstep`, `cash`, `magic` (`node tools/lumen-sfx.mjs presets` pour la liste détaillée).

## Règles

- **Feedback** : un son en moins de 100 ms après chaque action. Un coup : le son part sur l'impact (Anim Notify), pas au clic.
- **Pas d'effet mitraillette** : hauteur variée à chaque lecture ; pour un son très répété, plusieurs variantes dans un **Sound Cue** (nœud Random) ou une **MetaSound**.
- **Sons du joueur lui-même** en 2D ; sources du monde en 3D avec atténuation.
- **Multijoueur** : le son de sa propre action est joué localement tout de suite ; celui des autres arrive par réplication (Multicast non fiable) ou suit un effet déjà répliqué.
- **Musique** : pas avec le générateur. Un fichier que l'utilisateur possède (`import_file`), jamais une chanson du commerce.
