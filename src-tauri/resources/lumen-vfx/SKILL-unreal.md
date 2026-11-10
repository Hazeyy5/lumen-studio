---
name: lumen-vfx
description: Effets visuels d'un jeu Unreal Engine - Niagara (impacts, explosions, slashes, projectiles, auras, effets de collecte, avertissements au sol, météo), déclenchement en C++ et Blueprints, coût et lisibilité. À utiliser dès qu'il faut créer, régler, jouer ou optimiser un effet.
---

# VFX Niagara (Unreal Engine)

Principes de lecture et de rythme : `tools/knowledge/lihfolk/` (notes LihFolk, CC BY 4.0, exemples Roblox mais règles d'art valables partout). Lis les sections `## Common mistakes` et `## Checklist` de `VFX Design Playbook`, `VFX Readability and Value - Lessons from Blind Reviews`, `Slash VFX Recipes - Crescent and Ring`, `Projectile VFX - Muzzle, Travel and Impact`, `AoE Telegraphs - Readable Ground Warnings`, `VFX Performance Budgets and Culling`. (L'onglet VFX de Lumen est réservé aux projets Roblox.)

## Méthode

1. **Écris la table des couches** avant de toucher à l'éditeur : pour chaque émetteur, rôle (encre sombre, corps, cœur blanc, accent), sprite, couleur, taille, durée de vie, nombre de particules, rafale ou flux.
2. **Crée le système par le MCP** : `list_toolsets` puis `describe_toolset` sur le toolset Niagara d'Epic pour connaître ses outils (créer un système depuis un modèle, ajouter des émetteurs, régler les paramètres). À défaut, `lumen_toolset.LumenTools` / `execute_python` (module `unreal`) : dupliquer un système modèle (`/Niagara/DefaultAssets/Templates/…`) dans `/Game/Lumen/Vfx/NS_<Nom>`. Expose les réglages utiles en **User Parameters** (`User.Color`, `User.Scale`, `User.Direction`) pour les piloter depuis le code.
3. **Joue-le en C++** :
   ```cpp
   UPROPERTY(EditAnywhere, Category = "VFX") TObjectPtr<UNiagaraSystem> ImpactFx;
   UNiagaraComponent* Fx = UNiagaraFunctionLibrary::SpawnSystemAtLocation(this, ImpactFx, HitLocation, HitNormal.Rotation());
   if (Fx) Fx->SetVariableLinearColor(TEXT("User.Color"), FLinearColor(0.4f, 0.9f, 1.f));
   // Attaché (aura, traînée) :
   UNiagaraFunctionLibrary::SpawnSystemAttached(AuraFx, GetMesh(), NAME_None, FVector::ZeroVector, FRotator::ZeroRotator, EAttachLocation::SnapToTarget, true);
   ```
   `#include "NiagaraFunctionLibrary.h"` et `"NiagaraComponent.h"` ; ajoute `"Niagara"` aux `PublicDependencyModuleNames` du `.Build.cs` (changement de `.Build.cs` : éditeur fermé pour compiler). Renseigne les `UNiagaraSystem*` dans le Blueprint enfant par le MCP, jamais de chemin en dur.
4. **Vérifie** en Play in Editor sur un sol clair et un sol sombre, puis contre la checklist des notes.

## Règles

- **Lisibilité** : une encre sombre saturée (jamais grise), le corps de l'élément, un cœur blanc fin, au plus une teinte d'accent. Matériaux de particules non éclairés (Unlit) pour les effets lumineux, additifs avec parcimonie (ils blanchissent sur fond clair).
- **Rythme** : l'image la plus chaude dure moins de 0,1 s ; rafales (Spawn Burst Instantaneous) plutôt que flux pour un impact ; étincelles orientées dans la direction du coup.
- **Avertissement au sol** d'une attaque : décalque ou plan avec contour sombre, fond, remplissage de progression, liseré clair ; même forme pour le dessin et le test de dégâts côté serveur ; dégâts sur le « pop ».
- **Feedback** : effet + son (skill `lumen-audio`) en moins de 100 ms ; sur un coup, à l'impact (Anim Notify), pas au clic.
- **Multijoueur** : effets cosmétiques en local ; le serveur valide et réplique un événement (Multicast non fiable) avec des valeurs, chaque client spawn l'effet. Jamais d'acteur répliqué juste pour un effet.
- **Coût** : **Effect Type** avec scalabilité et budget (cull par distance et par nombre d'instances), pooling (`ENCPoolMethod::AutoRelease`) pour les effets fréquents, durée de vie bornée, pas de GPU sim inutile pour 20 particules.
- **Compile** (`node tools/lumen-unreal.mjs build`) à 0 erreur avant de rendre la main.
