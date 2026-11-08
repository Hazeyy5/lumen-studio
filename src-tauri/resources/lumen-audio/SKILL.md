---
name: lumen-audio
description: Son d'un jeu Roblox (roblox-ts) - bruitages (pièce, saut, clic, achat, coup, explosion, laser, pas, power-up, erreur…), feedback sonore de l'interface et du gameplay, ambiances, musique, mixage et son 3D. À utiliser dès qu'il faut ajouter, générer, régler ou corriger un son.
---

# Son et bruitages (Roblox, roblox-ts)

Les bruitages se **génèrent** avec Lumen : aucun fichier à trouver, aucun droit à vérifier. L'utilisateur les écoute dans l'onglet **Sons** de Lumen. La connaissance de référence est dans `tools/knowledge/lihfolk/vfx/` (notes audio de LihFolk Vault V2, CC BY 4.0, en anglais) : cherche dans `tools/knowledge/lihfolk/INDEX.md` (`sound`, `audio`, `music`, `mixing`, `footstep`).

**N'invente jamais un id de son**, et ne reprends pas d'ids trouvés dans de vieux scripts ou sur internet (souvent supprimés ou sans droits).

## Méthode

1. **Liste les sons utiles** avant de coder : chaque action du joueur (clic, achat, collecte, saut, coup, dégât, niveau gagné, erreur) a son son. Nomme-les en PascalCase par rôle : `UiClick`, `BuyConfirm`, `CoinPickup`, `PlayerHurt`.
2. **Génère des variantes** et fais choisir l'utilisateur :
   ```
   node tools/lumen-sfx.mjs presets
   node tools/lumen-sfx.mjs variants coin CoinPickup 4
   ```
   Dis à l'utilisateur d'écouter `CoinPickup_v1` à `_v4` dans l'onglet Sons de Lumen, puis garde son choix : `node tools/lumen-sfx.mjs keep CoinPickup_v3 CoinPickup`. S'il te laisse choisir, `make coin CoinPickup` directement.
3. **Ajuste** si besoin : `--pitch 1.3` (plus aigu, plus léger) ou `--length 0.7` (plus sec) sur `make`/`variants`, ou modifie le JSON (`assets/sfx/<Nom>.sfx.json`) puis `node tools/lumen-sfx.mjs render <Nom>`. L'onglet Sons se met à jour tout seul.
4. **Publie** les sons gardés, puis génère le module :
   ```
   node tools/lumen-sfx.mjs upload
   node tools/lumen-sfx.mjs sync
   ```
   `upload` ne republie que les WAV nouveaux ou modifiés (jamais les variantes `_vN`). Roblox limite les uploads audio par mois (plus pour un compte vérifié) : publie seulement les sons choisis. Un son tout juste publié peut rester muet quelques minutes, le temps de la modération.
5. **Branche** dans le code (fichier généré, ne le modifie pas) :
   ```ts
   import { playSfx } from "shared/sfx";
   playSfx("UiClick");                              // interface : son 2D
   playSfx("CoinPickup", { pitch: 1 + combo * 0.05 }); // montée de hauteur sur un combo
   playSfx("Explosion", { parent: part, range: 120 }); // son 3D positionné
   ```

## Préréglages

`coin` pièce, `pickup` objet ramassé, `powerup` bonus, `levelup` niveau gagné, `jump` saut, `bounce` rebond, `laser` tir rétro, `shoot` coup de feu, `explosion`, `hit` coup, `hurt` dégât reçu, `click` clic d'interface, `hover` survol, `confirm` validation/achat, `error` refus, `whoosh` attaque à l'arme, `blip` bip de dialogue, `footstep` pas, `cash` caisse, `magic` sort.

Format d'un son (pour un réglage fin) : `{ "volume": 0.9, "echo": [délai s, retour 0..1, mix], "layers": [ { "wave": "square|saw|triangle|sine|noise", "start", "attack", "hold", "decay" (s), "punch" 0..1, "freq": [Hz début, Hz fin], "curve": "exp|lin", "vibrato": [demi-tons, Hz], "arp": [[t s, ×fréquence]], "duty": [début, fin], "lowpass": [Hz, Hz], "resonance", "highpass", "bits", "repeat", "volume" } ] }`.

## Règles

- **Feedback** : un son en moins de 100 ms après chaque action du joueur. Un coup : le son part sur l'impact (marqueur d'animation `Hit`), pas au clic. Le son « je touche » est joué tout de suite chez l'attaquant ; le son « je suis touché » seulement sur l'événement de dégâts du serveur.
- **Pas d'effet mitraillette** : `playSfx` varie déjà la hauteur de ±6 %. Pour un son très répété (pas, tirs), génère 2 ou 3 variantes gardées (`Footstep1`, `Footstep2`…) et alterne.
- **Hiérarchie** : les sons d'interface courts et discrets (`click`, `hover` au volume 0.4-0.6) ; les récompenses (`levelup`, `cash`) plus longues et plus fortes ; un critique = une note aiguë en plus de l'impact, pas juste plus fort.
- **Son 3D** : `parent` = la pièce qui émet. Distance par catégorie (pas, petits impacts 40-60 studs ; explosions 120+). Les sons du joueur lui-même (ses pas, son interface) en 2D.
- **Mixage** : tous les bruitages passent par le `SoundGroup` « Sfx » (volume 0.6) : un réglage de volume des effets dans les options ne touche que lui. Mets la musique dans un autre groupe, plus bas que les effets.
- **Musique et ambiances** : le générateur fait des bruitages, pas de la musique. Pour une musique, utilise un fichier que l'utilisateur possède (`node tools/lumen-asset.mjs upload assets/audio/theme.mp3 Theme`) ou demande-lui un id de la bibliothèque Roblox (sons sous licence Roblox). Jamais une chanson du commerce.

## Notes à connaître

`Sound Fundamentals`, `Sound Design for Games`, `Combat Sound Design and Feedback`, `Spatial Audio and Mixing`, `The Audio API - Players Emitters Wires and Effects`, `Ambient Soundscapes - Layer Stacks, Overlapping Zones and Voice Pools`, `Music and Adaptive Audio`, `Custom Character Sounds - Replacing Default Footstep and Jump Sounds`, `Music Licensing and Audio Upload Rules for 2026`.

Synchroniser un son avec une animation (marqueurs `Tele` / `Hit`) : skill `lumen-animation`. Son + effet visuel d'un même impact : skill `lumen-vfx`.
