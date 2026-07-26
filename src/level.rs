use bevy::prelude::*;
use crate::components::*;
use crate::states::GameState;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelInfo>();
        app.add_systems(OnEnter(GameState::Playing), setup_level_system);
        app.add_systems(OnExit(GameState::Playing), cleanup_level_system);
        app.add_systems(
            Update,
            (update_level_timer_system, check_victory_system, update_pull_radius_sprite_system)
                .run_if(in_state(GameState::Playing)),
        );
    }
}

fn spawn_planet(
    commands: &mut Commands,
    circle_assets: &mut ResMut<CircleAssets>,
    images: &mut ResMut<Assets<Image>>,
    pos: Vec2,
    pull_radius: f32,
    body_radius: f32,
    gravity_force: f32,
) {
    let pull_img = circle_assets.get_or_create(images, pull_radius as u32, Palette::PLUM);
    let body_img = circle_assets.get_or_create(images, body_radius as u32, Palette::MINT);

    let planet_entity = commands
        .spawn((
            GravitationalBody {
                pull_radius,
                body_radius,
                gravity_force,
            },
            Sprite {
                image: body_img,
                ..default()
            },
            Transform::from_translation(pos.extend(0.0)),
            CelestialObject { is_hazard: true },
        ))
        .id();

    commands.spawn((
        PullRadiusSprite { body_entity: planet_entity },
        Sprite {
            image: pull_img,
            ..default()
        },
        Transform::from_translation(pos.extend(-0.1)),
    ));
}

fn spawn_goal_zone(
    commands: &mut Commands,
    circle_assets: &CircleAssets,
    pos: Vec2,
) {
    commands.spawn((
        GoalZone,
        Sprite {
            image: circle_assets.goal_zone.clone(),
            ..default()
        },
        Transform::from_translation(pos.extend(0.0)),
    ));
}

/// Spawns player, planets/asteroids, victory goal beacon, and sets countdown timer
pub fn setup_level_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut circle_assets: ResMut<CircleAssets>,
    mut images: ResMut<Assets<Image>>,
    mut level_info: ResMut<LevelInfo>,
) {
    if circle_assets.goal_zone == Handle::default() {
        circle_assets.goal_zone = create_goal_zone_image(&mut images);
    }
    if circle_assets.aim_reticle == Handle::default() {
        circle_assets.aim_reticle = create_aim_reticle_image(&mut images);
    }

    level_info.total_levels = 5;
    if level_info.current_level == 0 {
        level_info.current_level = 1;
    }

    // Set countdown timer and ammo limits per level
    let (time_limit, initial_ammo, start_pos) = match level_info.current_level {
        1 => (12.0, 5, Vec2::new(-120.0, -120.0)),
        2 => (14.0, 6, Vec2::new(-140.0, 0.0)),
        3 => (12.0, 6, Vec2::new(0.0, -140.0)),
        4 => (14.0, 7, Vec2::new(-140.0, -140.0)),
        _ => (16.0, 8, Vec2::new(-150.0, -150.0)),
    };

    level_info.initial_time = time_limit;
    level_info.level_timer = time_limit;

    // Spawn Player
    commands.spawn((
        Player {
            ammo: initial_ammo,
            max_ammo: initial_ammo,
            turn_speed: 2.4,
            base_speed: 135.0,
            current_speed: 135.0,
            disabled_gravity_body: None,
            tilt: 0.0,
        },
        Sprite {
            image: asset_server.load("ship.png"),
            custom_size: Some(Vec2::new(25.0, 25.0)),
            ..default()
        },
        Transform::from_translation(start_pos.extend(0.0)),
    ));

    // Spawn Aim Reticle sprite
    commands.spawn((
        AimReticleSprite,
        Sprite {
            image: circle_assets.aim_reticle.clone(),
            ..default()
        },
        Transform::from_translation(start_pos.extend(-0.05)),
    ));

    // Spawn Level-specific celestial bodies (asteroids / planets) and Goal Zone
    match level_info.current_level {
        1 => {
            // Level 1: Launch & Basic Thrust. 1 small planet off-path.
            spawn_planet(&mut commands, &mut circle_assets, &mut images, Vec2::new(40.0, -40.0), 50.0, 16.0, 16.0);
            spawn_goal_zone(&mut commands, &circle_assets, Vec2::new(150.0, 150.0));
        }
        2 => {
            // Level 2: Gravitational Slingshot. Single medium planet directly in path.
            spawn_planet(&mut commands, &mut circle_assets, &mut images, Vec2::new(20.0, 0.0), 60.0, 18.0, 18.0);
            spawn_goal_zone(&mut commands, &circle_assets, Vec2::new(190.0, 0.0));
        }
        3 => {
            // Level 3: Orbital Alignment Speed Boost. Heavy center planet.
            spawn_planet(&mut commands, &mut circle_assets, &mut images, Vec2::new(0.0, 0.0), 70.0, 20.0, 20.0);
            spawn_goal_zone(&mut commands, &circle_assets, Vec2::new(-160.0, 160.0));
        }
        4 => {
            // Level 4: Gravity Stun. Massive planet blocking goal entrance.
            spawn_planet(&mut commands, &mut circle_assets, &mut images, Vec2::new(40.0, 40.0), 80.0, 24.0, 22.0);
            spawn_goal_zone(&mut commands, &circle_assets, Vec2::new(180.0, 180.0));
        }
        _ => {
            // Level 5: Chain Slingshot Challenge. 3 planets linked with gentle, controllable gravity (14.0, 12.0, 14.0).
            spawn_planet(&mut commands, &mut circle_assets, &mut images, Vec2::new(-50.0, -30.0), 55.0, 16.0, 14.0);
            spawn_planet(&mut commands, &mut circle_assets, &mut images, Vec2::new(50.0, 55.0), 60.0, 18.0, 12.0); // Gentle 12.0 gravity!
            spawn_planet(&mut commands, &mut circle_assets, &mut images, Vec2::new(155.0, 140.0), 55.0, 16.0, 14.0);
            spawn_goal_zone(&mut commands, &circle_assets, Vec2::new(235.0, 215.0));
        }
    }
}

/// Updates pull radius sprite texture when player disables a body's gravity
pub fn update_pull_radius_sprite_system(
    mut circle_assets: ResMut<CircleAssets>,
    mut images: ResMut<Assets<Image>>,
    player_q: Query<&Player>,
    bodies_q: Query<(Entity, &GravitationalBody)>,
    mut pull_sprite_q: Query<(&PullRadiusSprite, &mut Sprite)>,
) {
    let disabled_body = player_q.iter().find_map(|p| p.disabled_gravity_body);

    for (pull_sprite, mut sprite) in pull_sprite_q.iter_mut() {
        if let Ok((body_entity, body)) = bodies_q.get(pull_sprite.body_entity) {
            let is_disabled = disabled_body == Some(body_entity);
            let color = if is_disabled { Palette::SLATE } else { Palette::PLUM };
            sprite.image = circle_assets.get_or_create(&mut images, body.pull_radius as u32, color);
        }
    }
}

/// Counts down level timer and triggers GameOver when time expires
pub fn update_level_timer_system(
    time: Res<Time>,
    mut level_info: ResMut<LevelInfo>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    level_info.level_timer -= time.delta_secs();

    if level_info.level_timer <= 0.0 {
        level_info.level_timer = 0.0;
        next_state.set(GameState::GameOver);
    }
}

/// Despawns all level gameplay entities when exiting playing state
pub fn cleanup_level_system(
    mut commands: Commands,
    player_q: Query<Entity, With<Player>>,
    bodies_q: Query<Entity, With<GravitationalBody>>,
    pull_sprites_q: Query<Entity, With<PullRadiusSprite>>,
    reticle_q: Query<Entity, With<AimReticleSprite>>,
    goal_q: Query<Entity, With<GoalZone>>,
    blast_q: Query<Entity, With<Blast>>,
) {
    for e in player_q
        .iter()
        .chain(bodies_q.iter())
        .chain(pull_sprites_q.iter())
        .chain(reticle_q.iter())
        .chain(goal_q.iter())
        .chain(blast_q.iter())
    {
        commands.entity(e).despawn();
    }
}

/// Checks if player enters GoalZone to trigger Victory state and records remaining time
pub fn check_victory_system(
    mut next_state: ResMut<NextState<GameState>>,
    mut level_info: ResMut<LevelInfo>,
    player_q: Query<&Transform, With<Player>>,
    goal_q: Query<&Transform, (With<GoalZone>, Without<Player>)>,
) {
    for player_transform in player_q.iter() {
        let p_pos = player_transform.translation.truncate();
        for goal_transform in goal_q.iter() {
            let g_pos = goal_transform.translation.truncate();
            // Victory condition (16.0px goal zone radius + 10.0px ship texture hitbox radius)
            if p_pos.distance(g_pos) <= 26.0 {
                level_info.last_remaining_time = level_info.level_timer;
                if level_info.current_level < level_info.total_levels {
                    level_info.current_level += 1;
                }
                next_state.set(GameState::Victory);
                return;
            }
        }
    }
}
