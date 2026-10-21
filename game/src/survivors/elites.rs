//! Elite affixes in the spirit of TQ2's champions: an elite gets one (two after five minutes)
//! of these, shown in its name above it and as a coloured aura.
use bevy::prelude::*;

use super::enemies::{Enemy, EnemySet};
use super::player::{Hero, HeroStats};
use super::{GameState, Health, Pos, RunEntity};
use crate::fx::{Element, Fx, Tex};
use crate::ground::Ground;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EliteMod {
    /// Moves 50 % faster.
    Swift,
    /// Attacks twice as often.
    Frenzied,
    /// A ward absorbs damage worth half its health first.
    Warded,
    /// Explodes shortly after death.
    Volatile,
    /// Splits into two smaller copies when killed.
    Splitting,
}

impl EliteMod {
    const ALL: [EliteMod; 5] = [EliteMod::Swift, EliteMod::Frenzied, EliteMod::Warded, EliteMod::Volatile, EliteMod::Splitting];

    pub fn name(self) -> &'static str {
        match self {
            EliteMod::Swift => "Swift",
            EliteMod::Frenzied => "Frenzied",
            EliteMod::Warded => "Warded",
            EliteMod::Volatile => "Volatile",
            EliteMod::Splitting => "Splitting",
        }
    }

    fn element(self) -> Element {
        match self {
            EliteMod::Swift => Element::Lightning,
            EliteMod::Frenzied => Element::Physical,
            EliteMod::Warded => Element::Cold,
            EliteMod::Volatile => Element::Fire,
            EliteMod::Splitting => Element::Poison,
        }
    }

    /// Random distinct affixes for a new elite.
    pub fn roll(count: usize) -> Vec<EliteMod> {
        let mut all = Self::ALL.to_vec();
        fastrand::shuffle(&mut all);
        all.truncate(count);
        all
    }
}

/// The affixes of an elite.
#[derive(Component, Clone)]
pub struct Elite(pub Vec<EliteMod>);

impl Elite {
    pub fn has(&self, m: EliteMod) -> bool {
        self.0.contains(&m)
    }
}

/// Damage the ward still absorbs, and the health seen last tick.
#[derive(Component)]
pub struct Ward {
    pub left: f32,
    last: f32,
}

impl Ward {
    pub fn new(hp: f32) -> Self {
        Self { left: hp * 0.5, last: hp }
    }
}

/// A Volatile elite's blast: goes off when `timer` runs out.
#[derive(Component)]
struct Blast {
    timer: f32,
    radius: f32,
    damage: f32,
}

/// The name shown above an elite.
#[derive(Component)]
struct EliteLabel(Entity);

pub fn plugin(app: &mut App) {
    // `absorb` runs inside combat's chain, between all damage and the death check.
    app.add_systems(FixedUpdate, tick_blasts.after(EnemySet).run_if(in_state(GameState::Playing)))
        .add_systems(Update, (dress_elites, place_labels));
}

/// The ward takes damage before health does.
pub fn absorb(mut q: Query<(&mut Health, &mut Ward)>) {
    for (mut hp, mut ward) in &mut q {
        if hp.hp < ward.last && ward.left > 0.0 {
            let absorbed = (ward.last - hp.hp).min(ward.left);
            hp.hp += absorbed;
            ward.left -= absorbed;
        }
        ward.last = hp.hp;
    }
}

/// What a dying elite leaves behind (called from combat's death handling).
pub fn on_death(commands: &mut Commands, elite: &Elite, enemy: &Enemy, at: Vec2, scale: f32) {
    if elite.has(EliteMod::Volatile) {
        commands.spawn((Blast { timer: 0.9, radius: 3.2, damage: enemy.damage * 2.5 }, Pos(at), RunEntity));
    }
    if elite.has(EliteMod::Splitting) {
        for side in [-1.0, 1.0] {
            let p = at + Vec2::new(side * 0.8, 0.0);
            super::enemies::spawn_enemy(commands, enemy.def, p, scale, false, false);
        }
    }
}

fn tick_blasts(
    mut commands: Commands,
    time: Res<Time>,
    fx: Res<Fx>,
    flipbook: Res<crate::vfx::Flipbook>,
    ground: Res<Ground>,
    stats: Res<HeroStats>,
    mut mech: ResMut<super::mechanics::Mechanics>,
    mut hero: Query<(&Pos, &mut Health), With<Hero>>,
    mut blasts: Query<(Entity, &Pos, &mut Blast), Without<Hero>>,
) {
    let Ok((hero_pos, mut hero_hp)) = hero.single_mut() else { return };
    for (e, pos, mut b) in &mut blasts {
        // A warning circle on the ground when the blast is placed.
        if b.is_added() {
            let at = Vec3::new(pos.0.x, ground.stand(pos.0), pos.0.y);
            commands.spawn((fx.decal(Tex::Scorch, Element::Fire, at, b.radius, b.timer + 0.3), RunEntity));
        }
        b.timer -= time.delta_secs();
        if b.timer > 0.0 {
            continue;
        }
        let at = Vec3::new(pos.0.x, ground.stand(pos.0) + 1.0, pos.0.y);
        commands.spawn((flipbook.explosion(at, b.radius * 1.6), RunEntity));
        if hero_pos.0.distance(pos.0) < b.radius {
            hero_hp.hp -= super::mechanics::hurt_hero(b.damage, &stats, &mut mech);
        }
        commands.entity(e).despawn();
    }
}

/// Aura and name label for new elites.
fn dress_elites(mut commands: Commands, fx: Res<Fx>, title: Res<crate::fonts::TitleFont>, new: Query<(Entity, &Elite, &Enemy, &super::Radius), Added<Elite>>) {
    for (e, elite, enemy, radius) in &new {
        let element = elite.0.first().map_or(Element::Spirit, |m| m.element());
        commands.entity(e).with_children(|p| {
            // A glowing ring flat on the ground under it (lives as long as the elite).
            p.spawn(fx.decal(Tex::Ring, element, Vec3::ZERO, radius.0 * 2.6, f32::MAX));
            p.spawn(fx.decal(Tex::Glow, element, Vec3::ZERO, radius.0 * 2.2, f32::MAX));
        });
        let names: Vec<&str> = elite.0.iter().map(|m| m.name()).collect();
        commands.spawn((
            EliteLabel(e),
            Text::new(format!("{} {}", names.join(" "), enemy.def.name)),
            title.at(15.0),
            TextColor(Color::srgb(0.88, 0.68, 1.0)),
            TextLayout::justify(Justify::Center),
            Node { position_type: PositionType::Absolute, padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), ..default() },
            BackgroundColor(Color::srgba(0.08, 0.05, 0.1, 0.7)),
            Pickable::IGNORE,
            RunEntity,
        ));
    }
}

fn place_labels(
    mut commands: Commands,
    camera: Query<(&Camera, &GlobalTransform)>,
    targets: Query<(&GlobalTransform, &super::Radius), With<Elite>>,
    mut labels: Query<(Entity, &EliteLabel, &mut Node, &ComputedNode)>,
    ui_scale: Res<UiScale>,
) {
    let Ok((camera, cam)) = camera.single() else { return };
    for (e, label, mut node, computed) in &mut labels {
        let Ok((t, radius)) = targets.get(label.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let head = t.translation() + Vec3::Y * (2.2 + radius.0 * 1.2);
        if let Ok(screen) = camera.world_to_viewport(cam, head) {
            // UI pixels are scaled by UiScale; viewport pixels are not.
            let at = screen / ui_scale.0;
            let size = computed.size() * computed.inverse_scale_factor();
            node.left = Val::Px(at.x - size.x / 2.0);
            node.top = Val::Px(at.y - size.y);
        }
    }
}
