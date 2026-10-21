//! Modular TQ2 characters: a body plus parts (hair, armour, mane…) exported as separate
//! skinned glTFs that all carry the same baked clips (`Tq2Extract animate`). Each part has
//! its own AnimationPlayer; `CharacterAnim` on the root drives all of them in lockstep.
//!
//! Once spawned, every part is re-skinned onto the body's joints and its duplicate joints are
//! removed, so a character animates one skeleton (plus the few bones only a part has, e.g. a
//! mane). Without this, extra parts tripled the animation cost per enemy.
//!
//! `UpperBodyAction` plays a one-shot clip (an attack) on the upper body only, from `Spine1_M`
//! up, over the base clip, so the legs keep running while the arms swing.
use bevy::animation::{AnimationTargetId, RepeatAnimation};
use bevy::gltf::{Gltf, GltfNode};
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PartLibrary>()
            .add_systems(
                Update,
                (
                    sync_parts,
                    instantiate_parts,
                    merge_parts,
                    bind_players,
                    drive_animation,
                    blend_base,
                    drive_upper_body,
                    reattach,
                    attach_to_bones,
                )
                    .chain(),
            );
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

/// A one-shot clip over the base clip (an attack): on the upper body only while running, on
/// the whole body when standing. Inserting a new one crossfades from the previous one. The
/// clip is sped up (at most `MAX_ACTION_SPEED`) to fit into `max_duration` seconds.
#[derive(Component, Clone)]
pub struct UpperBodyAction {
    pub clip: &'static str,
    pub max_duration: f32,
    pub full_body: bool,
}

const MAX_ACTION_SPEED: f32 = 1.6;
/// Time over which a replaced action fades out.
const RELEASE: f32 = 0.18;

/// Marks a character whose parts get upper-body animation nodes (the hero). Others skip
/// them: the masks cost a third of the frame rate with 500 animated enemies.
#[derive(Component)]
pub struct UpperBodyRig;

/// Bones from here down the hierarchy belong to the upper body.
const UPPER_BODY_ROOT: &str = "Spine1_M";
/// Mask group of all other (lower-body) bones; upper-body nodes mask it out.
const LOWER_BODY_GROUP: u32 = 0;
/// Weight of an upper-body action against the base clip at full strength (blending is
/// normalised, so 20:1 leaves ~5 % of the base clip in the arms).
const UPPER_BODY_WEIGHT: f32 = 20.0;
const FADE_IN: f32 = 0.1;
const FADE_OUT: f32 = 0.25;

/// A static mesh (weapon, prop) carried on one of the character's bones, e.g. a staff on
/// `HandAttach_R`. TQ2 weapon meshes are modelled around their grip, so only a rotation
/// relative to the bone is needed.
#[derive(Clone, PartialEq, Debug)]
pub struct Attachment {
    pub bone: &'static str,
    pub scene: &'static str,
    pub rotation: Quat,
}

/// Everything a character carries on its bones (main hand, off hand). Changing it replaces
/// all attached meshes.
#[derive(Component, Clone, PartialEq)]
pub struct Attachments(pub Vec<Attachment>);

/// The current `Attachments` are spawned on the bones.
#[derive(Component)]
struct AttachmentsDone;

/// The full list of parts a character should wear (body first). Changing it swaps parts:
/// parts no longer listed are removed, new ones are attached (e.g. a different armour set).
#[derive(Component, Clone, PartialEq)]
pub struct DesiredParts(pub Vec<&'static str>);

/// A mesh spawned for an `Attachment`; replaced when the character gets a new attachment.
#[derive(Component)]
struct AttachedMesh {
    root: Entity,
}

/// A part glb waiting for its asset before the scene is spawned; index 0 is the body.
#[derive(Component)]
struct PendingPart(String, usize);

/// Spawned part scene root; `root` is the character entity carrying `CharacterAnim`.
#[derive(Component)]
struct CharacterPart {
    root: Entity,
    path: String,
    index: usize,
}

/// The part's skins already point at the body's joints.
#[derive(Component)]
struct Merged;

/// Links an AnimationPlayer back to its character and part.
#[derive(Component)]
struct PlayerOf {
    root: Entity,
    path: String,
}

struct PartAssets {
    gltf: Handle<Gltf>,
    graph: Option<PartGraph>,
}

/// Every clip of a part as a looping base node (`base`) and, for rigged characters, as action
/// nodes on the upper body (`upper`) and on the whole body (`full`).
struct PartGraph {
    handle: Handle<AnimationGraph>,
    base: HashMap<String, AnimationNodeIndex>,
    upper: HashMap<String, AnimationNodeIndex>,
    full: HashMap<String, AnimationNodeIndex>,
    /// Clip length of every action node.
    durations: HashMap<AnimationNodeIndex, f32>,
}

/// One glTF + one animation graph per part file, shared by every character using it.
#[derive(Resource, Default)]
struct PartLibrary(HashMap<String, PartAssets>);

/// Spawns the parts of a character as children of `root` (which must have a Transform).
pub fn attach_parts(commands: &mut Commands, root: Entity, parts: &[&str]) {
    // Profiling switch: TQ2_BODY_ONLY=1 spawns only the first part of every character.
    let count = if std::env::var_os("TQ2_BODY_ONLY").is_some() { 1 } else { parts.len() };
    for (index, path) in parts[..count].iter().enumerate() {
        let child = commands.spawn((PendingPart(path.to_string(), index), Transform::default(), Visibility::default())).id();
        commands.entity(root).add_child(child);
    }
}

/// Brings a character's parts in line with its `DesiredParts`.
fn sync_parts(
    mut commands: Commands,
    changed: Query<(Entity, &DesiredParts, Option<&Children>), Changed<DesiredParts>>,
    parts: Query<&CharacterPart>,
    pending: Query<&PendingPart>,
) {
    for (root, desired, children) in &changed {
        let mut present = HashSet::new();
        let mut next_index = 0;
        for child in children.into_iter().flatten() {
            let (path, index) = match (parts.get(*child), pending.get(*child)) {
                (Ok(p), _) => (p.path.as_str(), p.index),
                (_, Ok(p)) => (p.0.as_str(), p.1),
                _ => continue,
            };
            next_index = next_index.max(index + 1);
            if desired.0.contains(&path) {
                present.insert(path.to_string());
            } else {
                commands.entity(*child).despawn();
            }
        }
        for path in desired.0.iter().filter(|p| !present.contains(**p)) {
            // The body keeps index 0 so merging knows which skeleton to keep.
            let index = if *path == desired.0[0] { 0 } else { next_index.max(1) };
            next_index = index + 1;
            let child = commands.spawn((PendingPart(path.to_string(), index), Transform::default(), Visibility::default())).id();
            commands.entity(root).add_child(child);
        }
    }
}

fn instantiate_parts(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut library: ResMut<PartLibrary>,
    gltfs: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<GltfNode>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    pending: Query<(Entity, &PendingPart, &ChildOf)>,
    rigs: Query<(), With<UpperBodyRig>>,
) {
    for (entity, part, parent) in &pending {
        let entry = library
            .0
            .entry(part.0.clone())
            .or_insert_with(|| PartAssets { gltf: assets.load(part.0.clone()), graph: None });
        let Some(gltf) = gltfs.get(&entry.gltf) else { continue };
        if entry.graph.is_none() {
            let mut graph = AnimationGraph::new();
            let (mut base, mut upper, mut full, mut durations) = (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());
            // Graphs are shared per part file; a file is either worn by rigged characters or not.
            let rigged = rigs.contains(parent.parent());
            for (name, clip) in &gltf.named_animations {
                base.insert(name.to_string(), graph.add_clip(clip.clone(), 1.0, graph.root));
                if rigged {
                    let duration = clips.get(clip).map_or(1.0, |c| c.duration());
                    let node = graph.add_clip_with_mask(clip.clone(), 1 << LOWER_BODY_GROUP, 1.0, graph.root);
                    upper.insert(name.to_string(), node);
                    durations.insert(node, duration);
                    let node = graph.add_clip(clip.clone(), 1.0, graph.root);
                    full.insert(name.to_string(), node);
                    durations.insert(node, duration);
                }
            }
            if rigged {
                for target in lower_body_targets(gltf, &gltf_nodes) {
                    graph.add_target_to_mask_group(target, LOWER_BODY_GROUP);
                }
            }
            entry.graph = Some(PartGraph { handle: graphs.add(graph), base, upper, full, durations });
        }
        commands
            .entity(entity)
            .remove::<PendingPart>()
            .insert((WorldAssetRoot(gltf.scenes[0].clone()), CharacterPart { root: parent.parent(), path: part.0.clone(), index: part.1 }));
    }
}

/// Animation targets of every bone outside the upper body, with ids computed like Bevy's glTF
/// loader does (names on the path from the scene root to the node).
fn lower_body_targets(gltf: &Gltf, nodes: &Assets<GltfNode>) -> Vec<AnimationTargetId> {
    let all: Vec<&GltfNode> = gltf.nodes.iter().filter_map(|h| nodes.get(h)).collect();
    let children: HashSet<usize> =
        all.iter().flat_map(|n| n.children.iter().filter_map(|c| nodes.get(c)).map(|c| c.index)).collect();
    let mut out = vec![];
    let mut stack: Vec<(&GltfNode, Vec<Name>, bool)> =
        all.iter().filter(|n| !children.contains(&n.index)).map(|n| (*n, vec![], false)).collect();
    while let Some((node, mut path, upper)) = stack.pop() {
        path.push(Name::new(node.name.clone()));
        let upper = upper || node.name == UPPER_BODY_ROOT;
        if !upper {
            out.push(AnimationTargetId::from_names(path.iter()));
        }
        for child in node.children.iter().filter_map(|c| nodes.get(c)) {
            stack.push((child, path.clone(), upper));
        }
    }
    out
}

/// Re-skins each non-body part onto the body's joints (matched by bone name), moves the
/// bones only the part has under their matching body bone, and despawns the duplicates.
fn merge_parts(
    mut commands: Commands,
    pending: Query<(Entity, &CharacterPart), Without<Merged>>,
    all_parts: Query<(Entity, &CharacterPart)>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    mut skins: Query<&mut SkinnedMesh>,
) {
    for (part_entity, part) in &pending {
        if part.index == 0 {
            commands.entity(part_entity).insert(Merged);
            continue;
        }
        let Some((body, _)) = all_parts.iter().find(|(_, p)| p.root == part.root && p.index == 0) else { continue };
        // Wait until both scenes have spawned their skinned meshes.
        let part_skins: Vec<Entity> = children.iter_descendants(part_entity).filter(|e| skins.contains(*e)).collect();
        if part_skins.is_empty() || !children.iter_descendants(body).any(|e| skins.contains(e)) {
            continue;
        }
        let body_bones: HashMap<&str, Entity> =
            children.iter_descendants(body).filter_map(|e| names.get(e).ok().map(|n| (n.as_str(), e))).collect();
        let part_joints: HashSet<Entity> = part_skins.iter().flat_map(|s| skins.get(*s).unwrap().joints.clone()).collect();
        let remap: HashMap<Entity, Entity> = part_joints
            .iter()
            .filter_map(|j| names.get(*j).ok().and_then(|n| body_bones.get(n.as_str())).map(|b| (*j, *b)))
            .collect();

        // Part-only bones keep their own animation but now follow the body's parent bone.
        for joint in part_joints.iter().filter(|j| !remap.contains_key(j)) {
            if let Some(&body_parent) = parents.get(*joint).ok().and_then(|p| remap.get(&p.parent())) {
                commands.entity(*joint).insert(ChildOf(body_parent));
            }
        }
        for skin in &part_skins {
            for joint in skins.get_mut(*skin).unwrap().joints.iter_mut() {
                if let Some(&b) = remap.get(joint) {
                    *joint = b;
                }
            }
        }
        // Duplicates are no longer referenced; despawning them removes their animation work.
        // (Queued after the re-parenting above, so part-only bones survive.)
        for duplicate in remap.keys() {
            commands.entity(*duplicate).try_despawn();
        }
        commands.entity(part_entity).insert(Merged);
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
        let Some(graph) = library.0.get(&part.path).and_then(|p| p.graph.as_ref()) else { continue };
        commands
            .entity(player)
            .insert((AnimationGraphHandle(graph.handle.clone()), PlayerOf { root: part.root, path: part.path.clone() }));
        // Re-apply the current clip so a late part starts in sync with the others.
        if let Ok(mut anim) = anims.get_mut(part.root) {
            anim.set_changed();
        }
    }
}

/// Seconds over which a new base clip (idle, run, attack) blends in and the old one out.
const BASE_BLEND: f32 = 0.2;

/// Base clips of a player that are blending: the new one in, the replaced ones out.
#[derive(Component, Default)]
struct BaseBlend {
    fade_in: Option<(AnimationNodeIndex, f32)>,
    fade_out: HashMap<AnimationNodeIndex, f32>,
}

fn drive_animation(
    mut commands: Commands,
    library: Res<PartLibrary>,
    changed: Query<(Entity, &CharacterAnim), Changed<CharacterAnim>>,
    mut players: Query<(Entity, &PlayerOf, &mut AnimationPlayer, Option<&mut BaseBlend>)>,
) {
    if changed.is_empty() {
        return;
    }
    let wanted: HashMap<Entity, &CharacterAnim> = changed.iter().collect();
    for (entity, of, mut player, blend) in &mut players {
        let Some(anim) = wanted.get(&of.root) else { continue };
        let Some(graph) = library.0.get(&of.path).and_then(|p| p.graph.as_ref()) else { continue };
        let Some(&node) = graph.base.get(anim.clip) else { continue };
        let mut blend = blend.map(|b| std::mem::take(b.into_inner())).unwrap_or_default();
        // The previous base clip fades out (a running upper-body action is left alone).
        let old: Vec<AnimationNodeIndex> =
            player.playing_animations().map(|(n, _)| *n).filter(|n| !graph.durations.contains_key(n) && *n != node).collect();
        let first = old.is_empty() && !player.is_playing_animation(node);
        for n in old {
            blend.fade_out.entry(n).or_insert(BASE_BLEND);
        }
        blend.fade_out.remove(&node);
        let active = player.play(node);
        active.set_speed(anim.speed);
        if anim.repeat {
            active.repeat();
        } else {
            active.replay();
            active.set_repeat(RepeatAnimation::Never);
        }
        // The very first clip starts at full weight; later ones blend in.
        if first {
            active.set_weight(1.0);
            blend.fade_in = None;
        } else {
            active.set_weight(0.0);
            blend.fade_in = Some((node, 0.0));
        }
        commands.entity(entity).insert(blend);
    }
}

/// Crossfades base clips (see `BaseBlend`).
fn blend_base(mut commands: Commands, time: Res<Time>, mut players: Query<(Entity, &mut AnimationPlayer, &mut BaseBlend)>) {
    let dt = time.delta_secs();
    for (entity, mut player, mut blend) in &mut players {
        if let Some((node, t)) = blend.fade_in.as_mut() {
            *t += dt;
            let w = (*t / BASE_BLEND).min(1.0);
            if let Some(active) = player.animation_mut(*node) {
                active.set_weight(w);
            }
            if w >= 1.0 {
                blend.fade_in = None;
            }
        }
        let mut done = vec![];
        for (node, left) in blend.fade_out.iter_mut() {
            *left -= dt;
            match player.animation_mut(*node) {
                Some(active) if *left > 0.0 => {
                    active.set_weight(*left / BASE_BLEND);
                }
                _ => done.push(*node),
            }
        }
        for node in done {
            player.stop(node);
            blend.fade_out.remove(&node);
        }
        if blend.fade_in.is_none() && blend.fade_out.is_empty() {
            commands.entity(entity).remove::<BaseBlend>();
        }
    }
}

/// Action nodes of a player that were replaced and are fading out (node -> seconds left).
#[derive(Component, Default)]
struct Releasing(HashMap<AnimationNodeIndex, f32>);

/// Starts actions and fades them in and out over the base clip; a new action crossfades from
/// the previous one instead of cutting it off.
fn drive_upper_body(
    mut commands: Commands,
    time: Res<Time>,
    library: Res<PartLibrary>,
    started: Query<(Entity, &UpperBodyAction), Changed<UpperBodyAction>>,
    mut players: Query<(Entity, &PlayerOf, &mut AnimationPlayer, Option<&mut Releasing>)>,
) {
    let dt = time.delta_secs();
    let started: HashMap<Entity, &UpperBodyAction> = started.iter().collect();
    for (entity, of, mut player, releasing) in &mut players {
        let Some(graph) = library.0.get(&of.path).and_then(|p| p.graph.as_ref()) else { continue };
        // Only rigged characters (the hero) have action nodes.
        if graph.durations.is_empty() {
            continue;
        }
        let had = releasing.is_some();
        let mut release = releasing.map(|r| std::mem::take(&mut r.into_inner().0)).unwrap_or_default();
        if let Some(action) = started.get(&of.root) {
            let nodes = if action.full_body { &graph.full } else { &graph.upper };
            if let Some(&node) = nodes.get(action.clip) {
                for (n, _) in player.playing_animations() {
                    if graph.durations.contains_key(n) && *n != node {
                        release.entry(*n).or_insert(RELEASE);
                    }
                }
                release.remove(&node);
                let speed = (graph.durations[&node] / action.max_duration).clamp(1.0, MAX_ACTION_SPEED);
                let active = player.play(node);
                active.replay();
                active.set_repeat(RepeatAnimation::Never).set_speed(speed).set_weight(0.0);
            }
        }
        let mut finished = vec![];
        for (node, active) in player.playing_animations_mut() {
            let Some(&duration) = graph.durations.get(node) else { continue };
            let t = active.seek_time();
            let left = release.get_mut(node).map(|r| {
                *r -= dt;
                *r
            });
            if active.is_finished() || t >= duration || left.is_some_and(|l| l <= 0.0) {
                finished.push(*node);
                continue;
            }
            let envelope = (t / FADE_IN).min(1.0) * ((duration - t) / FADE_OUT).clamp(0.0, 1.0) * left.map_or(1.0, |l| l / RELEASE);
            active.set_weight(UPPER_BODY_WEIGHT * envelope);
        }
        for node in finished {
            player.stop(node);
            release.remove(&node);
        }
        if had || !release.is_empty() {
            commands.entity(entity).insert(Releasing(release));
        }
    }
}

/// New `Attachments` have to be spawned again.
fn reattach(mut commands: Commands, changed: Query<Entity, Changed<Attachments>>) {
    for e in &changed {
        commands.entity(e).remove::<AttachmentsDone>();
    }
}

fn attach_to_bones(
    mut commands: Commands,
    assets: Res<AssetServer>,
    pending: Query<(Entity, &Attachments, &Children), Without<AttachmentsDone>>,
    parts: Query<&CharacterPart>,
    children: Query<&Children>,
    names: Query<&Name>,
    attached: Query<(Entity, &AttachedMesh)>,
) {
    for (root, attachments, root_children) in &pending {
        // Only the body keeps its bones after merging, so look there.
        let Some(body) = root_children.iter().find(|c| parts.get(*c).is_ok_and(|p| p.index == 0)) else { continue };
        let bone = |name: &str| children.iter_descendants(body).find(|e| names.get(*e).is_ok_and(|n| n.as_str() == name));
        let Some(bones) = attachments.0.iter().map(|a| bone(a.bone)).collect::<Option<Vec<Entity>>>() else { continue };
        // Replace whatever this character carried before.
        for (old, _) in attached.iter().filter(|(_, a)| a.root == root) {
            commands.entity(old).despawn();
        }
        for (attachment, bone) in attachments.0.iter().zip(bones) {
            let mesh = commands
                .spawn((
                    WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(attachment.scene))),
                    Transform::from_rotation(attachment.rotation),
                    AttachedMesh { root },
                ))
                .id();
            commands.entity(bone).add_child(mesh);
        }
        commands.entity(root).insert(AttachmentsDone);
    }
}
