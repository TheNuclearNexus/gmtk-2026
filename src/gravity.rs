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

/// System to detect when player enters gravitational pull radius and retains full momentum for conservation of angular momentum
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

                // Tangency check: dot product with radial vector must be small (|dot| <= 0.45) to capture into orbit
                let radial_dot = player_heading.dot(to_body).abs();

                if radial_dot <= 0.45 {
                    let diff = player_pos - body_pos;
                    let angle = diff.y.atan2(diff.x);
                    let radial_unit = diff.normalize_or_zero();
                    let tangent_ccw = Vec2::new(-angle.sin(), angle.cos());

                    let v_in = player_heading * player.current_speed;
                    let v_tan = v_in.dot(tangent_ccw);
                    let v_rad = v_in.dot(radial_unit);

                    // Clamp initial inward radial velocity so high approach speeds don't crash into planet surface
                    let initial_radial_vel = v_rad.clamp(-10.0, 20.0);

                    // Conserved angular momentum L = r * v_tan
                    let angular_momentum = distance * v_tan;

                    commands.entity(player_entity).insert(InOrbit {
                        body_entity,
                        radius: distance,
                        angle,
                        angular_momentum,
                        radial_velocity: initial_radial_vel,
                    });

                    break;
                }
            }
        }
    }
}

/// Controls orbital physics with rotational torque force aligning ship facing tangent to the orbit
pub fn orbital_movement_system(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut player_q: Query<(Entity, &mut Transform, &mut Player, &mut InOrbit)>,
    bodies_q: Query<(&Transform, &GravitationalBody), Without<Player>>,
) {
    let dt = time.delta_secs();

    for (player_entity, mut player_transform, mut player, mut orbit) in player_q.iter_mut() {
        if let Ok((body_transform, body_data)) = bodies_q.get(orbit.body_entity) {
            let body_pos = body_transform.translation.truncate();

            // Target tangent heading vector in orbit
            let tangent_heading = Vec2::new(-orbit.angle.sin(), orbit.angle.cos()) * orbit.angular_momentum.signum();
            let target_tangent_z = tangent_heading.y.atan2(tangent_heading.x) - std::f32::consts::FRAC_PI_2;

            // Apply a smooth rotational force to align ship heading towards orbital tangent
            let current_z = player_transform.rotation.to_euler(EulerRot::ZYX).0;
            let current_base_z = current_z - player.tilt;

            let mut angle_diff = target_tangent_z - current_base_z;
            while angle_diff > std::f32::consts::PI { angle_diff -= std::f32::consts::TAU; }
            while angle_diff < -std::f32::consts::PI { angle_diff += std::f32::consts::TAU; }

            // Rotational alignment force towards orbital tangent
            let alignment_force = angle_diff * (6.0 * dt).min(1.0);

            // Manual A / D steering torque input
            let mut manual_rotation = 0.0;
            let mut target_tilt = 0.0;

            if keyboard_input.pressed(KeyCode::KeyA) {
                manual_rotation += player.turn_speed * dt;
                target_tilt += 0.35;
            }
            if keyboard_input.pressed(KeyCode::KeyD) {
                manual_rotation -= player.turn_speed * dt;
                target_tilt -= 0.35;
            }

            // Smooth visual tilt
            player.tilt += (target_tilt - player.tilt) * (12.0 * dt).min(1.0);

            // Apply base heading update incorporating rotational alignment force and manual steering
            let base_heading = current_base_z + alignment_force + manual_rotation;
            player_transform.rotation = Quat::from_rotation_z(base_heading + player.tilt);

            // Ship facing vector
            let ship_facing = Vec2::new(-base_heading.sin(), base_heading.cos());

            // Radial direction pointing outward from planet
            let radial_out = Vec2::new(orbit.angle.cos(), orbit.angle.sin());

            // Radial facing component (facing outward > 0, facing inward < 0)
            let radial_facing = ship_facing.dot(radial_out);

            // Symmetric, responsive radial steering based on ship orientation (facing inward dives inward & speeds up, facing outward expands & slows down)
            let steering_strength = 60.0;
            orbit.radial_velocity += radial_facing * steering_strength * dt;

            // Gentle radial velocity damping so steering feels responsive yet smooth
            orbit.radial_velocity *= 1.0 - (2.0 * dt).min(0.9);

            // Update orbital radius
            orbit.radius += orbit.radial_velocity * dt;
            orbit.radius = orbit.radius.clamp(body_data.body_radius + 10.0, body_data.pull_radius);

            // Conservation of Angular Momentum: v_tan = L / r, angular_velocity = L / (r^2)
            let angular_velocity = orbit.angular_momentum / (orbit.radius * orbit.radius).max(1.0);

            // Update linear speed property (v_tan)
            let current_tan_speed = (orbit.angular_momentum / orbit.radius.max(1.0)).abs();
            player.current_speed = current_tan_speed;

            // Update angle
            orbit.angle += angular_velocity * dt;

            // Update player 2D position
            let new_pos = body_pos + Vec2::new(orbit.angle.cos(), orbit.angle.sin()) * orbit.radius;
            player_transform.translation.x = new_pos.x;
            player_transform.translation.y = new_pos.y;

            // If player steers outward past pull radius, gracefully release from orbit
            if orbit.radius >= body_data.pull_radius {
                commands.entity(player_entity).remove::<InOrbit>();
            }
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

            if dist > 0.0 && dist <= body_data.pull_radius {
                let dir = to_body.normalize();
                let pull_factor = 1.0 - (dist / body_data.pull_radius).clamp(0.0, 1.0);
                let pull_strength = body_data.gravity_force * pull_factor;
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

            // Crash condition (matches 25x25 ship texture hitbox bounds)
            if dist <= body_data.body_radius + 10.0 {
                next_state.set(GameState::GameOver);
                return;
            }
        }
    }
}
