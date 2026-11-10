---
name: lumen-audio
description: Son d'une île UEFN (Fortnite, Verse) - bruitages (pièce, saut, clic, achat, coup, explosion, laser, pas, power-up, erreur…), feedback sonore, ambiances, musique, mixage. À utiliser dès qu'il faut ajouter, générer, régler ou jouer un son.
---

# Son et bruitages (UEFN, Verse)

Les bruitages se **génèrent** avec Lumen (aucun fichier à trouver, aucun droit à vérifier) : l'utilisateur les écoute dans l'onglet **Sons** de Lumen. Principes de sound design : `tools/knowledge/lihfolk/` (notes LihFolk, CC BY 4.0, exemples Roblox mais règles valables partout) : `grep -i "sound\|audio\|music" tools/knowledge/lihfolk/INDEX.md`.

## Méthode

1. **Liste les sons utiles** : chaque action du joueur (collecte, achat, élimination, capture, erreur, niveau gagné) a son son. Noms en PascalCase par rôle : `CoinPickup`, `BuyConfirm`.
2. **Génère des variantes** et fais choisir l'utilisateur :
   ```
   node tools/lumen-sfx.mjs presets
   node tools/lumen-sfx.mjs variants coin CoinPickup 4
   node tools/lumen-sfx.mjs keep CoinPickup_v3 CoinPickup
   ```
   Il écoute `CoinPickup_v1..v4` dans l'onglet Sons. Réglages : `--pitch 1.3`, `--length 0.7`, ou le JSON `assets/sfx/<Nom>.sfx.json` puis `render <Nom>`.
3. **Importe** le WAV dans UEFN avec l'outil MCP `import_file` : `file = assets/sfx/CoinPickup.wav`, `destination = /<Projet>/Lumen/Sfx`. Ça crée un Sound Wave. (L'utilisateur peut aussi cliquer « Importer dans le projet » dans l'onglet Sons.) Ne lance jamais `upload` / `sync` : c'est pour Roblox.
4. **Joue-le** avec un **Audio Player device** :
   - pose un Audio Player (spawn_actor, ou demande à l'utilisateur de le poser) et règle son champ **Audio** sur le Sound Wave importé (set_actor_properties / execute_python) ;
   - réglages utiles du device : volume, « Play Location » (sur le device pour un son 3D, ou sur le joueur), qui l'entend (tous / le joueur déclencheur), atténuation ;
   - en Verse : `@editable CoinSound : audio_player_device = audio_player_device{}` puis `CoinSound.Play(Agent)` (pour ce joueur) ou `CoinSound.Play()` (pour tous). Aussi `Stop`, `Enable`, `Disable`. Relie le champ dans le panneau Détails (ou avec `link_verse_field`).
   - un Audio Player par son ; pour un son très répété, 2 ou 3 variantes gardées et alternées (pas d'effet mitraillette).
5. **Build** (`node tools/lumen-verse.mjs build`) à 0 erreur, puis dis à l'utilisateur de tester en session.

## Préréglages

`coin` pièce, `pickup` objet ramassé, `powerup` bonus, `levelup` niveau gagné, `jump` saut, `bounce` rebond, `laser` tir rétro, `shoot` coup de feu, `explosion`, `hit` coup, `hurt` dégât reçu, `click` clic d'interface, `hover` survol, `confirm` validation/achat, `error` refus, `whoosh` attaque, `blip` bip de dialogue, `footstep` pas, `cash` caisse, `magic` sort.

## Règles

- **Feedback** : un son en moins de 100 ms après chaque action. Récompenses plus longues et plus fortes que les sons d'interface.
- **Qui entend** : le son d'une action personnelle (achat, collecte) pour ce joueur seulement (`Play(Agent)`) ; un événement de partie (fin de manche, objectif capturé) pour tous.
- **Son 3D** : le device à l'endroit de la source, atténuation courte pour les petits sons, longue pour les explosions.
- **Musique** : le générateur ne fait pas de musique. Utilise le **Patchwork** / les pistes musicales de Fortnite, ou un fichier que l'utilisateur possède (importé de la même façon). Jamais une chanson du commerce.
- N'invente pas de fonction Verse : vérifie dans `Fortnite.digest.verse` (`grep -n "audio_player_device" …`).
