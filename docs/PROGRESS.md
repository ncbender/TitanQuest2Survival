# Progress

## Done (2026-10-21)

- Environment: .NET 10, Rust 1.99, Python 3.14 (`py`), Windows SDK 10.0.26100 installed for MSVC linking.
- TQ2 archives open with CUE4Parse (`GAME_TitanQuest2`); own `.usmap` from live game memory.
- **Milestone 1:** Boarman Brute (mesh, textures, skeleton, 39 original clips) animated in Bevy.
- **Milestone 2:** `l_main_menu_scene_02` beach cove (2,906 placements, landscape, 105 materials)
  rendered with animated enemies.
- **Vertical slice (playable):** hero (TQ2 male body + Greek adventurer armour + hair, staff
  animations) on the original terrain; Boarman waves with elites and bosses (5:00, 9:00);
  auto-aimed forge bolts, Nova, crits, pierce; XP crystals (TQ2 mystic shards); level-up with
  3 random upgrades; HUD; 10-minute run, death/victory screen, restart.
- ACL animations decode (`CUE4Parse-Natives.dll` built locally, see ASSET_PIPELINE).

## Performance (measured, release build, Ryzen 7 9800X3D + Radeon RX 9070 XT, VSync 120 Hz)

`TQ2_AUTOPLAY=1 TQ2_BENCH=<n> cargo run --release` (each Boarman = 3 skinned parts, all animated):

| enemies | avg FPS | avg ms | p95 ms |
|---|---|---|---|
| 50 | 120 (VSync cap) | 8.33 | 8.63 |
| 100 | 112 | 8.91 | 9.73 |
| 250 | 60 | 16.73 | 20.94 |
| 500 | 45 | 22.27 | 30.59 |

After merging part skeletons into the body at runtime (`character.rs`, same day):

| enemies | avg FPS | avg ms | p95 ms |
|---|---|---|---|
| 250 | 115 | 8.69 | 10.15 |
| 500 | 84 | 11.95 | 14.70 |
| 800 | 50 | 19.94 | 25.39 |

Profiling note: with only the body part (`TQ2_BODY_ONLY=1`) 500 Boarmen hit the 120 FPS cap, so
the extra skeletons of mane/arm-hair parts were the main cost.

- Playtest fixes (2026-10-21): multishot fans out visibly (±11° per extra bolt), floating damage
  numbers, Nova fire flash (verified: 11–15 enemies hit per pulse with 30 enemies in range),
  flow-field pathing so enemies walk around rocks/water (unit test + 500 enemies at 79 FPS).

## Next

1. Performance beyond 500: animation LOD / reduced update rate for distant enemies.
2. ~~More enemy types~~ Spider, Giant Boar, Skeleton (rises from the ground), Harpy, Cyclops boss
   added via `tools/assets.json` + `bestiary.rs`; still no real death animation.
3. ~~Weapon on the hero~~, ~~attack animations~~ done; the hero still faces where it runs, not
   where it attacks. TQ2 flipbook VFX for fireballs/Nova/Meteor done.
4. ~~Terrain layer blending~~ done (11 original ground layers); larger land arena (Greece world
   partition, FastGeo) still open.
5. Satyr assembly (static-mesh parts on the humanoid rig).
6. ~~Main menu, character creation, save~~ done; settings, more items (rarity/stats), then
   two-player co-op.

## Gear (first version, 2026-10-21)

- `equipment.rs`: weapons (Staff of Apollo – fire bolts, Bow of Iris – piercing arrows,
  Battlemage blade – melee sweep) with their own stance clips, attach bone, sounds and attack;
  armour sets (Adventurer, Scholar, Ranger) swap the visible torso/legs/shoulders and absorb
  damage. Elites and bosses drop a TQ2 chest; walking into it offers three pieces of gear.
- Verified: screenshots of bow+ranger and sword+scholar; chest -> sword swap in a test run.
- Superseded by "Weapon slots, attack animations, meta progression" below.

## Character creation, fireballs, English UI (2026-10-21)

- Main menu → "New Hero" → character creation: pick the starting weapon (now part of the
  profile, see below).
- Staff fires fireballs: TQ2 flipbook `t_explosion_01_05x05_c` (frames 0–4 looping) with an
  orange point light; explodes on impact (frames 0–11, the fiery part before the smoke).
  `vfx.rs` holds the camera-facing flipbook sprites. Fire Nova spawns a ring of 10 explosions.
- First elite is guaranteed after 30 s (verified: elite at 31 s, chest dropped at 40 s in an
  autoplay run), then every 15th spawn after a minute as before.
- All in-game text is English (menu, pause, HUD, level-up, upgrades, gear, loot, end screen,
  enemy names).
- Test hook `TQ2_MENU=hero` opens the character creation page (for screenshots).

## Weapon slots, attack animations, meta progression (2026-10-21)

- Slots (user's rules, D7): two-handed weapon alone, or one-handed weapon + second one-handed
  weapon / shield / talisman. Items: Staff of Apollo, Arch of Iris, Sickle of Kronos (2H axe),
  Battlemage Sword, Sword of Peleus (1H), Shield of Heracles (20 % block), Arcane Mirror
  (talisman, +40 % skill damage), three armour sets. Dual wielding attacks 30 % faster.
- TQ2 stance sets per combination (staff2h, bow, axe2h, sword, sword+shield, sword+sword,
  sword+talisman): idle, run and three attack clips each (35 clips baked into the hero).
- Attacks play TQ2's attack clips on the upper body only (`UpperBodyAction`, mask from
  `Spine1_M`), legs keep running. Verified by screenshots: sword+shield and axe swings while
  running, dual wield, talisman. Left-hand weapons need the opposite blade flip.
- Meta progression (D6): chest items go to the stash at the end of the run; the armoury (main
  menu) equips them; favour buys skill-tree nodes (Warfare, Defence, Fire, Fortune). Fire Nova
  and the new Meteor skill must be unlocked in the tree before level-ups offer them.
- Verified: unit tests (slot rules, dual wield needs two copies, loadout round trip, favour,
  skill purchase/refund); a 45 s autoplay run with `TQ2_TEST_END` banked the chest item and
  1 favour into the profile file; screenshots of title, armoury, skill tree, end screen.
- Performance: 500 enemies 76–86 FPS (two runs) after limiting the masked graph to the hero.

## TQ2 masteries, styled menus (2026-10-21)

- Six TQ2 masteries with real data: 61 active skills, 127 passives, 509 modifiers, 330+ icons,
  artwork. Mastery screen in TQ2's look (gold skill/passive frames, tiers I–V, mastery art and
  emblem, details with TQ2's text filled with this game's numbers); mastery chooser; armoury
  with TQ2 item icons, marble background, gold frames, gemstone buttons.
- All 61 skills have behaviour (`abilities.rs`: projectiles, strikes, novas, lines, zones,
  rain, buffs, summons, dashes, melee, curses, chains, orbs, mines) with element ailments and
  TQ2 VFX textures (`fx.rs`). Verified: every skill casts in a 300-enemy run per mastery, no
  crash; FPS with all 10 skills of a mastery and 300 enemies: Storm 73, Earth 113, Warfare 105,
  Rogue 97, Spirit 114, Forge 115. 500 enemies without skills: 79.5 FPS.
- Passives: 117 of 127 act through their stat lines; modifiers: 327 of 509 have effects in
  runs. Not yet: feats, special passives (Rage, …), summon models (pets are glowing orbs),
  the hero facing its attack target.

## Arenas, more enemies and items, TQ2 mechanics, updates (2026-10-21)

- Four arenas (`arenas.rs`), chosen on a new page after "Start Run": Aegean Coast (menu2 beach),
  Arcadian Woods, Necropolis, Burnt Village. The last three are cut from TQ2's open-world map
  (Greece world partition, `Tq2Extract region`, listed under `regions` in `tools/assets.json`).
  Each has its own roster and boss, enemy strength x1.0–2.5 and loot x1.0–2.2; surviving an
  arena unlocks the next (shared by all heroes, `profile.cleared`). The map is now loaded per
  run (`map.rs`) instead of once at start-up.
- Region export: helper meshes (shadow casters, fog gates, light shafts, VFX/dev meshes) are
  dropped, invisible blockers keep only their collision; the landscape streaming proxies are
  merged into world-aligned layer weight maps; landscape holes are marked in `ground.json`,
  blocked for units and shown as dark pits. The playable area is the props' extent.
- Nine new TQ2 creatures: Wolf, Scorpion, Giant Bat, Crab, Lion, Crocolisk, Boarman Shaman,
  Lich, Lich Lord (boss), plus scaled bosses Nemean Lion and Elder Crocolisk. Verified in the
  gallery (textures, animations).
- 24 more TQ2 unique items (staffs, bows, two-handed axes, swords, shields, talismans) with
  their TQ2 meshes and icons; verified in hand for axe, sword+shield, bow, staff.
- TQ2 mechanics for the passives that had no effect (`mechanics.rs`): Barrier, Rage, glancing
  hits, execution threshold, shades; plus Divine Core range (pickup) and devices (skill
  damage). Coverage: passives 147 of 148 (was 117 of 127 counted before the data update).
- `update.ps1`: one command for the friend's PC (pull, mappings, natives, assets only when the
  tools changed, build, start). README has a quick start.
- Not yet: real death animations (bodies still tip over and sink), animation LOD for big
  hordes, enemy ranged attacks, untextured spline meshes in the woods.

## Blockers / risks

- Usmap RVA is build-specific: a TQ2 patch requires re-running the dumper with the game open.
- `a_boarman_mage_death_01` decodes to only 2 frames (unverified why).
