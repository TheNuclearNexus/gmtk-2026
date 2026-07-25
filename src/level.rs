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
            (update_level_timer_system, check_victory_system, render_level_gizmos)
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// Spawns player, planets/asteroids, victory goal beacon, and sets countdown timer
pub fn setup_level_system(
    mut commands: Commands,
    mut level_info: ResMut<LevelInfo>,
) {
    if level_info.total_levels == 0 {
        level_info.total_levels = 3;
        level_info.current_level = 1;
    }

    // Set countdown timer duration per level
    let time_limit = match level_info.current_level {
        1 => 18.0,
        2 => 22.0,
        _ => 25.0,
    };

    level_info.initial_time = time_limit;
    level_info.level_timer = time_limit;

    // Spawn Player
    commands.spawn((
        Player {
            ammo: 6 + level_info.current_level * 2,
            max_ammo: 6 + level_info.current_level * 2,
            turn_speed: 2.0,
            base_speed: 85.0,
            current_speed: 85.0,
            disabled_gravity_body: None,
            tilt: 0.0,
        },
        Transform::from_xyz(-120.0, -120.0, 0.0),
    ));

    // Spawn Level-specific celestial bodies (asteroids / planets)
    match level_info.current_level {
        1 => {
            // Level 1: One main central gravitational planet
            commands.spawn((
                GravitationalBody {
                    pull_radius: 50.0,
                    body_radius: 18.0,
                    gravity_force: 40.0,
                },
                Transform::from_xyz(0.0, 0.0, 0.0),
                CelestialObject { is_hazard: true },
            ));

            // Goal beacon at top right
            commands.spawn((
                GoalZone,
                Transform::from_xyz(120.0, 120.0, 0.0),
            ));
        }
        2 => {
            // Level 2: Two orbiting planets forming a gravitational binary field
            commands.spawn((
                GravitationalBody {
                    pull_radius: 42.0,
                    body_radius: 15.0,
                    gravity_force: 35.0,
                },
                Transform::from_xyz(-45.0, 20.0, 0.0),
                CelestialObject { is_hazard: true },
            ));

            commands.spawn((
                GravitationalBody {
                    pull_radius: 45.0,
                    body_radius: 16.0,
                    gravity_force: 38.0,
                },
                Transform::from_xyz(45.0, -20.0, 0.0),
                CelestialObject { is_hazard: true },
            ));

            // Goal beacon at top center
            commands.spawn((
                GoalZone,
                Transform::from_xyz(0.0, 130.0, 0.0),
            ));
        }
        _ => {
            // Level 3: Asteroid field challenge
            commands.spawn((
                GravitationalBody {
                    pull_radius: 35.0,
                    body_radius: 12.0,
                    gravity_force: 30.0,
                },
                Transform::from_xyz(-60.0, 50.0, 0.0),
                CelestialObject { is_hazard: true },
            ));

            commands.spawn((
                GravitationalBody {
                    pull_radius: 50.0,
                    body_radius: 20.0,
                    gravity_force: 45.0,
                },
                Transform::from_xyz(0.0, -10.0, 0.0),
                CelestialObject { is_hazard: true },
            ));

            commands.spawn((
                GravitationalBody {
                    pull_radius: 35.0,
                    body_radius: 12.0,
                    gravity_force: 30.0,
                },
                Transform::from_xyz(60.0, 60.0, 0.0),
                CelestialObject { is_hazard: true },
            ));

            // Goal beacon at far top-right
            commands.spawn((
                GoalZone,
                Transform::from_xyz(125.0, 125.0, 0.0),
            ));
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
    goal_q: Query<Entity, With<GoalZone>>,
    blast_q: Query<Entity, With<Blast>>,
) {
    for e in player_q.iter().chain(bodies_q.iter()).chain(goal_q.iter()).chain(blast_q.iter()) {
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
            if p_pos.distance(g_pos) <= 16.0 {
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

/// Render Goal Zone as a green glowing portal/beacon gizmo
pub fn render_level_gizmos(
    mut gizmos: Gizmos,
    goal_q: Query<&Transform, With<GoalZone>>,
) {
    for transform in goal_q.iter() {
        let pos = transform.translation.truncate();
        gizmos.circle_2d(pos, 16.0, LinearRgba::rgb(0.2, 1.0, 0.3));
        gizmos.circle_2d(pos, 10.0, LinearRgba::rgb(0.4, 1.0, 0.5));
        gizmos.circle_2d(pos, 4.0, LinearRgba::rgb(0.8, 1.0, 0.9));
    }
}
