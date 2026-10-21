//! Modular TQ2 characters: a body plus parts (hair, armour, mane…) exported as separate
//! skinned glTFs that all carry the same baked clips (`Tq2Extract animate`). Each part has
//! its own AnimationPlayer; `CharacterAnim` on the root drives all of them in lockstep.
use bevy::gltf::Gltf;
use bevy::prelude::*;
use std::collections::HashMap;

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PartLibrary>()
            .add_systems(Update, (instantiate_parts, bind_players, drive_animation).chain());
    }
}

/// The clip every part of a character plays. Change `clip` to switch animations.
#[derive(Component, Clone, PartialEq)]
pub struct CharacterAnim {
    pub clip: &'static str,
    pub repeat: bool,
    pub speed: f32,
}

impl CharacterAnim {
    pub fn looping(clip: &'static str) -> Self {
        Self { clip, repeat: true, speed: 1.0 }
    }
    pub fn once(clip: &'static str) -> Self {
        Self { clip, repeat: false, speed: 1.0 }
    }
}

/// A part glb waiting for its asset before the scene is spawned.
#[derive(Component)]
struct PendingPart(String);

/// Spawned part scene root; `root` is the character entity carrying `CharacterAnim`.
#[derive(Component)]
struct CharacterPart {
    root: Entity,
    path: String,
}

/// Links an AnimationPlayer back to its character and part.
#[derive(Component)]
struct PlayerOf {
    root: Entity,
    path: String,
}

struct PartAssets {
    gltf: Handle<Gltf>,
    graph: Option<(Handle<AnimationGraph>, HashMap<String, AnimationNodeIndex>)>,
}

/// One glTF + one animation graph per part file, shared by every character using it.
#[derive(Resource, Default)]
struct PartLibrary(HashMap<String, PartAssets>);

/// Spawns the parts of a character as children of `root` (which must have a Transform).
pub fn attach_parts(commands: &mut Commands, root: Entity, parts: &[&str]) {
    for path in parts {
        let child = commands.spawn((PendingPart(path.to_string()), Transform::default(), Visibility::default())).id();
        commands.entity(root).add_child(child);
    }
}

fn instantiate_parts(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut library: ResMut<PartLibrary>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    pending: Query<(Entity, &PendingPart, &ChildOf)>,
) {
    for (entity, part, parent) in &pending {
        let entry = library
            .0
            .entry(part.0.clone())
            .or_insert_with(|| PartAssets { gltf: assets.load(part.0.clone()), graph: None });
        let Some(gltf) = gltfs.get(&entry.gltf) else { continue };
        if entry.graph.is_none() {
            let mut graph = AnimationGraph::new();
            let nodes = gltf
                .named_animations
                .iter()
                .map(|(name, clip)| (name.to_string(), graph.add_clip(clip.clone(), 1.0, graph.root)))
                .collect();
            entry.graph = Some((graphs.add(graph), nodes));
        }
        commands
            .entity(entity)
            .remove::<PendingPart>()
            .insert((WorldAssetRoot(gltf.scenes[0].clone()), CharacterPart { root: parent.parent(), path: part.0.clone() }));
    }
}

fn bind_players(
    mut commands: Commands,
    library: Res<PartLibrary>,
    new_players: Query<Entity, Added<AnimationPlayer>>,
    parents: Query<&ChildOf>,
    parts: Query<&CharacterPart>,
    mut anims: Query<&mut CharacterAnim>,
) {
    for player in &new_players {
        let Some(part) = parents.iter_ancestors(player).find_map(|e| parts.get(e).ok()) else { continue };
        let Some((graph, _)) = library.0.get(&part.path).and_then(|p| p.graph.as_ref()) else { continue };
        commands
            .entity(player)
            .insert((AnimationGraphHandle(graph.clone()), PlayerOf { root: part.root, path: part.path.clone() }));
        // Re-apply the current clip so a late part starts in sync with the others.
        if let Ok(mut anim) = anims.get_mut(part.root) {
            anim.set_changed();
        }
    }
}

fn drive_animation(
    library: Res<PartLibrary>,
    changed: Query<(Entity, &CharacterAnim), Changed<CharacterAnim>>,
    mut players: Query<(&PlayerOf, &mut AnimationPlayer)>,
) {
    if changed.is_empty() {
        return;
    }
    let wanted: HashMap<Entity, &CharacterAnim> = changed.iter().collect();
    for (of, mut player) in &mut players {
        let Some(anim) = wanted.get(&of.root) else { continue };
        let Some((_, nodes)) = library.0.get(&of.path).and_then(|p| p.graph.as_ref()) else { continue };
        let Some(&node) = nodes.get(anim.clip) else { continue };
        player.stop_all();
        let active = player.play(node);
        active.set_speed(anim.speed);
        if anim.repeat {
            active.repeat();
        }
    }
}
