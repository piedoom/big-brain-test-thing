#![feature(total_cmp)]

use bevy::core::FixedTimestep;
use bevy::pbr::AmbientLight;
use bevy::{prelude::*, render::camera::PerspectiveProjection};
use big_brain::prelude::*;
use rand::random;
fn main() {
    // Once all that's done, we just add our systems and off we go!
    App::build()
        .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
        .add_plugins(DefaultPlugins)
        .add_plugin(BigBrainPlugin)
        .add_startup_system(startup.system())
        .add_system_set(
            SystemSet::default()
                .with_system(PersueAction::run.system())
                .with_system(EatAction::run.system()),
        )
        .add_system_set(
            SystemSet::default()
                .with_system(HungerScorer::run.system())
                .with_system(DistanceScorer::run.system()),
        )
        .add_system_set(
            SystemSet::default()
                .with_system(hunger_tick_fixed.system())
                .with_run_criteria(FixedTimestep::steps_per_second(60.)),
        )
        .add_system(update_targets.system())
        .add_system(cleanup_prey.system())
        .run();
}

// Keeps track of the nearest prey entity to the AI so we only have to calculate it once
pub struct Target(pub Option<Entity>);

fn update_targets(
    targets: Query<(Entity, &mut Target)>,
    mut transforms: Query<&mut Transform>,
    preys: Query<Entity, With<Prey>>,
) {
    targets.for_each_mut(|(e, mut target)| {
        let transform_copy = transforms.get_mut(e).unwrap().clone();
        // get the nearest prey
        let mut prey_transforms_vec: Vec<(Entity, Transform)> = preys
            .iter()
            .map(|e| (e, transforms.get_mut(e).unwrap().clone()))
            .collect();
        prey_transforms_vec.sort_by(|(_entity_a, a), (_entity_b, b)| {
            a.translation
                .distance_squared(transform_copy.translation)
                .total_cmp(&b.translation.distance_squared(transform_copy.translation))
        });

        // set the target to the first entity, if it exists
        target.0 = prey_transforms_vec.first().map(|(e, _)| *e);
    });
}

fn cleanup_prey(mut cmd: Commands, prey: Query<(Entity, &Prey)>) {
    prey.for_each(|(e, p)| {
        if p.points <= 0. {
            cmd.entity(e).despawn_recursive();
        }
    });
}
pub struct Hunger(f32);

impl Hunger {
    pub fn new() -> Self {
        Self(1.0f32)
    }

    pub fn get(&self) -> f32 {
        self.0
    }

    pub fn set(&mut self, value: f32) {
        self.0 = value.clamp(0., 1.);
    }
}

pub struct Prey {
    pub points: f32,
}

impl Default for Prey {
    fn default() -> Self {
        Self { points: 0.9 }
    }
}

fn random_xy(scalar: f32) -> (f32, f32) {
    (
        (random::<f32>() - 0.5) * scalar,
        (random::<f32>() - 0.5) * scalar,
    )
}

fn startup(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ambient_light: ResMut<AmbientLight>,
) {
    // create a top-down camera
    let mut camera = cmd
        .spawn_bundle(PerspectiveCameraBundle {
            perspective_projection: PerspectiveProjection {
                fov: 90f32.to_radians(),
                near: 0.1f32,
                ..Default::default()
            },

            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 24.0))
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..PerspectiveCameraBundle::default()
        })
        .id();

    // create our main AI entity
    let ai = cmd
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube::new(1.0))),
            material: materials.add(Color::rgb(1.0, 0.0, 0.0).into()),
            ..Default::default()
        })
        .id();

    // add the necessary components for our thinker stuff & rendering
    cmd.entity(ai)
        .insert_bundle((Hunger::new(), Target(None)))
        .insert(
            Thinker::build()
                .picker(FirstToScore { threshold: 0.6 })
                .when(
                    AllOrNothing::build(0.8)
                        .push(DistanceScorer::build())
                        .push(HungerScorer::build()),
                    Concurrently::build()
                        .push(EatAction::build())
                        .push(PersueAction::build()),
                )
                .when(HungerScorer::build(), PersueAction::build()),
            // This sometimes breaks in different ways if enabled:
            // .otherwise(RestAction::build()),
        )
        .with_children(|parent| {
            parent.spawn_bundle(LightBundle {
                transform: Transform::from_xyz(0.0, 0.0, 4.0),
                ..Default::default()
            });
        });

    // add some more ai that act as the prey
    for i in 0..10 {
        const SPACING_MULTIPLIER: f32 = 50f32;
        let (x, y) = random_xy(SPACING_MULTIPLIER);
        let prey = cmd
            .spawn_bundle(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Cube::new(1.0))),
                material: materials.add(Color::rgb(0.0, 0.0, 1.0).into()),
                transform: Transform::from_xyz(x, y, 0f32),
                ..Default::default()
            })
            .id();
        cmd.entity(prey).insert_bundle((Prey::default(),));
    }

    ambient_light.color = Color::WHITE;
}

#[derive(Debug, Clone)]
pub struct PersueAction;

impl PersueAction {
    pub fn build() -> PersueActionBuilder {
        PersueActionBuilder
    }
    pub fn run(
        mut state_query: Query<(&Actor, &mut ActionState), With<Self>>,
        targets: Query<&Target>,
        mut transforms: Query<&mut Transform>,
        time: Res<Time>,
    ) {
        for (Actor(actor), mut state) in state_query.iter_mut() {
            match &*state {
                ActionState::Requested => {
                    // move towards target
                    let target = targets.get(*actor).unwrap();
                    let target_transform = transforms.get_mut(target.0.unwrap()).unwrap().clone();
                    let mut transform = transforms.get_mut(*actor).unwrap();

                    let direction =
                        Vec3::from(target_transform.translation - transform.translation)
                            .normalize_or_zero();
                    // apply translation
                    transform.translation += direction * 10.0 * time.delta_seconds();

                    *state = ActionState::Success;
                }
                ActionState::Cancelled => *state = ActionState::Failure,
                _ => (),
            }
        }
    }
}
#[derive(Debug, Clone)]
pub struct PersueActionBuilder;

impl ActionBuilder for PersueActionBuilder {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(PersueAction);
    }
}

#[derive(Debug, Clone)]
pub struct EatAction;

impl EatAction {
    pub fn build() -> EatActionBuilder {
        EatActionBuilder
    }
    pub fn run(
        mut state_query: Query<(&Actor, &mut ActionState), With<Self>>,
        mut hungers: Query<&mut Hunger>,
        targets: Query<&Target>,
        mut preys: Query<(Entity, &mut Prey)>,
        time: Res<Time>,
    ) {
        for (Actor(actor), mut state) in state_query.iter_mut() {
            match &*state {
                ActionState::Requested => {
                    let mut hunger = hungers.get_mut(*actor).unwrap();
                    let current = hunger.get();
                    // deplete hunger at a constate rate. (if this doesn't work, might need to bump up magic value for eating)
                    let points = (time.delta().as_secs_f32() * 20.);
                    // subtract these points from the target
                    let (prey_entity, mut prey) = preys
                        .get_mut(targets.get(*actor).unwrap().0.unwrap())
                        .unwrap();
                    prey.points -= points;
                    hunger.set(current - points);
                    *state = ActionState::Success;
                }
                ActionState::Cancelled => *state = ActionState::Failure,
                _ => (),
            }
        }
    }
}
#[derive(Debug, Clone)]
pub struct EatActionBuilder;

impl ActionBuilder for EatActionBuilder {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(EatAction);
    }
}

#[derive(Debug, Clone)]
pub struct HungerScorer;

impl HungerScorer {
    pub fn build() -> HungerScorerBuilder {
        HungerScorerBuilder
    }
    pub fn run(mut state_query: Query<(&Actor, &mut Score), With<Self>>, hungers: Query<&Hunger>) {
        for (Actor(actor), mut score) in state_query.iter_mut() {
            let hunger = hungers.get(*actor).unwrap();
            score.set(hunger.get());
        }
    }
}

#[derive(Debug, Clone)]
pub struct HungerScorerBuilder;
impl ScorerBuilder for HungerScorerBuilder {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(HungerScorer);
    }
}

#[derive(Debug, Clone)]
pub struct DistanceScorer;

impl DistanceScorer {
    const DISTANCE: f32 = 1.0;
    pub fn build() -> DistanceScorerBuilder {
        DistanceScorerBuilder
    }
    pub fn run(
        mut state_query: Query<(&Actor, &mut Score), With<Self>>,
        transforms: Query<&Transform>,
        prey_transforms: Query<&Transform, With<Prey>>,
    ) {
        for (Actor(actor), mut score) in state_query.iter_mut() {
            // if we're anywhere within our arbitrary distance, set the scorer to 1. otherwise, it is 0.
            let transform = transforms.get(*actor).unwrap();
            let mut in_range: bool = false;
            prey_transforms.for_each(|pt| {
                if transform.translation.distance_squared(pt.translation) <= Self::DISTANCE.powi(2)
                {
                    in_range = true;
                    return;
                }
            });
            score.set(in_range as u8 as f32);
        }
    }
}

#[derive(Debug, Clone)]
pub struct DistanceScorerBuilder;
impl ScorerBuilder for DistanceScorerBuilder {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(DistanceScorer);
    }
}

#[derive(Debug, Clone)]
pub struct RestAction;

impl RestAction {
    pub fn build() -> RestActionBuilder {
        RestActionBuilder
    }
    pub fn run(mut state_query: Query<(&Actor, &mut ActionState), With<Self>>) {
        for (Actor(actor), mut state) in state_query.iter_mut() {
            match &*state {
                ActionState::Requested => {
                    *state = ActionState::Success;
                }
                ActionState::Cancelled => *state = ActionState::Failure,
                _ => (),
            }
        }
    }
}
#[derive(Debug, Clone)]
pub struct RestActionBuilder;

impl ActionBuilder for RestActionBuilder {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(RestAction);
    }
}

/// Add to all hunger values over time.
fn hunger_tick_fixed(hungers: Query<&mut Hunger>) {
    hungers.for_each_mut(|mut hunger| {
        let current = hunger.get();
        hunger.set(current + 0.003);
        info!("Hunger: {}", hunger.get());
    });
}
