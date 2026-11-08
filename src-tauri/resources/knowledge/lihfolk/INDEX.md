# Index des notes LihFolk (VFX, éclairage, audio, animation)

Une ligne par note : chemin, puis son résumé (symptôme -> correction, checklist).
Cherche un mot-clé ici (Grep), puis lis seulement la section utile de la note (`grep -n "^## "`).

## vfx

### Ambient Soundscapes - Layer Stacks, Overlapping Zones and Voice Pools
`tools/knowledge/lihfolk/vfx/Ambient Soundscapes - Layer Stacks, Overlapping Zones and Voice Pools.md`
- Avoid: New player per bed per transition -> One player per asset, volume targets
- Avoid: No hysteresis -> Dwell time and larger exit bounds
- Avoid: `Instance.new` per one-shot -> Fixed voice pool with stealing
- Soundscapes defined as data: beds and one-shot sets with conditions.
- Overlap resolution by priority with hysteresis; polled at 4 to 10 Hz.
- One player per bed asset; equal-power crossfades; random start offsets.

### AoE Telegraphs - Readable Ground Warnings
`tools/knowledge/lihfolk/vfx/AoE Telegraphs - Readable Ground Warnings.md`
- Warning vanishes on bright floors: Only a bright rim -> Dark outline under the shape
- Warning vanishes on dark floors: Only a dark outline -> Bright rim inside the edge
- Players "dodged but got hit": Pop and damage on different frames, or different shape numbers -> One shared shape table; damage at the pop
- Four layers: dark outline, base fill, progress fill with a hot edge, bright rim.
- Duration covers 0.25 s reaction plus escape at walk speed; longer for bosses and touch.
- Fill eases in; pop flashes on the damage frame and fades in about 0.22 s.

### Atmosphere Sky Clouds and Post-Processing
`tools/knowledge/lihfolk/vfx/Atmosphere Sky Clouds and Post-Processing.md`
- Fog stopped working: An `Atmosphere` was added -> Use Atmosphere density and haze.
- Skybox visible through mountains: `Offset` too low for the `Density` -> Raise `Offset`.
- Glare or Decay does nothing: `Haze` is 0 (and Glare 0 for Decay) -> Raise Haze first.
- Atmosphere or fog, not both.
- Density and Offset tuned together; no ghosting at the horizon.
- Clouds under Terrain, moved by `GlobalWind`.

### Auras, Shields and Status Effects
`tools/knowledge/lihfolk/vfx/Auras, Shields and Status Effects.md`
- Character tinted by its own aura: Highlight fill or a big translucent shell over the body -> `FillTransparency` 1; rims and wisps around the body
- Shins glow in the aura colour: Effect light at foot height -> Light off, or above and behind at low power
- Auras cost frame time across the map: Streams never sleep -> Sleep beyond 160 studs, wake inside 136
- Persistent layers at 3-8 per second; about 15 particles alive per aura.
- Everything on Attachments or adornments; nothing welded to the body.
- Rim with an outline-only Highlight; no fill, no wash; light off unless asked.

### Beams and Trails
`tools/knowledge/lihfolk/vfx/Beams and Trails.md`
- Solid coloured slab instead of the texture: Texture not loaded (or invalid id) -> Preload; new uploads need about a minute.
- Beam missing: One attachment missing, same attachment twice, or not in Workspace -> Two distinct attachments in Workspace.
- Gradient looks stepped: Too few segments for the keypoints -> Segments at least keypoints - 1, more for curves.
- All beam and trail textures preloaded with the owning item.
- Arcs built with 4/3 r handles and enough segments.
- VFX ribbons at LightInfluence 0 with appropriate LightEmission.

### Collectible Pickup FX - Idle Glint, Magnet Pull and Collect Pop
`tools/knowledge/lihfolk/vfx/Collectible Pickup FX - Idle Glint, Magnet Pull and Collect Pop.md`
- Avoid: Server-side `Touched` coins. Exploitable and expensive.
- Avoid: One tween per coin. Hundreds of tweens; one shared loop is cheaper.
- Avoid: Constant-fraction lerp magnet. Feels sticky at the end.
- Server owns records and validates batched requests by distance and rate.
- Per-player pickups are never server instances.
- One shared animation loop with phase offsets; glint via occasional `Emit`.

### Combat Sound Design and Feedback
`tools/knowledge/lihfolk/vfx/Combat Sound Design and Feedback.md`
- Hits feel mushy online, fine in Studio: Impact played on server confirmation -> Predict the neutral contact on the attacker's client
- False "you got hit": Hurt sound predicted -> Hurt only on server damage events
- Crits do not feel special: Crit is just a louder hit -> A tonal sting above the impact's range
- Outcome matrix written: each outcome distinct in spectrum and envelope.
- Neutral contact predicted on the attacker; meaning waits for the server.
- Kill and hurt confirms 2D and personal; never predicted.

### Custom Character Sounds - Replacing Default Footstep and Jump Sounds
`tools/knowledge/lihfolk/vfx/Custom Character Sounds - Replacing Default Footstep and Jump Sounds.md`
- Avoid: Adding sounds to the rig without disabling the default -> Replace `RbxCharacterSounds`
- Avoid: Playing footsteps on the server -> Client-side for all characters
- Avoid: Speed-based loop for steps -> Animation markers
- `RbxCharacterSounds` replaced (disabled or forked, fork dated).
- Footstep markers on every locomotion animation.
- Surface from raycast with attribute overrides; Terrain covered.

### Designing VFX Textures and Flipbooks in Figma
`tools/knowledge/lihfolk/vfx/Designing VFX Textures and Flipbooks in Figma.md`
- Avoid: Painting textures in their final colours, then fighting the emitter's `Color`.
- Avoid: Gradients that fade to transparent *black*, giving grey or dark edge rings.
- Avoid: Flipbook sheets that are not 1024 x 1024, or exported with trim on.
- Art is white and grey with alpha; colour comes from the palette roles (main, hot, dark, glow, spark, debris, deep).
- Flipbooks are 1024 x 1024 frames with a 2x2, 4x4 or 8x8 grid, one clipped frame per cell, in play order.
- Every cell keeps a transparent margin (16 / 12 / 8 px for 512 / 256 / 128 cells); edges fade before it.

### Elemental VFX Kits - Fire, Ice, Earth, Water, Wind and Lightning
`tools/knowledge/lihfolk/vfx/Elemental VFX Kits - Fire, Ice, Earth, Water, Wind and Lightning.md`
- Avoid: Colour as the only identifier. Red fire and red blood, blue ice and blue lightning collide.
- Avoid: Ice that fades in. It reads as mist or magic, not ice.
- Avoid: Wind with a dark smoke layer. Looks like fire smoke.
- Palette table written per element, tested on your brightest and darkest map.
- Each element has a distinct motion signature visible in greyscale.
- Slashes and swing arcs spawn and emit only in the frontal arc; novas and auras may be radial.

### Engine and Tyre Audio Driven by RPM and Slip
`tools/knowledge/lihfolk/vfx/Engine and Tyre Audio Driven by RPM and Slip.md`
- Avoid: One loop pitched over the whole RPM range -> 3 to 4 loops with windowed crossfades
- Avoid: RPM proportional to speed -> Gear model or banded sawtooth
- Avoid: Ignoring load -> On-load and off-load sets
- Vehicle model exposes rpm, load, slip, grounded and surface per frame.
- Engine uses windowed RPM layers with on/off-load sets and an idle loop.
- Tyre skid, rolling-by-surface, landing and wind layers in place.

### Escalation and Grandeur Without Clutter
`tools/knowledge/lihfolk/vfx/Escalation and Grandeur Without Clutter.md`
- Move looks busy, not grand: Identical layer copied per beat -> Each beat distinct and escalating; later layers shorter
- Score drops after adding "grandeur": Vortex, rune circle, smoke and rings added together bury the... -> Escalate a few layers; cut or shrink one for each added; check...
- Weak ending: Finisher no bigger than the spins, blade low or pointing down -> One bigger, brighter forward arc on the contact frame, blade high...
- Every repeat bigger, brighter, faster and visibly different from the last
- Each added layer paid for by cutting or shrinking another; silhouette checked at game distance after each
- At most one growing environmental element (vortex, sigil, cracks, aura), first to cut

### Game Feel and Juice
`tools/knowledge/lihfolk/vfx/Game Feel and Juice.md`
- Game feels unresponsive: No immediate feedback on input -> Pop plus sound on every input within 100 ms.
- Rewards feel flat: Numbers snap -> Motion to the counter, count-up, pop.
- Juice on whiffs: Feedback scheduled at click time -> Fire on the hit marker.
- Every input: pop and sound.
- Every reward: motion, number, payoff pop.
- Big moments: anticipation then reveal.

### Gore Policy and Kid-Friendly Hit Feedback
`tools/knowledge/lihfolk/vfx/Gore Policy and Kid-Friendly Hit Feedback.md`
- Avoid: Answering the questionnaire for the default setting instead of the maximum content the game can show.
- Avoid: Persistent blood decals in a Minimal-label game.
- Avoid: Red as the default hit colour. Easy to read as blood; pick a non-blood hit colour unless blood is intended.
- Label target chosen before art; VFX style guide states "stylised" or "blood".
- Questionnaire answered for the most extreme content reachable, including settings and modes.
- Default hit feedback uses non-blood colours and no persistent marks (for Minimal or Mild targets).

### Highlights and Selection Effects
`tools/knowledge/lihfolk/vfx/Highlights and Selection Effects.md`
- New highlights do not appear: Slot limit reached, often by disabled highlights -> Destroy unused highlights.
- Outline stopped showing after a hit flash: A leftover Highlight on the same model -> One arbiter per object, or destroy flashes.
- Frame spike on every flash: Creating and destroying Highlights -> Animate transparency of a persistent one.
- Every Highlight sets fill, outline and depth mode explicitly.
- One owner per highlighted object (arbiter) or one container Highlight per class.
- Flashes animate a persistent Highlight; unused Highlights destroyed, not disabled.

### Ice Ball Cast to Explosion - Approved Recipe
`tools/knowledge/lihfolk/vfx/Ice Ball Cast to Explosion - Approved Recipe.md`
- Avoid: Wide, static camera for a hand-scale effect: the charge becomes invisible.
- Avoid: Flash on the impact frame hiding the landing.
- Avoid: Thin shard spikes and flat disc rings: they read as glass and water, not ice.
- Markers `ChargeStart`, `Release`, `Impact` on the clip
- Charge readable at game camera distance (test zoomed out too)
- Projectile and explosion stay in front of the caster

### Impact Decals and Material-Aware Hit Sparks
`tools/knowledge/lihfolk/vfx/Impact Decals and Material-Aware Hit Sparks.md`
- Avoid: Expecting `MaterialVariant` names from `RaycastResult.Material`.
- Avoid: Creating a part and decal per shot, never cleaning up.
- Avoid: Decals on unanchored parts left in world space.
- Resolution order: attribute, variant, material, default.
- Profile table in config; every common map material covered.
- Sparks oriented by normal plus reflection; one pooled emitter per profile.

### Light Sources and Shadows
`tools/knowledge/lihfolk/vfx/Light Sources and Shadows.md`
- Light does not reach far enough: Range too low; Brightness does not extend it -> Raise Range (max 120).
- Light seems not to exist: Parented to something that is not an Attachment or BasePart, or... -> Parent correctly.
- Lamp casts a black shadow from its own shade: Light inside geometry with Shadows on -> Move it out, or CastShadow off on the shade.
- Each light has a job; decoration lights have Shadows off.
- Shadow-casting lights limited to a few hero lights per view.
- VFX parts have CastShadow off; VFX lights have Shadows off and expire.

### Lighting Mood and Time of Day
`tools/knowledge/lihfolk/vfx/Lighting Mood and Time of Day.md`
- Time differs between players: Each client ran its own timer from join -> Derive from server time and a published epoch.
- Night looks like dim day: Only ClockTime changes -> Mood keyframes for ambient, shifts, atmosphere.
- Night unplayable on phones: Ambient near black -> Tinted ambient, outlines on enemies.
- Server publishes epoch, cycle and day share; clients compute time.
- Mood keyframes for at least dawn, noon, dusk, midnight.
- Night readable on a dim phone screen.

### Lighting Technologies and Settings
`tools/knowledge/lihfolk/vfx/Lighting Technologies and Settings.md`
- `Lighting.Technology` set by script errors: `Technology` deprecated, not scriptable -> Studio: Future=Realistic+Prioritize on, Voxel=Soft+off
- Fog settings do nothing: An `Atmosphere` is in `Lighting` (fog properties are hidden and... -> Use `Atmosphere.Density`/`Haze` instead, or remove the Atmosphere.
- Indoors is as bright as outdoors: `GlobalShadows` off, or `Ambient` channels above `OutdoorAmbient` -> Turn shadows on; keep `Ambient` at or below `OutdoorAmbient` per...
- Old `Technology` mapped: Future = Realistic + Prioritize on, ShadowMap = Soft + on, Voxel = Soft + off.
- `LightingStyle` and `PrioritizeLightingQuality` chosen deliberately, not inherited from a template.
- `GlobalShadows` on unless the art style needs it off; interiors sealed and thick enough.

### Lightning Strikes and Thunder - Procedural Bolts and Sound Delay
`tools/knowledge/lihfolk/vfx/Lightning Strikes and Thunder - Procedural Bolts and Sound Delay.md`
- Avoid: Creating parts per strike -> Pool segment parts or beams
- Avoid: Each client randomises the bolt -> Seed from the server event
- Avoid: Bolt visible half a second -> Total under ~0.3 s with flicker
- Server sends `{seed, position, strikeAt}`; clients render identical bolts.
- Segment parts or beams pooled; segment count capped.
- Strike sequence under ~0.3 s with a weaker second stroke.

### Music and Adaptive Audio
`tools/knowledge/lihfolk/vfx/Music and Adaptive Audio.md`
- Hard cut between tracks: One player, swap asset -> Two decks, crossfade before the end.
- Background restarts after every shop visit: Stop resets / new track each time -> Pause at silence and resume.
- Stems drift out of sync: Started one by one -> `Play(atTime)` on mixer time.
- Every track licensed or owned; uploads within limits.
- Two-deck crossfades; fade-in on join; silences between tracks.
- Context switches with priorities; pause and resume where sensible.

### Music Licensing and Audio Upload Rules for 2026
`tools/knowledge/lihfolk/vfx/Music Licensing and Audio Upload Rules for 2026.md`
- Avoid: Uploading commercial songs -> Only owned, commissioned or library audio
- Avoid: Third-party ids from old scripts -> Re-upload with rights, or replace
- Avoid: Personal uploads in a group game -> Upload to the group or grant permission
- Every audio asset has source, licence and uploader in the manifest.
- Music uploaded by the group that owns the game, or granted explicitly.
- Periodic load check for music ids, with fallbacks per context.

### Particle Emitters In Depth
`tools/knowledge/lihfolk/vfx/Particle Emitters In Depth.md`
- Effect dark in caves, grey in shade: `LightInfluence` 1 (Studio insert default) -> Set 0 explicitly.
- Emitter built in code looks different from the Studio template: Different `LightInfluence` defaults between insert paths -> Set every property explicitly in code or...
- Sphere/Cylinder shape looks wrong: Emitter on an Attachment -> Put it on a part.
- `LightInfluence` set explicitly on every VFX emitter (usually 0).
- Brightness chosen by screen area; one dark contrast layer per bright stack.
- Bursts via `Emit` with your own quality scaling; streams capped.

### Projectile VFX - Muzzle, Travel and Impact
`tools/knowledge/lihfolk/vfx/Projectile VFX - Muzzle, Travel and Impact.md`
- Shot reads as a dot: Trail too short for its speed -> Lifetime so that speed x lifetime is 12-30 studs
- Shot reads as a laser: Trail too long, or no taper -> `WidthScale` 1 to 0, transparency rising to 1
- Flat, one-colour core: No hot centre or dark layer -> Hot inner ball plus a dark wake
- Muzzle 1-2 studs ahead of the hand, flash about 0.6x the impact flash, sparks forward.
- Core has a hot centre and a body colour; dark elements use a rim shell.
- Trail streak is 12-30 studs (speed x lifetime) and tapers.

### Rain - Camera-Local Particles, Splashes and Wet Surfaces
`tools/knowledge/lihfolk/vfx/Rain - Camera-Local Particles, Splashes and Wet Surfaces.md`
- Avoid: One emitter covering the whole map -> Camera-local slab
- Avoid: `LockedToPart = true` -> Leave unlocked
- Avoid: Host lerped to the camera -> Snap to camera each frame
- Rain host is client-only, anchored, non-colliding, snapped above the camera.
- `Rate`, sound volume and wetness all driven by one intensity value.
- Near and far layers; streaks use `VelocityParallel` and `Squash`.

### Replicating VFX - Server Validates, Clients Render
`tools/knowledge/lihfolk/vfx/Replicating VFX - Server Validates, Clients Render.md`
- Abilities feel laggy for the caster: Caster waits for the server echo -> Play on input; the server skips the caster
- Caster sees every effect twice: Echo not skipped -> Skip the caster on the server; ignore own client-castable presets...
- Network spikes in big fights: Effects built on the server, or tables of options in each payload -> Ids and numbers only; clients build
- Caster plays on input; the server never echoes to the caster.
- Payload is `(caster, presetId, cf, elementIndex, seed, aux)`; lists are append-only.
- Server checks shape, allowed list, finite values, alive, range (clamp), per-preset cooldown and a token bucket.

### Screen Damage Effects - Hit Flash, Low-Health Vignette and Blur
`tools/knowledge/lihfolk/vfx/Screen Damage Effects - Hit Flash, Low-Health Vignette and Blur.md`
- Avoid: Several scripts tweening the same `ColorCorrectionEffect`.
- Avoid: Effects parented to `Lighting` in a game that also uses lighting zones.
- Avoid: Uncapped stacking. Shotguns and DoT ticks white out the screen.
- One client module owns every screen effect instance.
- Hits are pulses; low health is a level; both capped.
- Damage effects driven by server-confirmed events, with source position for a directional indicator.

### Slash VFX Recipes - Crescent and Ring
`tools/knowledge/lihfolk/vfx/Slash VFX Recipes - Crescent and Ring.md`
- The ring reads as a flat glowing plate: Hold with no motion; extra concentric ground rings -> Keep it drifting (about 110 deg over the erode); one faint ground...
- Dark smear invisible: The smear sits inside the body's inner edge or under the body, so... -> Overlap it with the body's inner half (scale about 1.0) and draw...
- Hot edge disappears after the sweep: The edge ribbon sits below a mesh with a raised outer lip -> Lift the ribbon onto the lip (about 0.15 x radius for a dished ring)
- Slash arcs stay in front (+-100 deg of the LookVector); no rings or spins
- Three tones present in every frame: white edge, element body, dark smear
- Sweep 3 to 5 frames, hold 1 to 2, erode 10 to 15 (at 30 fps)

### Snow, Sandstorm, Blizzard and Ash Recipes
`tools/knowledge/lihfolk/vfx/Snow, Sandstorm, Blizzard and Ash Recipes.md`
- Avoid: Storm sold only by particles -> Push Atmosphere, clouds and grade first
- Avoid: Sand colour with low Haze -> Raise Haze so `Color` shows
- Avoid: Snow host too small -> Wider box, offset upwind
- Each storm has Atmosphere, clouds, wind, grade, particles and sound in one look table.
- Air values chosen so the storm reads with particles off.
- Hosts sized for lifetime times wind drift; offset upwind.

### Sound Design for Games
`tools/knowledge/lihfolk/vfx/Sound Design for Games.md`
- Hits feel late: Sound started on the beat, peak later -> Start at beat minus peak.
- Machine-gun repetition: One take, no variation -> Bags, pitch variance, layer recombination.
- Big creature roar sounds slow and blurry: Pitched down with PlaybackSpeed -> Pitch shifter or a lower take.
- Each core action layered (transient, body, air, sweetener).
- Peaks aligned to visual beats from measured data.
- Variation by bags, pitch and recombination; choke on rapid repeats.

### Sound Fundamentals
`tools/knowledge/lihfolk/vfx/Sound Fundamentals.md`
- Sound silent in the live game only: Experience lacks permission to the asset -> Upload under the experience owner; test live.
- First impact sound late: Not preloaded -> Preload with the owner.
- Mix wildly uneven: Volumes set by ear per file -> Measure and level.
- Assets uploaded under the experience owner and verified in a live server.
- Forbidden or removed ids listed.
- Bus tree with settings sliders wired to buses.

### Spatial Audio and Mixing
`tools/knowledge/lihfolk/vfx/Spatial Audio and Mixing.md`
- Distant fights audible everywhere: Default 10,000-stud max distance -> Per-category distances.
- Sound cuts off abruptly: Inverse/Linear with a low max distance -> InverseTapered or a custom curve.
- Own footsteps sound far away: Listener at the third-person camera -> 2D own sounds or a closer listener.
- Attenuation set per category; no defaults left on positional sounds.
- Listener position chosen deliberately for the camera style.
- Every sound measured and levelled to a category target.

### Stylized Lighting Recipes
`tools/knowledge/lihfolk/vfx/Stylized Lighting Recipes.md`
- Looks great on PC, flat on phone: Look depends on shadows; phones below quality 4 have none -> Contrast in colour shifts and ambient tint
- Night unplayable: Ambient lowered instead of hue shifted -> Blue hue, less saturation, ambient around a third
- White walls glow: Bloom threshold under 1 -> Threshold 1.2 to 2; let effects exceed it
- `LightingStyle` and material style chosen before tuning.
- Each zone has a named recipe stored as data.
- Greyscale test passed: characters, enemies, pickups and paths are distinct values.

### The Audio API - Players Emitters Wires and Effects
`tools/knowledge/lihfolk/vfx/The Audio API - Players Emitters Wires and Effects.md`
- 3D sound silent: No listener (default location `None` or no voice), or listener not... -> Create and wire a listener.
- Emitter silent for one listener only: Different `AudioInteractionGroup` -> Match groups.
- Wire does nothing: Pin name wrong, missing instance or a cycle (`Connected` false) -> Check `Wire.Connected`.
- Graph drawn before code: producers, buses, master chain, output.
- Listener strategy chosen (default or custom per category).
- Interaction groups consistent between emitters and listeners.

### VFX Design Playbook
`tools/knowledge/lihfolk/vfx/VFX Design Playbook.md`
- Effect is a white blob: All additive, no dark layer, big sprites too bright -> Dark contrast layer; brightness by area.
- Effect feels slow or floaty: Phases too long -> Cut every phase; hot frame under 0.1 s.
- Sparks look random: Radial sparks on a directional hit -> Sparks along the swing direction.
- Layer table written before building; six phases considered.
- White core, colour edge, dark rim; one dark layer per bright stack.
- Tier ladder written and audited.

### VFX Performance Budgets and Culling
`tools/knowledge/lihfolk/vfx/VFX Performance Budgets and Culling.md`
- Fine alone, lags with a full server: No onlooker share or global cap -> Governor with a global cap; onlookers at half or less
- Hitch every few seconds in long fights: Cloning and destroying per hit -> Pools with a cap
- Phones lag only when an effect is near the camera: Overdraw from big transparent pieces -> Shrink, raise transparency on the camera line, fewer stacked layers
- Budgets written per cast, per caster window and per client, with onlooker share and distances.
- Cost estimation script run on every template; worst offenders listed.
- Gameplay effects flagged; never skipped, only simplified.

### VFX Previews in Blender for Roblox - Colour and Framing
`tools/knowledge/lihfolk/vfx/VFX Previews in Blender for Roblox - Colour and Framing.md`
- Avoid: Feeding Roblox `Color3` sRGB numbers into Blender as linear values.
- Avoid: Rendering with the AgX or Filmic view transform.
- Avoid: Close-up cinematic framing instead of the game camera.
- sRGB -> linear conversion on every colour input
- View transform set to Standard
- Camera at game distance; floor at ground height; neutral stage

### VFX Readability and Value - Lessons from Blind Reviews
`tools/knowledge/lihfolk/vfx/VFX Readability and Value - Lessons from Blind Reviews.md`
- Effect looks muddy: Grey or desaturated dark layer -> Saturated blue-black / deep element ink, never grey
- Effect looks flat or monochrome: No dark value layer, one hue only -> Deep ink layer + thin white-hot core + one accent hue
- Effect looks washed out or white: Wide white body, many additive layers -> White only as a thin core; fewer bright layers
- Every frame has dark ink, element body, thin white core and at most one accent hue
- Dark ink saturated (no grey)
- No two repeats identical; later layers shorter; stack thin

### Weather System Architecture - State Machine, Transitions and Replication
`tools/knowledge/lihfolk/vfx/Weather System Architecture - State Machine, Transitions and Replication.md`
- Avoid: Server tweens `Lighting` -> Server writes attributes, client interpolates
- Avoid: `math.random` per server -> Seeded walk from a fixed epoch
- Avoid: `WeatherStart = now` -> Use the slot boundary time
- Weather decided only on the server; clients only render.
- Four attributes, `WeatherState` written last as the commit.
- Alpha computed from `GetServerTimeNow()`; late joiners land mid-blend correctly.

## animation

### Animation Asset Permissions and Groups
`tools/knowledge/lihfolk/animation/01 Fundamentals/Animation Asset Permissions and Groups.md`
- Avoid: Staging universe with no grants. Symptom: everything frozen in staging while production works.
- Avoid: Uploading clips the morning of a release.
- Avoid: Personal uploads by contractors. Symptom: clips stop being fixable when the contract ends.
- Every clip id in the manifest is owned by the experience owner or explicitly granted.
- No temporary (registered) ids anywhere in scripts or data.
- No ids copied from free models without a grant.

### Animation Objects
`tools/knowledge/lihfolk/animation/01 Fundamentals/Animation Objects.md`
- Avoid: `Humanoid:LoadAnimation` in new code -> `humanoid:FindFirstChildOfClass("Animator"):LoadAnimation(anim)`
- Avoid: Creating an Animator on the client for a character -> Use the server-created Animator
- Avoid: Loading the same Animation every time you play it -> Cache one track per Animator per clip

### AnimationConstraint and Physically Simulated Characters
`tools/knowledge/lihfolk/animation/01 Fundamentals/AnimationConstraint and Physically Simulated Characters.md`
- Avoid: Testing only in a baseplate with an old dummy.
- Avoid: Leaving `IsKinematic = false` after a get-up.
- Avoid: Ramping `MaxTorque` on the server for a player's character.
- Every joint lookup accepts AnimationConstraint and Motor6D.
- No per-frame C0 writes on characters; procedural layers write `Transform` in `PreSimulation` and skip throttled frames.
- No server-side pose writes; inputs replicate, clients compute.

### AnimationTrack Reference
`tools/knowledge/lihfolk/animation/01 Fundamentals/AnimationTrack Reference.md`
- Avoid: `Play()` every frame -> Check `IsPlaying`
- Avoid: Setting `TimePosition` before `Play` -> Play first
- Avoid: Using `Length` right after loading -> Wait, preload, or use data

### Animator Deep Dive
`tools/knowledge/lihfolk/animation/01 Fundamentals/Animator Deep Dive.md`
- Joints are matched by name. A Pose (or curve folder) binds to the joint whose `Part1` has that name (for Bones, the bone's name).
- The top-level pose is never applied. The first Pose under each Keyframe (`HumanoidRootPart`, or a bone rig's `Root`) does nothing.
- Translation offsets scale with `Model:GetScale()`.
- `track.Length` stays 0: the asset never loaded.

### Bones and Skinned Meshes
`tools/knowledge/lihfolk/animation/01 Fundamentals/Bones and Skinned Meshes.md`
- Avoid: Root motion on the `Root` bone -> Key the root's child
- Avoid: Using `Bone.WorldCFrame` for animated position -> `TransformedWorldCFrame`
- Avoid: Relying on MeshPart collision for animated limbs -> Hitbox parts on bones, or data-driven hits

### CurveAnimation and Curve Channels
`tools/knowledge/lihfolk/animation/01 Fundamentals/CurveAnimation and Curve Channels.md`
- Avoid: Calling `X()` on a read-only inspection pass (Creates empty FloatCurves inside clips you meant only to read)
- Avoid: Setting `RightTangent` on a Linear key (Runtime error)
- Avoid: Assuming tangents are per frame (Overshoot 30x or 60x too large or small; test the unit)

### Frame Order and RunService Timing
`tools/knowledge/lihfolk/animation/01 Fundamentals/Frame Order and RunService Timing.md`
- Avoid: Procedural `Transform` write in `PreRender` / `RenderStepped` -> PreSimulation
- Avoid: Writing in `Heartbeat` -> PreSimulation
- Avoid: Composing onto unkeyed joints -> Assign from your own base

### How the Animator Really Behaves
`tools/knowledge/lihfolk/animation/01 Fundamentals/How the Animator Really Behaves.md`
- Per joint, only Poses with `Weight > 0` are keys.
- The top-level Pose of each Keyframe (HumanoidRootPart on characters, the `Root` bone on bone rigs) is never applied.
- Keys bind by name to joints (Part1 name, or Bone name).
- CurveAnimation playback against `RotationCurve` / `FloatCurve` sampling.

### Humanoid States and Animation
`tools/knowledge/lihfolk/animation/01 Fundamentals/Humanoid States and Animation.md`
- Avoid: Treating `Jumping(false)` as landing -> `StateChanged` to Landed
- Avoid: Choosing walk vs idle by state -> Speed thresholds with hysteresis
- Avoid: `SetStateEnabled` on the server for a player -> Run it on the client

### Keyframes, Easing and Interpolation
`tools/knowledge/lihfolk/animation/01 Fundamentals/Keyframes, Easing and Interpolation.md`
- Avoid: Easing set on the arriving key -> Put it on the leaving key
- Avoid: Legacy `Cubic` -> `CubicV2`
- Avoid: Key-to-key turns near 180 deg -> Mid keys; stay under 120

### KeyframeSequence Keyframe and Pose Reference
`tools/knowledge/lihfolk/animation/01 Fundamentals/KeyframeSequence Keyframe and Pose Reference.md`
- Top-level pose is `HumanoidRootPart` (or the rig's root) at Weight 0; motion starts on its child.
- Every Pose name matches a Part1 (or Bone) name on the target rig.
- Placeholders are Weight 0; real keys are Weight 1.
- Easing written on the key that starts each segment; no legacy `Cubic`.
- No key-to-key turn near 180 degrees; ideally under 120.
- Markers carry gameplay events; Keyframe names left as "Keyframe" unless a legacy listener needs them.

### Markers and Events
`tools/knowledge/lihfolk/animation/01 Fundamentals/Markers and Events.md`
- Avoid: Parry window built from client markers. Symptom: parry feels inconsistent across devices and is exploitable.
- Avoid: Footstep markers on every far character.
- Avoid: Renamed marker in one clip. Symptom: one weapon's hits have no sparks.
- Every gameplay marker also exists in a generated data table; the server reads the table.
- Listeners connect once per track and disconnect on `Ended`.
- Upper-body copies are marked so cosmetic listeners skip them.

### Motor6D Math and CFrame Composition
`tools/knowledge/lihfolk/animation/01 Fundamentals/Motor6D Math and CFrame Composition.md`
- Avoid: Server `C0` writes for aiming. Symptom: high network traffic and laggy aim seen by others.
- Avoid: Assuming R15 axes on R6 or imported rigs.
- Avoid: World-space joint math in huge maps. Symptom: limb jitter far from the origin.

### Playback, Priorities and Blending
`tools/knowledge/lihfolk/animation/01 Fundamentals/Playback, Priorities and Blending.md`
- Priority works per joint. A track only competes on the joints it keys (Poses with Weight > 0).
- The docs say equal priorities "blend by weight".
- `Play(fadeTime)`: weight ramps linearly from its current value to the target over `fadeTime`.
- `Play(_, _, speed)` or `AdjustSpeed(s)`: duration becomes `Length / speed`.

### Replication and Ownership
`tools/knowledge/lihfolk/animation/01 Fundamentals/Replication and Ownership.md`
- All replication of playing tracks goes through the Animator.
- If the Animator is a descendant of a player's character (under its Humanoid or an AnimationController in it), tracks started...
- If the Animator is not under a player's character, tracks must be loaded and started on the server to replicate.
- Remote viewers see the same clip a little later, by roughly the owner-to-server-to-viewer latency.

### Rigs and Joints
`tools/knowledge/lihfolk/animation/01 Fundamentals/Rigs and Joints.md`
- Avoid: Pose named after the joint (`RightShoulder`) -> Name after Part1 (`RightUpperArm`)
- Avoid: `IsA("Motor6D")` only -> Accept AnimationConstraint
- Avoid: Animating `C0` every frame -> Write `Transform` in PreSimulation

### Scale and Animation
`tools/knowledge/lihfolk/animation/01 Fundamentals/Scale and Animation.md`
- Avoid: Hard-coded hip height. Symptom: custom avatars float or sink after a ground-fit or IK pass.
- Avoid: Camera offsets tuned on one avatar. Symptom: tall avatars get the camera inside their head in cinematics.
- Avoid: Growth mechanic with cached locomotion rate.
- Know whether your characters are body-scaled or `ScaleTo`-scaled; author translations to match.
- Root drops and lifts tested at the real in-game size.
- Locomotion rate divides by authored ground speed times scale.

### The Default Animate Script
`tools/knowledge/lihfolk/animation/01 Fundamentals/The Default Animate Script.md`
- Avoid: Setting ids before `CharacterAppearanceLoaded` -> Set after appearance loads
- Avoid: Not stopping playing tracks after swapping ids -> `Stop(0)` the playing tracks
- Avoid: Stopping all tracks every frame -> Stop only your own tracks, by reference

### 3D Importer Settings Reference
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/3D Importer Settings Reference.md`
- Meshes should meet the modeling specifications first (20,000 triangles per mesh, watertight, frozen transforms; see Rigging in...
- Dimensions and polycount shown under File Geometry are read-only information.
- The importer page above does not list animation options.
- When a mesh object has the same name as a bone in the connected armature, the importer turns that bone into a Motor6D joining...

### Accessories and Armor That Follow Animation
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Accessories and Armor That Follow Animation.md`
- Avoid: Re-rigging the character to add gear (new joints, moved attachments) -> Weld gear; never touch joints
- Avoid: A rigid plate spanning a joint -> Split at the joint; overlap shapes
- Avoid: Gear with mass or collision -> Massless, no collide, query or touch

### Animation Data from Blender to Roblox
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Animation Data from Blender to Roblox.md`
- Avoid: Reading `matrix_basis` when constraints drive the rig -> Use evaluated `pose_bone.matrix` as in the exporter
- Avoid: Root travel keyed on the top pose -> Key the first child
- Avoid: Axis map applied twice, or not at all -> Calibration clip; one conversion in one place

### Blender Armature and Bone Roll Conventions
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Blender Armature and Bone Roll Conventions.md`
- Avoid: Roll recalculated with different options on different limbs -> One scripted rule for the whole rig
- Avoid: Editing the rest pose after clips exist -> Re-bake clips from world-space targets, or never touch rest after...
- Avoid: Assuming glTF +Z front means Roblox +Z front -> Know the measured importer turn; test with the calibration clip

### Creature Rig Conventions
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Creature Rig Conventions.md`
- Avoid: A member's roll differs from the family -> Rebuild roll by the family script
- Avoid: Deleting unused family bones -> Keep them unweighted; Keep Zero Influence Bones on import
- Avoid: Root translation not retargeted -> Scale translation by the height ratio
- Build the family armature at its reference height with the names above; save one rig file with a proxy mesh.
- Author the shared clips on the proxy (12-13 clips, markers included); export a test GLB, import it into a scratch place, and...
- For each enemy: duplicate the armature, edit rest bone lengths only, model at true stud height, bind, paint the atlas...

### Custom Characters and StarterCharacter
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Custom Characters and StarterCharacter.md`
- Avoid: Non-R15 StarterCharacter with the default Animate -> Own `Animate` in StarterCharacterScripts
- Avoid: Wrong HipHeight -> Compute from the rest model
- Avoid: Anchored or unjoined parts -> Joint everything to the root; nothing anchored

### Custom Rigs - Weapons and Props
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Custom Rigs - Weapons and Props.md`
- Avoid: Clips on a weapon with no Motor6Ds -> Build hinges; check every keyed Part1 has a joint
- Avoid: `RegisterKeyframeSequence` ids in shipped code -> Publish ids or play in Lua
- Avoid: Weapon parts with mass or collision -> `Massless`, no collisions, no queries
- Every moving piece has a Motor6D named after it; static pieces welded.
- Joint frames from a manifest (world- or body-aligned), not from mesh orientation.
- Weapon parts massless, non-colliding, unanchored (template root anchored only in storage).

### Exporting FBX vs GLB
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Exporting FBX vs GLB.md`
- Avoid: Default FBX export, or two scale fixes stacked -> Apply Scalings: FBX Unit Scale, other scales 1.0; or GLB, which...
- Avoid: GLB with Apply Modifiers off (default) -> Turn it on (Armature is skipped; shape keys then do not export) or...
- Avoid: Leaf bones left on (FBX) -> Add Leaf Bones off
- Transforms applied (scale 1, rotation 0) on armature and meshes; root at the origin.
- Faces -Y, final stud height, feet on Z = 0.
- Weights limited to 4 and normalised; no zero-weight vertices.

### Facial Animation and Dynamic Heads
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Facial Animation and Dynamic Heads.md`
- Avoid: Frame 0 not neutral, or Animation Offset not 0 on import -> Neutral at frame 0; offset 0
- Avoid: Same FACS name on two frames -> One frame per pose
- Avoid: Corrective with 4 names -> At most 3 names

### Importing Models and Rigs
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Importing Models and Rigs.md`
- Import preset chosen for the asset kind; Upload off while testing.
- Import found by content, not by name or selection.
- Validation table passes (names, hierarchy, scale, facing, pivot, colours, clips).
- Prepare run; Status shows nothing missing; run it twice to prove idempotency.
- Imported clips renamed, moved, axis-fixed, markers added.
- Pristine import kept somewhere outside the live hierarchy until verified, then removed.

### Layered Clothing Cages and Animation
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Layered Clothing Cages and Animation.md`
- Avoid: Unskinned garment with `AutoSkin` Disabled -> Skin it, or EnabledPreserve
- Avoid: Deleted or re-UV'd cage vertices -> Start from the cage templates; edit positions only
- Avoid: Only an inner cage -> Both cages, outer adjusted

### R15 Rig Anatomy and Attachments
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/R15 Rig Anatomy and Attachments.md`
- Avoid: Code looks for Motor6D only -> Accept AnimationConstraint too
- Avoid: IK uses part centres or vertical arm length -> Use RigAttachment positions (slanted upper arm)
- Avoid: Root motion keyed on HumanoidRootPart -> Key LowerTorso

### Retargeting Animations Between Rigs
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Retargeting Animations Between Rigs.md`
- Avoid: Copying clips between rigs with different roll -> Conjugate by the frame difference
- Avoid: Delta retarget across different rest poses -> Absolute retarget, or pose-match the rests
- Avoid: Root translation unscaled -> Scale by height or hip-height ratio

### Rigging in Blender
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Rigging in Blender.md`
- Avoid: Unapplied object scale -> Ctrl+A > All Transforms on mesh and armature; re-export
- Avoid: Root motion on the root bone -> Move translation to the first child bone
- Avoid: Inconsistent bone roll -> Recalculate roll with one rule for the whole rig; never hand-roll...
- All transforms applied on meshes and armature (scale 1, rotation 0).
- Character faces -Y, feet on Z = 0, final stud height.
- Root bone at origin, no weights on it, motion on its first child.

### Weight Painting and Skinning for Roblox
`tools/knowledge/lihfolk/animation/02 Rigging and Importing/Weight Painting and Skinning for Roblox.md`
- Avoid: More than 4 influences -> Limit Total 4, Normalize All, re-export
- Avoid: Unnormalised weights -> Normalize All; paint with Auto Normalize
- Avoid: Weights on the root bone -> Move influence to the hips or body bone
- ROM action covers every joint's extremes.
- Preserve Volume off while judging.
- Clean, Limit Total 4, Normalize All.

### Animating Creatures and Monsters
`tools/knowledge/lihfolk/animation/03 Authoring/Animating Creatures and Monsters.md`
- Avoid: Making big creatures slow by playing clips slowly -> author slow; never play below 1x
- Avoid: Lift on the top-level root -> key the first child bone
- Avoid: Imported clips not axis-fixed -> apply the 180-degree data fix
- Weight reads through anticipation, impact settle and follow-through, not slow playback
- Cycle speed matches scale (about 1 / sqrt(scale) for scaled copies)
- Every attack has a unique telegraph of 0.45 s or more (0.5 s for bosses), with `Tele` and `Hit` markers

### Animation Capture and Mocap
`tools/knowledge/lihfolk/animation/03 Authoring/Animation Capture and Mocap.md`
- Avoid: Shipping capture output unedited: floaty, slow anticipation, mushy contacts.
- Avoid: Smoothing through the hit frame.
- Avoid: Leaving tiny translations on every limb after retargeting.
- Source shot meets the capture requirements (one person, one shot, under 15 s, stable camera)
- Skeleton errors and flips fixed
- Jitter filtered, contacts protected

### Animation Editor Advanced
`tools/knowledge/lihfolk/animation/03 Authoring/Animation Editor Advanced.md`
- Avoid: Converting to curves when a downstream tool needs KeyframeSequences.
- Avoid: Converting with Euler rotation type on a shoulder-heavy clip and getting gimbal wobble that cannot be undone.
- Avoid: Running Optimize Keyframes and not re-checking contacts and big turns.

### Animation Editor Workflow
`tools/knowledge/lihfolk/animation/03 Authoring/Animation Editor Workflow.md`
- Avoid: Keys inserted off by a frame -> insert at the scrubber position you want, check with frame stepping
- Avoid: Legacy Cubic easing in an old clip -> convert to CubicV2 with the direction flipped
- Avoid: Only some tracks keyed on a key pose -> key all tracks on key poses during blocking
- Rig: correct assembly root, only the root anchored, game proportions, weapon on a Motor6D
- Planned beats with times; gameplay hit time known
- Blocked in Constant; poses read in silhouette from the game camera

### Animation Quality Standard
`tools/knowledge/lihfolk/animation/03 Authoring/Animation Quality Standard.md`
- Arms behind the body: sprint, surf and "load" poses.
- Sprint into thrust or cross-cut lead-ins drive the forearm into the torso; use a flat wind-up.
- Backhand winds and two-handed low guards put the forearm across the chest.
- R15, scaled 1.6x, about 9 studs tall.

### Animation Review Checklist
`tools/knowledge/lihfolk/animation/03 Authoring/Animation Review Checklist.md`
- Watch it in context first. On the real character, with the real weapon, at the real speed and priority, over the real idle and...
- Hit frames land when design says. `Hit` markers sit on the contact frame; data tables (server hit times) match the markers.
- Something moves on the input frame for player actions.
- Secondary motion on dangling parts and the free hand.

### Arm Twist and Grip Roll - Lessons from Baked Sword Clips
`tools/knowledge/lihfolk/animation/03 Authoring/Arm Twist and Grip Roll - Lessons from Baked Sword Clips.md`
- Avoid: Measuring roll with swing-twist angles.
- Avoid: Solving each key independently and trusting interpolation.
- Avoid: Windups that roll against the strike.
- Roll measured as angular velocity about the bone axis
- Per-frame pronation and swivel plan after baking; one direction per segment
- Windup posed so the strike rolls one way

### Authoring Clips in Code
`tools/knowledge/lihfolk/animation/03 Authoring/Authoring Clips in Code.md`
- Avoid: Easing written on the wrong key -> convert "into" to "leaving" on write
- Avoid: Legacy Cubic -> CubicV2
- Avoid: Root keyed on HumanoidRootPart -> key LowerTorso; HRP weight 0

### Blade Grip and Wrist Limits
`tools/knowledge/lihfolk/animation/03 Authoring/Blade Grip and Wrist Limits.md`
- Avoid: Solving the hand as the minimal rotation from rest to target: wrist flips, blades through the head.
- Avoid: Using a fixed "+-50 degree cone from perpendicular" (the old LihFolk rule).
- Avoid: Asking for a blade in line with the forearm (thrust or contact keys).
- Sword modelled with the blade on Handle +Y and the true edge on Handle -Z; `Tool.Grip = CFrame.Angles(math.rad(30), 0, 0)`
- Keys authored with `skey` (hand target + blade), never a raw hand rotation
- `qa.wrist.blade_err_max_deg` under 5; breakdown keys reviewed

### Combat Animation Craft
`tools/knowledge/lihfolk/animation/03 Authoring/Combat Animation Craft.md`
- Avoid: Strike eases into contact -> arrive at speed, decelerate after contact
- Avoid: Hit-stop baked into the clip -> runtime hit-stop only on confirmed hits
- Avoid: No input buffer -> buffer during startup/active; execute at the cancel window
- Something moves on the input frame
- Anticipation 2 to 6 frames (light) / 6 to 15 (heavy); direction obvious
- Strike 1 to 3 frames with dense in-betweens and a trail window

### Free Mocap and Mixamo Licensing for Roblox
`tools/knowledge/lihfolk/animation/03 Authoring/Free Mocap and Mixamo Licensing for Roblox.md`
- Avoid: Downloading a "free CMU FBX pack" with a non-commercial licence and shipping it in a paid game.
- Avoid: Publishing Mixamo or CMU animations as free models or Creator Store packages.
- Avoid: Selling raw mocap as emote items without checking the licence.
- Licence read at the source (Adobe, CMU, vendor), snapshot saved.
- Converted packs checked for their own licence; prefer converting originals yourself.
- Use is inside your game, not redistribution of the data.

### Human Swing Biomechanics
`tools/knowledge/lihfolk/animation/03 Authoring/Human Swing Biomechanics.md`
- Avoid: Solving each key's wrist and elbow independently.
- Avoid: Measuring "twist" with swing-twist decomposition.
- Avoid: Putting all the rotation into the arm with the hips square.
- Hips lead the chest by ~1 frame and the arm by ~2 frames
- 35-60 deg hip-shoulder separation at the top of the windup
- `Clip.cut()` declared for every swing

### Idle Breathing and Secondary Motion
`tools/knowledge/lihfolk/animation/03 Authoring/Idle Breathing and Secondary Motion.md`
- Avoid: Statue idle -> breathing and weight shift; no dead holds over 0.3 s
- Avoid: Panting idle -> 1 to 3 degrees, 3 to 5 s cycle
- Avoid: Idle pose far from attack start poses -> keep the idle near the class rest; re-check clip entries if the...

### Locomotion and Melee Reference Numbers
`tools/knowledge/lihfolk/animation/03 Authoring/Locomotion and Melee Reference Numbers.md`
- Avoid: Authoring the default-speed locomotion as a walk: it slides at 16 studs/s.
- Avoid: Copying real running knee angles onto R15 boxes: the shin enters the pelvis.
- Avoid: Light attacks shorter than 8 f of startup in PvP: nobody can react to them.
- Ground speed in the clip JSON matches the `WalkSpeed` it's played at (foot slide 0)
- Run lift 0.3-0.45 studs; pelvis pitch 8° or less; lean in the chest
- Light attack: hit on f6-8, ComboWindow ~f15 of 24

### Locomotion Clip Lessons - Jump, Sprint, Idle
`tools/knowledge/lihfolk/animation/03 Authoring/Locomotion Clip Lessons - Jump, Sprint, Idle.md`
- Score flat after several tweak rounds: Global flaw treated as local -> Re-author the key poses
- Jump looks stiff: Arms swing with the legs -> Arms lead: back f2, down f3, forward f4, forward-up at take-off
- Jump arms read as T-pose or cheer: Wide or overhead air arms -> Forward, then forward-down, hands outside the knees
- Flaw classified as local (port a fix) or global (re-author) before fixing
- Jump arms lead the push-off by a frame; no wide, overhead or bent-elbow air arms
- Pelvis held up until contact; landing recovery about 13 frames

### Locomotion Readability - Lessons from Blind Panels
`tools/knowledge/lihfolk/animation/03 Authoring/Locomotion Readability - Lessons from Blind Panels.md`
- Avoid: Authoring a walk for WalkSpeed 16.
- Avoid: Sine-wave bobs and even spacing.
- Avoid: Leaning the pelvis instead of the chest.
- Walk at native speed with an AdjustSpeed plan; 16 studs/s = run
- Contact / down / passing / up with uneven spacing and flat holds
- Hip shift + drop; chest roll toward the swing side; head lateral < pelvis lateral

### Moon Animator and Third-Party Tools
`tools/knowledge/lihfolk/animation/03 Authoring/Moon Animator and Third-Party Tools.md`
- Avoid: A tool writes legacy `Cubic` with TweenService meanings -> rewrite to CubicV2 (keep the intended direction)
- Avoid: Frame rate assumptions (24 vs 30 vs 60) -> Roblox stores seconds; check key times after import
- Avoid: Curves the enum cannot represent -> thin with a tolerance

### Posing and Silhouettes
`tools/knowledge/lihfolk/animation/03 Authoring/Posing and Silhouettes.md`
- Avoid: Posing only in the side view -> check every key pose from the gameplay camera
- Avoid: Symmetric arms and legs -> break symmetry everywhere
- Avoid: Weapon inside the silhouette -> move the weapon outside the body outline
- Clear line of action in every key pose
- Centre of mass over the base of support (unless falling)
- No twinning

### Principles of Animation for Games
`tools/knowledge/lihfolk/animation/03 Authoring/Principles of Animation for Games.md`
- These do not appear in the classic twelve but decide whether game animation works.
- Responsiveness: the time from the player's input to the result.
- Film: volumes deform to show speed, impact and flexibility.
- Film: a preparatory move tells the audience something is coming.

### Publishing Animations
`tools/knowledge/lihfolk/animation/03 Authoring/Publishing Animations.md`
- Avoid: Publishing to your account for a group game -> publish under the group, or grant the experience permission
- Avoid: Shipping Studio-only ids -> publish, or a Lua player
- Avoid: Ids hard-coded in many scripts -> one data table or attributes
- No shipped code path uses `RegisterKeyframeSequence` / `RegisterActiveKeyframeSequence`
- Every clip slot has a published id (scan for empty `*Id` attributes and placeholder ids)
- Assets owned by the experience's owner (user or group), or permission granted

### R15 Run Cycle Key Poses
`tools/knowledge/lihfolk/animation/03 Authoring/R15 Run Cycle Key Poses.md`
- Avoid: Lean put in `LowerTorso` -> Pelvis 8° or less; lean the `UpperTorso`
- Avoid: No head counter-rotation -> Head `rx` about equal and opposite to the chest lean
- Avoid: Hip height set by eye -> Solve `LowerTorso` py per key so the planted foot touches the ground
- Two steps per cycle, each with contact, down, push-off and flight.
- Chest leans, pelvis stays within 8°, head counter-rotates.
- Hip bob solved per key for foot contact.

### Spec-Driven Move Authoring
`tools/knowledge/lihfolk/animation/03 Authoring/Spec-Driven Move Authoring.md`
- Avoid: Easing written as "into" -> the key's easing shapes the segment leaving it
- Avoid: Overhead straight to drive -> chop key
- Avoid: Deep root drops -> y >= -1.0 at scale 1

### Spin and Whole-Body Rotation Moves
`tools/knowledge/lihfolk/animation/03 Authoring/Spin and Whole-Body Rotation Moves.md`
- Spin reverses or skips a turn: Rotation keyed sparsely; interpolation takes the short way -> Key the Root joint every frame
- Character over-spins in game: Game code also rotates the HRP -> AutoRotate off during the clip; no HRP turns
- Spin reads as a loop: Equal frames per turn -> Accelerate (8, 7, 6 frames per turn)
- Whole-body rotation on the Root joint, every frame, fixed composition order
- Gameplay code does not rotate the HRP during the clip
- Turns accelerate; hips over the support foot; weapon arm extended

### Timing Spacing and Frame Charts
`tools/knowledge/lihfolk/animation/03 Authoring/Timing Spacing and Frame Charts.md`
- Between two keys the Animator computes `alpha = easing(u)` where `u` runs 0 to 1 over the segment, then blends the two poses...
- Mark the gameplay frames first (hit, cancel window, end).
- Mark which segments must be fast (strike) and which slow (anticipation build, settle).
- Note where in-betweens should be denser (fast arcs need more samples; the reference project's baker sampled strike segments...

### Weapon Swing Design
`tools/knowledge/lihfolk/animation/03 Authoring/Weapon Swing Design.md`
- Avoid: Wrist aims the blade -> aim with the arm; wrist within 70/30/85
- Avoid: Flat leads the cut -> edge = plane axis x blade direction
- Avoid: Overhead straight to drive -> add a chop key over the front

### Elbow Poles Swivel and Joint Limits
`tools/knowledge/lihfolk/animation/04 Procedural/Elbow Poles Swivel and Joint Limits.md`
- Elbow flips sides between frames: per-frame best candidate -> continuity cost, search near last frame, Viterbi offline
- Wrist flips 170 degrees in one frame: twist wrapped at +-180 then clamped -> unwrap twist against last frame before clamping
- Thrust looks like a broken wrist: wrist asked to aim the blade -> aim with the arm; wrist limits force it

### Flight Flocks and Path Animation
`tools/knowledge/lihfolk/animation/04 Procedural/Flight Flocks and Path Animation.md`
- Creature turns flat like a cursor: no bank -> `atan(v * yawRate / g)`, lagged and clamped
- Bank snaps at every control point: C1 curve (Catmull-Rom, arcs) -> uniform cubic B-spline
- Speeds up and slows down for no reason: moving `u` at a constant rate -> arc-length table

### Foot Placement and Ground IK
`tools/knowledge/lihfolk/animation/04 Procedural/Foot Placement and Ground IK.md`
- Feet glued to the ground during a jump: no contact weight, or IK not disabled in air -> weight by animated foot height; disable on air states
- Whole body floats on slopes: hips never lowered -> drop the pelvis by the lowest foot's deficit
- Body bobs up and down climbing stairs: pelvis offset unsmoothed -> asymmetric rates or a spring

### IKControl Reference
`tools/knowledge/lihfolk/animation/04 Procedural/IKControl Reference.md`
- Nothing happens: one of Type / EndEffector / Target / ChainRoot unset, or not under... -> check `GetChainCount() > 0`
- Whole character slides or tilts: `ChainRoot` set to the HumanoidRootPart or LowerTorso -> start the chain at the limb
- Limb pops on start and stop: toggling `Enabled` -> fade `Weight`

### Inverse Kinematics
`tools/knowledge/lihfolk/animation/04 Procedural/Inverse Kinematics.md`
- Elbow snaps from down to up between frames: pole direction nearly parallel to the shoulder-target line -> fall back to a secondary pole, or blend poles by angle
- Arm fully straight then "pops": target beyond reach; `acos` of a value > 1 -> clamp the distance to `[abs(a-b)+e, a+b-e]`
- Hand is exactly on target but the arm looks broken: elbow solved as a ball joint -> solve the elbow as a hinge about its own axis

### Look-At Aim and Head Tracking
`tools/knowledge/lihfolk/animation/04 Procedural/Look-At Aim and Head Tracking.md`
- Head rolls when looking up and sideways: `CFrame.Angles(pitch, yaw, 0)` (X then Y) -> `CFrame.fromEulerAnglesYXZ(pitch, yaw, 0)`
- Head whips shoulder to shoulder: target crossing directly behind -> lose interest behind; hysteresis; unwrap
- Head jitters while the character runs: measuring in last frame's part CFrames -> FK this frame, or measure in the root's frame

### Multi-Bone IK - FABRIK and CCD
`tools/knowledge/lihfolk/animation/04 Procedural/Multi-Bone IK - FABRIK and CCD.md`
- Tail corkscrews when it passes vertical: `CFrame.lookAt` with a world up vector per segment -> minimal rotation from the rest direction, root first
- Chain slowly stretches: lengths taken from the current pose, or drift from float error -> lengths from the rig, re-measured on scale change
- Chain jitters on a still target: not enough iterations, tight cone limits fighting the passes -> more iterations, looser limits, early-out

### Procedural Animation
`tools/knowledge/lihfolk/animation/04 Procedural/Procedural Animation.md`
- Procedural head turn does nothing on a character: write happens in `PreRender` or `Heartbeat` -> move it to `PreSimulation`
- Works on an R6/old rig, does nothing on new avatars: code searches for `Motor6D` -> accept `AnimationConstraint` too
- Head slowly spins away, worse when far from camera: compounding `Transform = Transform * R` on a joint nothing rewrites -> restore in `PreAnimation`, or compute from...
- Joint lookup accepts `Motor6D`, `AnimationConstraint` and `Bone`.
- Writes to animated joints happen in `PreSimulation`.
- Overlays compose with the animated value (`R * A` or `A * R`) and cannot compound.

### Procedural Death Knockdown and Hit Reactions
`tools/knowledge/lihfolk/animation/04 Procedural/Procedural Death Knockdown and Hit Reactions.md`
- Hits feel weightless: no hit-stop, no flash -> 40-90 ms freeze and a 0.1 s flash
- Every hit looks identical: one flinch clip -> directional clips, or springs with noise
- Reactions stack into jitter: flinches during stuns and knockups -> skip the flinch while a stronger reaction owns the body

### Procedural Gait and Footstep Planning
`tools/knowledge/lihfolk/animation/04 Procedural/Procedural Gait and Footstep Planning.md`
- Feet slide at top speed: rate clamp on the run clip -> foot-plant layer, or a faster clip, or accept and hide with dust
- Feet pedal frantically: cadence tied linearly to speed -> cap the cadence; lengthen the stride instead
- Legs pop when walk blends to run: clips out of phase -> shared phase, same contact timing in all cycles

### Procedural Weapon Motion
`tools/knowledge/lihfolk/animation/04 Procedural/Procedural Weapon Motion.md`
- Blade spins about its own centre, not the grip: rotation applied without the pivot sandwich -> `CFrame.new(P) * R * CFrame.new(-P) * B`
- Spin ends with the edge backwards: turns not an integer or ramps ignored -> solve `W` so the integral is whole turns
- Toss rockets up and slams down: `workspace.Gravity` -> gravity scaled to the avatar

### Ragdolls and Physics-Based Animation
`tools/knowledge/lihfolk/animation/04 Procedural/Ragdolls and Physics-Based Animation.md`
- Living player dies the instant the ragdoll starts: neck joint disabled with `RequiresNeck = true` -> set it false first
- Body falls apart into loose parts on death: `BreakJointsOnDeath = true` -> set false before death and build sockets
- Ragdoll stands back up or slides around: Humanoid still in a normal state -> `Physics` state on the owner, or `EvaluateStateMachine = false`

### Springs Damping and Smoothing
`tools/knowledge/lihfolk/animation/04 Procedural/Springs Damping and Smoothing.md`
- Smoothing feels faster on high-refresh monitors: `lerp` by a constant per frame -> `1 - exp(-k dt)`
- Spring explodes after a lag spike: explicit or semi-implicit step with a large `dt` -> clamp `dt`, substep, or exact update
- Spring "dies" at 30 fps, lively at 144: implicit Euler's numerical damping -> exact update

### Tween and CFrame Animation of Objects
`tools/knowledge/lihfolk/animation/04 Procedural/Tween and CFrame Animation of Objects.md`
- Model's parts fly apart during a tween: tweening `Position` on a welded root -> tween `CFrame`
- Door stops halfway: a second tween on the same property cancelled the first -> one controller per property; check `Completed` state
- Lid clips into the base on close: Back / Elastic overshoot not checked -> sample `GetValue` over the curve, check the range against geometry

### Two-Bone IK Math
`tools/knowledge/lihfolk/animation/04 Procedural/Two-Bone IK Math.md`
- Rig parts vanish (NaN): `acos` of a value outside [-1, 1] -> clamp the distance and the cosine
- Forearm rolls as the arm bends: bones treated as straight sticks on slanted R15 bones -> solve the hinge in the elbow's own frame (section 4)
- Elbow bends backwards: wrong branch sign, or a ball-joint solve -> pick `s` per joint, clamp to the hinge range

### Ability Kits Architecture
`tools/knowledge/lihfolk/animation/05 Systems/Ability Kits Architecture.md`
- Move names checked against the reserved list.
- Clips installed, each with markers that survived the bake (`Hit` or beat names, `Open` and a climax marker on ultimates).
- Upper copies for non-cinematic moves, tagged.
- Server hit table with `t = marker - 0.06`; guard wired (three lines: create, origin clamp, cast check).
- Registry entry (moves, labels, cooldowns) and Info descriptions; icons.
- Beat table entries for non-standard marker names; travel list.

### Boss Animation and Phases
`tools/knowledge/lihfolk/animation/05 Systems/Boss Animation and Phases.md`
- Intro once per player, skippable after 2 s, untouchable, restores on every exit.
- Phase shift clips untouchable, their hits still land; hazards start after the clip.
- `(Hit - Tele) / speed >= 0.5` for every attack in every phase.
- Following markers lock 0.45 s before the blow; fast shot markers last until arrival.
- Only the last combo link gold.
- Root motion on a child joint; orbit pieces in code.

### Camera Shake and Camera FX
`tools/knowledge/lihfolk/animation/05 Systems/Camera Shake and Camera FX.md`
- FOV ends up a degree off after a session: Exponential decays never reach 0 -> Finite envelopes ending at exactly 0
- Effects fight sprint FOV: Two writers of FieldOfView -> One owner adds the stack's delta
- Camera turns slightly after every dash: Look rotated in the offset, not undone -> Translation and roll only; undo before the module

### Cameras and Cutscenes
`tools/knowledge/lihfolk/animation/05 Systems/Cameras and Cutscenes.md`
- Camera slowly turns after effects: Additive offset not undone, or yaw in the offset -> Undo at Camera-1; translation and roll only
- HUD missing after a cutscene: No restore on an error path -> Watchdog and restore on every exit
- Camera stuck Scriptable after death: Restore not hooked to Died / CharacterRemoving -> Hook both

### Cinematic Ultimates
`tools/knowledge/lihfolk/animation/05 Systems/Cinematic Ultimates.md`
- Explosion next to the enemies: Damage, VFX and enemy position use different centres -> One anchor, shared compute
- Enemies die during the drag: Damage before pull -> Pull-only first hit
- Impact a few frames off the cut: Enemy snap on server time -> Local climax for the caster

### Combat Combo Systems
`tools/knowledge/lihfolk/animation/05 Systems/Combat Combo Systems.md`
- Clicks get "eaten": Early presses dropped -> One-press buffer of 0.1 to 0.2 s
- Two swings blur together: Fading the new swing over a still-playing old one -> Stop the old (0.06), start the new from the link pose (0.04)
- Snap when the player stops clicking: End pose not the class rest -> Re-key the last frame to the rest
- Hit times stored per step (from markers), divided by playback speed.
- Min gap per class; later-hit rule applied.
- One-press buffer; combo window resets the step.

### Custom Locomotion Controller Design
`tools/knowledge/lihfolk/animation/05 Systems/Custom Locomotion Controller Design.md`
- Legs reset to frame 0 after every ability: Controller stopped locomotion for an overrider instead of keeping...
- Gait flickers on slopes: Raw speed, no smoothing or hysteresis
- Brief T-pose on seat: `decide` returns Idle for `Seated` instead of handing back
- `decide` is pure and unit tested with a speed/state table.
- Gait data lives with the clips; load-time validation of bands vs clamps.
- Speed smoothed, hysteresis on downshift.

### Emotes and Social Animation
`tools/knowledge/lihfolk/animation/05 Systems/Emotes and Social Animation.md`
- Wheel opens but emotes do nothing: `Animate` removed or replaced -> Keep Animate, or play emotes yourself and disable the menu
- Only arms move during an emote: Locomotion at higher priority than the emote asset -> Play emotes at Action yourself
- Emote survives into combat: No cancel hooks -> Central `stop()` from every gameplay action

### Enemy and NPC Animation
`tools/knowledge/lihfolk/animation/05 Systems/Enemy and NPC Animation.md`
- Enemy attacks from behind: Imported clips play turned 180 degrees -> Axis fix every imported clip
- Root motion missing: Root keyed at the top-level pose -> Key a child joint
- Floating or sinking: Clips not ground-fitted; wrong HipHeight -> Fit per clip; set HipHeight; runtime slope clamp

### Enemy Animation State Protocol
`tools/knowledge/lihfolk/animation/05 Systems/Enemy Animation State Protocol.md`
- Same attack twice in a row plays once: Listening to `Anim` only -> Always rewrite and listen to `AnimT`
- Wrong speed on first frames: Client read `Anim` before `AnimSpeed` -> Write `AnimT` last, react to it
- Late joiners see attacks restart: No offset -> `TimePosition = offset * speed`

### Equip Draw and Flourish Systems
`tools/knowledge/lihfolk/animation/05 Systems/Equip Draw and Flourish Systems.md`
- Flourish cancelled immediately on equip: A carry or other non-action layer at Action2+ -> Keep non-actions below Action2
- Wrist snaps at the end: Flourish end pose differs from the hold -> One shared end pose (blade over shoulder)
- Toss looks too heavy: `workspace.Gravity` used -> Gravity scaled to character height

### First-Person Viewmodels - Arms Rig, Sway and Camera Binding
`tools/knowledge/lihfolk/animation/05 Systems/First-Person Viewmodels - Arms Rig, Sway and Camera Binding.md`
- Avoid: Queryable viewmodel parts -> `CanQuery`, `CanCollide`, `CanTouch` off
- Avoid: Bound before the camera -> Bind after camera priority
- Avoid: Mouse-delta sway only -> Sway from camera rotation change
- Local-only viewmodel under the camera; all parts non-colliding, non-queryable, shadowless.
- Bound after camera priority; destroyed on every exit path.
- Sway works for mouse, touch and gamepad; bob scales with speed; reduced-motion respected.

### Hit Reactions Stagger and Hit-Stop
`tools/knowledge/lihfolk/animation/05 Systems/Hit Reactions Stagger and Hit-Stop.md`
- Sword arrives after its sparks in flurries: Plain freeze hit-stop with clock-timed VFX -> Catch-up hit-stop
- Enemy twitches while held or stunned: Flinch not skipped in higher states -> Ladder skip rules
- Stagger feels like lag: Attack clips slowed -> Only slow the stagger clip

### Locomotion Systems
`tools/knowledge/lihfolk/animation/05 Systems/Locomotion Systems.md`
- Feet skate at top speed: Rate clamp below `speed / GroundSpeed` -> Author a gait for that speed, or widen the band check above
- Legs scissor on walk to run: Gaits start at time 0, or authored with different lead foot -> Phase-preserving swap, one foot-order convention
- Stumble on every bunny-hop: Land plays while moving -> Land only standing
- Each gait's authored GroundSpeed stored as data, read by the controller.
- The most common movement speed has a clip authored at that speed.
- For every gait, `[threshold_low, threshold_high]` is inside `[clamp_min, clamp_max] * GroundSpeed`.

### Mounts and Rideable Creatures
`tools/knowledge/lihfolk/animation/05 Systems/Mounts and Rideable Creatures.md`
- Avoid: Mount parts with mass welded to rider -> `Massless`, no collision on mount parts
- Avoid: Mount Animator created on client -> Create or clone it on the server
- Avoid: Design B mount tracks played on rider client -> Server plays, or replicated gait attribute
- Architecture picked for a stated reason (A, B or C).
- Mount parts massless and non-colliding in Design A; ownership handoff in Design B.
- Animators created server-side; replication path matches the design.

### Stance and Charge Moves
`tools/knowledge/lihfolk/animation/05 Systems/Stance and Charge Moves.md`
- Strike plays seconds late the first time: Clip not loaded -> Preload on kit load; cast at once without a clip
- Dash fires off the visual cut: Clips retimed, marker table stale -> Regenerate the table from clips
- Sheath inside the thigh: Sheath fitted to the idle -> Fit to the stance pose

### UI and GUI Animation
`tools/knowledge/lihfolk/animation/05 Systems/UI and GUI Animation.md`
- Pop grows from a corner: Default AnchorPoint -> Scale about the centre
- List jiggles during a pop: Tweening `Size` in a layout -> `UIScale`
- Phone gets hot in menus: Per-frame loops while hidden -> Loops only while visible

### VFX Timing with Animation
`tools/knowledge/lihfolk/animation/05 Systems/VFX Timing with Animation.md`
- Sparks before the blade arrives after hit-stops: Plain freeze plus wall-clock effects -> Catch-up hit-stop
- Sheet of trail under the blade: Trail on through the wind-up -> Trail window on markers, or crescents
- Streak across the body when a trail enables: Trail not cleared -> `Trail:Clear()` before enabling

### Weapon and Combat Animation
`tools/knowledge/lihfolk/animation/05 Systems/Weapon and Combat Animation.md`
- Weapon clip has no effect: Weld grip, or keys for joints with no Motor6D -> Motor6D grip; check joint names
- Weapon parts animate in Studio only: Runtime-registered clip ids -> Publish, or play through a Lua player
- Arm sticks straight out over custom poses: Animate's tool-hold -> Fade it to 0.001

### Animation LOD
`tools/knowledge/lihfolk/animation/07 Gotchas/Animation LOD.md`
- Avoid: Skipping the whole update for far rigs, including time and events.
- Avoid: Updating every Nth frame but advancing by one frame's dt (slow motion at range).
- Avoid: All rigs re-tiering on the same frame (periodic spike).
- Time and gameplay events advance for every rig every frame.
- Skipped frames accumulate dt.
- Tiers re-evaluated on a staggered schedule, with hysteresis.

### Debugging Animation Problems
`tools/knowledge/lihfolk/animation/07 Gotchas/Debugging Animation Problems.md`
- Observation sentence written with rig, mode, viewer, clip, symptom, time.
- Track id printed; `Length > 0` confirmed.
- `GetPlayingAnimationTracks` report captured at the moment of the bug.
- Clip soloed (other tracks and Animate off).
- Clip tried on a known-good rig.
- Owner, other client and server compared.

### Engine Gotchas
`tools/knowledge/lihfolk/animation/07 Gotchas/Engine Gotchas.md`
- Avoid: Treating "works in Studio" as "works". It means "works with Studio-only ids and Studio-only timing".
- Avoid: Calling `LoadAnimation` per use.
- Avoid: Using `Stopped` for cleanup of things that are still visible.

### Open Questions
`tools/knowledge/lihfolk/animation/07 Gotchas/Open Questions.md`

### Performance and Memory
`tools/knowledge/lihfolk/animation/07 Gotchas/Performance and Memory.md`
- No `RBX_ANIMSAVES`, `AnimSaves` or backup clip folders shipped.
- Kit folders under budget (2,500 used); per-move instance counts known.
- Keys thinned; placeholders pruned; marker keyframes kept.
- Clips that are not edited this week are stubbed or published.
- Tracks cached per Animator; marker connections bounded.
- One shared update loop for Lua players and procedural motion.

### Testing Animations in Studio
`tools/knowledge/lihfolk/animation/07 Gotchas/Testing Animations in Studio.md`
- Clip stepped in Edit on a correctly scaled dummy with real joints (Motor6D and AnimationConstraint variants).
- Lua models A/B'd against the Animator on originals, not stubs.
- Filmstrip and marker contact sheet reviewed.
- Static lint clean; dynamic QA within thresholds.
- Two-client test with late join and simulated lag.
- Published test place run for any clip or system that could use Studio-only ids.

### Troubleshooting Table
`tools/knowledge/lihfolk/animation/07 Gotchas/Troubleshooting Table.md`

### Snippets - Animation Controller Module
`tools/knowledge/lihfolk/animation/08 Snippets/Snippets - Animation Controller Module.md`
- Avoid: Several scripts calling `Play`/`Stop` on the same tracks.
- Avoid: Choosing gaits without hysteresis.
- Avoid: `AdjustSpeed` every frame on a replicated character (network noise).

### Snippets - Building and Sampling Clips
`tools/knowledge/lihfolk/animation/08 Snippets/Snippets - Building and Sampling Clips.md`
- Avoid: Sampling with `CFrame:Lerp` and trusting it on big turns.
- Avoid: Putting easing on the destination key.
- Avoid: Keying the top-level root pose.

### Snippets - Camera Shake and FX
`tools/knowledge/lihfolk/animation/08 Snippets/Snippets - Camera Shake and FX.md`
- Avoid: Writing `Camera.CFrame` additively without removing it (camera drifts).
- Avoid: Switching to `Scriptable` for gameplay shake.
- Avoid: Random per-frame jitter instead of noise.

### Snippets - Enemy Client Animator
`tools/knowledge/lihfolk/animation/08 Snippets/Snippets - Enemy Client Animator.md`
- Avoid: The server also playing tracks on the same enemy (double playback, server cost).
- Avoid: Using `os.clock()` instead of `workspace:GetServerTimeNow()` for start times.
- Avoid: Writing the serial before the other attributes (the client reads a half-updated state).

### Snippets - IK
`tools/knowledge/lihfolk/animation/08 Snippets/Snippets - IK.md`
- Avoid: Reading `Part.CFrame` in `PreSimulation` for the chain parent (one frame behind).
- Avoid: Targets past full reach (snaps and flips at the singular straight pose).
- Avoid: A pole on the limb line (undefined bend plane).

### Snippets - Lua Keyframe Player Core
`tools/knowledge/lihfolk/animation/08 Snippets/Snippets - Lua Keyframe Player Core.md`
- Avoid: Writing `Transform` from `RenderStepped`.
- Avoid: Firing markers inside the mixer loop. Symptom: a handler that starts a track corrupts the iteration, random missing poses.
- Avoid: No cleanup when a rig is destroyed. Symptom: memory climbs per respawn, errors on parented-nil joints.
- A/B against `Animator:StepAnimations` on a few clips: uneven and evenly keyed, Linear and eased, big turns far from rest, two...
- Markers: every marker fires exactly once per pass, including at time 0 and at the clip's end, and on loop wrap.
- Stop during a fade, Play during a fade, Play while playing (restart), AdjustWeight while fading.

### Snippets - Playing Animations
`tools/knowledge/lihfolk/animation/08 Snippets/Snippets - Playing Animations.md`
- Avoid: `animator:LoadAnimation(anim):Play()` per click (new track each time; 256-track warning).
- Avoid: Setting `Priority` after `Play`.
- Avoid: Computing speed from `Length` before the clip has loaded (speed 0, frozen clip).

### Snippets - Procedural Motion
`tools/knowledge/lihfolk/animation/08 Snippets/Snippets - Procedural Motion.md`
- Avoid: Writing joints in `PreRender`/`Heartbeat` (overwritten by the next animation step).
- Avoid: Compounding `Transform = Transform * offset` (winds up on throttled or unkeyed frames).
- Avoid: Per-frame smoothing with a constant factor (`x += (t - x) * 0.2`), which changes with frame rate.

### Snippets - Ragdoll
`tools/knowledge/lihfolk/animation/08 Snippets/Snippets - Ragdoll.md`
- Avoid: Changing a player character's Humanoid state from the server.
- Avoid: Leaving the Animator playing locomotion during the ragdoll (limbs fight physics or the body slides).
- Avoid: Editing the R15 rig attachments to set socket axes (create your own).

### Pipeline - Importing Animated Enemies
`tools/knowledge/lihfolk/animation/06 Pipelines and Tooling/Pipeline - Importing Animated Enemies.md`
- Blender conventions met (units, -Y facing, Root bone, influences, bone budget, names).
- Clips table exported with durations, loop flags and marker times.
- Imported without moving it; identified by content.
- `Prepare` report: clips all found, markers added, no Humanoid.
- `SetIds` (with or without ids): model stored, clip sources moved out, definition patched, Tele / Hit present on every attack.
- Axis fix applied (clip attribute shows fixed or wrapped); facing checked against the root's LookVector in a test spawn.

### Enemy Import Axis Fix
`tools/knowledge/lihfolk/animation/06 Pipelines and Tooling/Enemy Import Axis Fix.md`
- body faces backwards but hits land in front
- body faces backwards and hits land behind
- root never rises, leaps happen in place
- forward travel mirrored on one rig
- one limb occasionally swings the long way

### Lua Keyframe Player
`tools/knowledge/lihfolk/animation/06 Pipelines and Tooling/Lua Keyframe Player.md`
- It is not free. You take on correctness (every Animator behaviour you rely on must be re-implemented and tested), CPU cost...
- Enemies and other non-player rigs first (the initial goal: no published ids for 1,500 enemy clips).
- AnimationTrack-compatible tracks. Tracks expose `Play(fade, weight, speed)`, `Stop(fade)`, `AdjustSpeed`...
- Measured in Edit against `Animator:StepAnimations` (details and numbers in How the Animator Really Behaves and Lua Keyframe...

### Lua Keyframe Player Internals
`tools/knowledge/lihfolk/animation/06 Pipelines and Tooling/Lua Keyframe Player Internals.md`
- Rotations are stored as rotation vectors (`axis * angle`, from `CFrame:ToAxisAngle()`), not quaternions or CFrames, because...
- `CubicV2`: code `4 + direction` (In 0, Out 1, InOut 2): TweenService directions.
- `SampleCFrame(clip, joint, t, scale)` wraps this and returns a CFrame; it is how tools and procedural systems read clips...
- Track state: `TimePosition`, `Speed`, `Length`, `Looped`, `Priority` (and a numeric rank), `IsPlaying`, `WeightCurrent`...

### Animation Start Here
`tools/knowledge/lihfolk/animation/Animation Start Here.md`

### Animation Glossary
`tools/knowledge/lihfolk/animation/Animation Glossary.md`
- Avoid: Reading "Core" as the top priority. Symptom: sorting priorities by enum value puts Core last.
- Avoid: Calling a stub an animation. Symptom: a stub plays nothing through the engine.
- Avoid: Confusing IKControl with a baked IK clip.
