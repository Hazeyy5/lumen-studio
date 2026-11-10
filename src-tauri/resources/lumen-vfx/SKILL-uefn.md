---
name: lumen-vfx
description: Effets visuels d'une île UEFN (Fortnite, Verse) - impacts, explosions, auras, effets de collecte, avertissements au sol, pièges, ambiance, Niagara, VFX Spawner, VFX Creator, SpawnParticleSystem. À utiliser dès qu'il faut créer, placer, déclencher ou régler un effet.
---

# VFX (UEFN)

Principes de lecture et de rythme : `tools/knowledge/lihfolk/` (notes LihFolk, CC BY 4.0, exemples Roblox mais règles d'art valables partout). Lis les sections `## Common mistakes` et `## Checklist` de `VFX Design Playbook`, `VFX Readability and Value - Lessons from Blind Reviews`, `AoE Telegraphs - Readable Ground Warnings`, `Elemental VFX Kits - Fire, Ice, Earth, Water, Wind and Lightning`. (L'onglet VFX de Lumen est réservé aux projets Roblox.)

## Choisir l'outil (du plus simple au plus libre)

1. **VFX Spawner device** : effets tout faits de Fortnite (explosions, fumée, étincelles, feu…). À poser et régler dans la map. Verse : `vfx_spawner_device` → `Enable()`, `Disable()`, `Restart()` (relance un effet « Burst »), événements `EffectEnabledEvent` / `EffectDisabledEvent`.
2. **VFX Creator device** : effet personnalisé sans Niagara (forme, sprite, couleurs, taille, vitesse, durée, réglés dans le panneau Détails). Verse : `vfx_creator_device` → `Begin()` / `Begin(Agent)` / `BeginForAll()`, `End…`, `Toggle…`, `SpawnAt(Agent)` (sur un joueur), `Enable()`, `Disable()`.
3. **Système Niagara** du projet (le plus libre) : créé dans le Content Browser (Niagara System, à partir d'un modèle « Simple Sprite Burst », « Fountain »…), puis joué en Verse :
   ```verse
   using { /UnrealEngine.com/Assets }
   SpawnParticleSystem(Lumen.Vfx.NS_IceImpact, Position) # renvoie un cancelable : .Cancel() pour l'arrêter
   ```
   Le nom exact du module est dans `*-Assets.digest.verse` après un Build Verse Code. La création et le réglage fin d'un Niagara passent par l'éditeur : tu peux créer l'asset par `execute_python` (dupliquer un modèle) mais l'édition des modules est très limitée en Python. Si l'effet demande des réglages de modules, propose un VFX Creator, ou guide l'utilisateur pas à pas dans l'éditeur Niagara (quels modules, quelles valeurs).

Texture d'effet sur mesure (sprite de VFX Creator ou de Niagara) : génère un PNG blanc sur fond transparent (`node tools/lumen-asset.mjs image "…"`), puis `import_file`.

## Règles (communes à tous les moteurs)

- **Lisibilité** : une encre sombre saturée (jamais grise), le corps de l'élément, un cœur blanc fin, au plus une teinte d'accent. Vérifie sur sol clair et sol sombre.
- **Rythme** : l'image la plus chaude dure moins de 0,1 s ; coupe chaque phase. Étincelles dans la direction du coup.
- **Avertissement au sol** d'une attaque : contour sombre, fond, remplissage de progression, liseré clair ; durée = réaction (0,25 s) + temps de sortir de la zone ; dégâts sur le « pop ».
- **Feedback** : effet + son (skill `lumen-audio`) en moins de 100 ms après l'action du joueur.
- **Coût** : pas d'effet en boucle infini partout ; désactive les spawners hors de vue ou hors de la phase de jeu ; peu d'effets simultanés en multijoueur.
- N'invente pas de fonction Verse : vérifie dans `Fortnite.digest.verse` / `UnrealEngine.digest.verse` (grep). **Build** à 0 erreur avant de rendre la main.
