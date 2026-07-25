use bevy::prelude::*;
use crate::components::*;
use crate::states::GameState;

pub struct GravityPlugin;

impl Plugin for GravityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                gravity_immunity_check_system,
                orbit_entry_system,
                orbital_movement_system,
                gravity_pull_system,
                orbit_collision_system,
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// Checks if player has exited pull radius of a disabled planet to re-enable gravity
pub fn gravity_immunity_check_system(
    mut player_q: Query<(&Transform, &mut Player)>,
    bodies_q: Query<(Entity, &Transform, &GravitationalBody), Without<Player>>,
) {
    for (player_transform, mut player) in player_q.iter_mut() {
        if let Some(disabled_entity) = player.disabled_gravity_body {
            if let Ok((_, body_transform, body_data)) = bodies_q.get(disabled_entity) {
                let p_pos = player_transform.translation.truncate();
                let b_pos = body_transform.translation.truncate();
                let dist = p_pos.distance(b_pos);

                // Re-enable gravity pull once player leaves pull radius
                if dist > body_data.pull_radius {
                    player.disabled_gravity_body = None;
                }
            } else {
                player.disabled_gravity_body = None;
            }
        }
    }
}

/// System to detect when player enters gravitational pull radius and retains incoming velocity for slingshotting
pub fn orbit_entry_system(
    mut commands: Commands,
    player_q: Query<(Entity, &Transform, &Player), Without<InOrbit>>,
    bodies_q: Query<(Entity, &Transform, &GravitationalBody), Without<Player>>,
) {
    for (player_entity, player_transform, player) in player_q.iter() {
        let player_pos = player_transform.translation.truncate();

        // Extract base heading without current visual tilt
        let current_z = player_transform.rotation.to_euler(EulerRot::ZYX).0;
        let base_heading_z = current_z - player.tilt;
        let player_heading = Vec2::new(-base_heading_z.sin(), base_heading_z.cos()).normalize();

        for (body_entity, body_transform, body_data) in bodies_q.iter() {
            // Skip pull/orbit capture if planet is currently disabled by blast
            if player.disabled_gravity_body == Some(body_entity) {
                continue;
            }

            let body_pos = body_transform.translation.truncate();
            let distance = player_pos.distance(body_pos);

            if distance <= body_data.pull_radius && distance > body_data.body_radius {
                let to_body = (body_pos - player_pos).normalize_or_zero();

                // Tangency check: dot product with radial vector must be small (|dot| <= 0.40)
                let radial_dot = player_heading.dot(to_body).abs();

                if radial_dot <= 0.40 {
                    let diff = player_pos - body_pos;
                    let angle = diff.y.atan2(diff.x);

                    let tangent_ccw = Vec2::new(-angle.sin(), angle.cos());

                    // Calculate initial angular velocity derived directly from incoming linear velocity (v_tan / radius)
                    let v_tangential = player_heading.dot(tangent_ccw) * player.current_speed;
                    let initial_angular_velocity = v_tangential / distance.max(10.0);

                    commands.entity(player_entity).insert(InOrbit {
                        body_entity,
                        radius: distance,
                        angle,
                        angular_velocity: initial_angular_velocity,
                        decay_rate: 10.0,
                    });

                    break;
                }
            }
        }
    }
}

/// Controls orbital movement, radius decay towards planet, smooth rotation alignment, and directional A/D orbital speed controls
pub fn orbital_movement_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut player_q: Query<(&mut Transform, &mut Player, &mut InOrbit)>,
    bodies_q: Query<&Transform, (With<GravitationalBody>, Without<Player>)>,
) {
    let dt = time.delta_secs();

    for (mut player_transform, mut player, mut orbit) in player_q.iter_mut() {
        if let Ok(body_transform) = bodies_q.get(orbit.body_entity) {
            let body_pos = body_transform.translation.truncate();
            let orbit_dir = orbit.angular_velocity.signum();

            let mut accel = 0.0;
            let mut target_tilt = 0.0;

            if orbit_dir >= 0.0 {
                // Counter-Clockwise (CCW): Planet is to the left
                // A (towards planet) accelerates, D (away from planet) decelerates
                if keyboard_input.pressed(KeyCode::KeyA) {
                    accel += 3.5;
                    target_tilt += 0.35;
                }
                if keyboard_input.pressed(KeyCode::KeyD) {
                    accel -= 3.5;
                    target_tilt -= 0.35;
                }
            } else {
                // Clockwise (CW): Planet is to the right
                // D (towards planet) accelerates, A (away from planet) decelerates
                if keyboard_input.pressed(KeyCode::KeyD) {
                    accel -= 3.5; // Increases negative magnitude (accelerates CW)
                    target_tilt -= 0.35;
                }
                if keyboard_input.pressed(KeyCode::KeyA) {
                    accel += 3.5; // Decreases negative magnitude (decelerates CW)
                    target_tilt += 0.35;
                }
            }

            // Interpolate visual tilt smoothly
            player.tilt += (target_tilt - player.tilt) * (12.0 * dt).min(1.0);

            orbit.angular_velocity += accel * dt;

            // Preserve orbit direction: never stop (0.0) or reverse direction!
            if orbit_dir >= 0.0 {
                orbit.angular_velocity = orbit.angular_velocity.clamp(1.2, 8.0);
            } else {
                orbit.angular_velocity = orbit.angular_velocity.clamp(-8.0, -1.2);
            }

            // Move player closer to planet over time (radius decay)
            orbit.radius -= orbit.decay_rate * dt;

            // Update angle
            orbit.angle += orbit.angular_velocity * dt;

            // Update player position seamlessly on orbit circle
            let new_pos = body_pos + Vec2::new(orbit.angle.cos(), orbit.angle.sin()) * orbit.radius;
            player_transform.translation.x = new_pos.x;
            player_transform.translation.y = new_pos.y;

            // Target orbital tangent vector
            let move_dir = Vec2::new(-orbit.angle.sin(), orbit.angle.cos()) * orbit_dir;

            // Calculate ship rotation angle matching orbital tangent heading + visual tilt
            let ship_rotation_angle = move_dir.y.atan2(move_dir.x) - std::f32::consts::FRAC_PI_2;
            player_transform.rotation = Quat::from_rotation_z(ship_rotation_angle + player.tilt);
        }
    }
}

/// Applies gravitational acceleration to ship when near but not yet captured in orbit
pub fn gravity_pull_system(
    time: Res<Time>,
    mut player_q: Query<(&mut Transform, &Player), Without<InOrbit>>,
    bodies_q: Query<(Entity, &Transform, &GravitationalBody), Without<Player>>,
) {
    let dt = time.delta_secs();

    for (mut player_transform, player) in player_q.iter_mut() {
        let player_pos = player_transform.translation.truncate();

        for (body_entity, body_transform, body_data) in bodies_q.iter() {
            if player.disabled_gravity_body == Some(body_entity) {
                continue;
            }

            let body_pos = body_transform.translation.truncate();
            let to_body = body_pos - player_pos;
            let dist = to_body.length();

            if dist > 0.0 && dist <= body_data.pull_radius * 1.5 {
                let dir = to_body.normalize();
                let pull_strength = (body_data.gravity_force / (dist * 0.1).max(1.0)).min(120.0);
                player_transform.translation.x += dir.x * pull_strength * dt;
                player_transform.translation.y += dir.y * pull_strength * dt;
            }
        }
    }
}

/// Checks if ship collides with body surface while in orbit or flying, triggering GameOver
pub fn orbit_collision_system(
    mut next_state: ResMut<NextState<GameState>>,
    player_q: Query<&Transform, With<Player>>,
    bodies_q: Query<(&Transform, &GravitationalBody), Without<Player>>,
) {
    for player_transform in player_q.iter() {
        let player_pos = player_transform.translation.truncate();

        for (body_transform, body_data) in bodies_q.iter() {
            let body_pos = body_transform.translation.truncate();
            let dist = player_pos.distance(body_pos);

            // Crash condition
            if dist <= body_data.body_radius + 4.0 {
                next_state.set(GameState::GameOver);
                return;
            }
        }
    }
}
