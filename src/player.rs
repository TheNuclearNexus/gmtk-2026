use bevy::prelude::*;
use crate::components::*;
use crate::states::GameState;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                player_steering_system,
                player_shooting_system,
                blast_update_system,
                gizmo_render_system,
            )
                .run_if(in_state(GameState::Playing)),
        );

        // Anti-jitter: camera tracking runs in PostUpdate schedule after all movement systems
        // using frame-rate independent exponential smoothing.
        app.add_systems(
            PostUpdate,
            camera_follow_system
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// Helper to map screen mouse cursor position to 2D world space
pub fn get_cursor_world_pos(
    windows: &Query<&Window>,
    camera_q: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let window = windows.iter().next()?;
    let (camera, camera_transform) = camera_q.iter().next()?;
    let cursor_pos = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_transform, cursor_pos).ok()
}

/// Camera system running in PostUpdate using frame-rate independent exponential smoothing
pub fn camera_follow_system(
    time: Res<Time>,
    player_q: Query<&Transform, With<Player>>,
    mut camera_q: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
) {
    let dt = time.delta_secs();

    if let Some(player_transform) = player_q.iter().next() {
        if let Some(mut camera_transform) = camera_q.iter_mut().next() {
            let target_x = player_transform.translation.x;
            let target_y = player_transform.translation.y;

            // Exponential decay factor guarantees frame-rate independent smooth tracking without jitter
            let decay_factor = 1.0 - (-16.0 * dt).exp();

            camera_transform.translation.x += (target_x - camera_transform.translation.x) * decay_factor;
            camera_transform.translation.y += (target_y - camera_transform.translation.y) * decay_factor;
        }
    }
}

/// Free flight steering system with dynamic visual tilt and retained launch velocity decay
pub fn player_steering_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut player_q: Query<(&mut Transform, &mut Player), Without<InOrbit>>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut player) in player_q.iter_mut() {
        let mut rotation_amount = 0.0;
        let mut target_tilt = 0.0;

        if keyboard_input.pressed(KeyCode::KeyA) {
            rotation_amount += player.turn_speed * dt;
            target_tilt += 0.35; // Tilt into left turn
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            rotation_amount -= player.turn_speed * dt;
            target_tilt -= 0.35; // Tilt into right turn
        }

        // Smoothly interpolate visual tilt
        player.tilt += (target_tilt - player.tilt) * (12.0 * dt).min(1.0);

        // Extract base heading (removing existing tilt)
        let current_z = transform.rotation.to_euler(EulerRot::ZYX).0;
        let base_heading = current_z - player.tilt + rotation_amount;

        // Apply new rotation with visual tilt included
        transform.rotation = Quat::from_rotation_z(base_heading + player.tilt);

        // Forward motion retaining current orbital launch speed
        let forward = Vec2::new(-base_heading.sin(), base_heading.cos());
        transform.translation.x += forward.x * player.current_speed * dt;
        transform.translation.y += forward.y * player.current_speed * dt;

        // Gradually bleed off high slingshot launch speed back to base cruising speed
        player.current_speed = (player.current_speed - 25.0 * dt).max(player.base_speed);

        // Keep player bounded within extended level boundaries (-260 to 260)
        transform.translation.x = transform.translation.x.clamp(-260.0, 260.0);
        transform.translation.y = transform.translation.y.clamp(-260.0, 260.0);
    }
}

/// Handles blast shooting AWAY from cursor, propelling ship TOWARDS cursor
pub fn player_shooting_system(
    mut commands: Commands,
    mouse_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    bodies_q: Query<(Entity, &Transform, &GravitationalBody), Without<Player>>,
    mut player_q: Query<(Entity, &mut Transform, &mut Player, Option<&InOrbit>)>,
) {
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let cursor_world = get_cursor_world_pos(&windows, &camera_q);

    for (player_entity, mut transform, mut player, in_orbit) in player_q.iter_mut() {
        if player.ammo == 0 {
            continue;
        }

        player.ammo -= 1;
        let player_pos = transform.translation.truncate();

        // Direction towards cursor (ship launch direction)
        let cursor_dir = if let Some(target) = cursor_world {
            let dir = (target - player_pos).normalize_or_zero();
            if dir == Vec2::ZERO {
                let z = transform.rotation.to_euler(EulerRot::ZYX).0 - player.tilt;
                Vec2::new(-z.sin(), z.cos())
            } else {
                dir
            }
        } else {
            let z = transform.rotation.to_euler(EulerRot::ZYX).0 - player.tilt;
            Vec2::new(-z.sin(), z.cos())
        };

        let ship_launch_dir = cursor_dir;
        let projectile_dir = -cursor_dir; // Projectile fires AWAY from cursor

        if let Some(orbit) = in_orbit {
            let current_orbital_speed = (orbit.angular_momentum / orbit.radius.max(1.0)).abs();
            let orbit_dir = Vec2::new(-orbit.angle.sin(), orbit.angle.cos()) * orbit.angular_momentum.signum();

            // Alignment reward: Launching aligned with orbital movement direction gives up to 2.2x speed boost!
            let alignment = ship_launch_dir.dot(orbit_dir).max(0.0);
            let boost_multiplier = 1.0 + 1.2 * alignment;
            let launch_speed = (current_orbital_speed * boost_multiplier).clamp(player.base_speed, 420.0);

            player.current_speed = launch_speed;
            player.disabled_gravity_body = Some(orbit.body_entity);
            commands.entity(player_entity).remove::<InOrbit>();
        } else {
            // Free flight: preserve speed with light penalty
            let launch_speed = (player.current_speed * 0.9).max(player.base_speed);
            player.current_speed = launch_speed;
        }

        // Orient ship facing towards launch direction (towards cursor)
        let ship_rotation_angle = ship_launch_dir.y.atan2(ship_launch_dir.x) - std::f32::consts::FRAC_PI_2;
        transform.rotation = Quat::from_rotation_z(ship_rotation_angle);

        // Spawn Blast projectile traveling AWAY from cursor out of the rear of the ship
        let blast_spawn_pos = (player_pos + projectile_dir * 10.0).extend(transform.translation.z);
        commands.spawn((
            Transform::from_translation(blast_spawn_pos),
            Blast {
                timer: Timer::from_seconds(1.2, TimerMode::Once),
                velocity: projectile_dir * (player.current_speed + 150.0),
            },
        ));

        // Propel ship forward towards cursor
        transform.translation += ship_launch_dir.extend(0.0) * 12.0;

        // If near a planet, disable its gravity pull until player leaves pull radius
        for (body_entity, body_transform, body_data) in bodies_q.iter() {
            let dist = player_pos.distance(body_transform.translation.truncate());
            if dist <= body_data.pull_radius {
                player.disabled_gravity_body = Some(body_entity);
                break;
            }
        }
    }
}

/// Updates blast position and despawns expired blasts
pub fn blast_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut blast_q: Query<(Entity, &mut Transform, &mut Blast)>,
) {
    let dt = time.delta_secs();

    for (entity, mut transform, mut blast) in blast_q.iter_mut() {
        blast.timer.tick(time.delta());
        if blast.timer.just_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        transform.translation.x += blast.velocity.x * dt;
        transform.translation.y += blast.velocity.y * dt;
    }
}

/// Render player ship, blasts, screen-length rear orange firing lines, and clean non-intersecting forward launch arrows
pub fn gizmo_render_system(
    mut gizmos: Gizmos,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    player_q: Query<(&Transform, &Player, Option<&InOrbit>)>,
    blast_q: Query<&Transform, With<Blast>>,
    bodies_q: Query<(Entity, &Transform, &GravitationalBody), Without<Player>>,
) {
    let cursor_world = get_cursor_world_pos(&windows, &camera_q);

    // Draw player ship, targeting line, and post-shot movement arrow
    for (transform, player, in_orbit) in player_q.iter() {
        let pos = transform.translation.truncate();
        let forward = (transform.rotation * Vec3::Y).truncate();
        let right = (transform.rotation * Vec3::X).truncate();

        // 1. Draw Player Spacecraft Triangle centered on transform origin (pos)
        let tip = pos + forward * 7.0;
        let rear = pos - forward * 7.0;
        let left_wing = rear - right * 5.0;
        let right_wing = rear + right * 5.0;

        let color = if in_orbit.is_some() {
            LinearRgba::rgb(0.2, 0.9, 1.0)
        } else if player.disabled_gravity_body.is_some() {
            LinearRgba::rgb(1.0, 0.8, 0.2) // Gold color when gravity immunity active
        } else {
            LinearRgba::rgb(0.0, 0.8, 0.5)
        };

        gizmos.line_2d(tip, left_wing, color);
        gizmos.line_2d(left_wing, right_wing, color);
        gizmos.line_2d(right_wing, tip, color);

        // 2. Trajectory Launch Arrow (towards cursor, skipping ship body)
        let launch_dir = if let Some(target) = cursor_world {
            let dir = (target - pos).normalize_or_zero();
            if dir != Vec2::ZERO { dir } else { forward }
        } else {
            forward
        };

        let ship_radius_offset = 10.0; // Skip ship geometry visually
        let total_arrow_dist = 28.0;   // Shorter arrow
        let arrow_end = pos + launch_dir * total_arrow_dist;
        let arrow_right = Vec2::new(-launch_dir.y, launch_dir.x);
        let launch_color = LinearRgba::rgb(0.2, 1.0, 0.7);

        // Draw dotted launch line skipping ship body
        let dash_len = 4.0;
        let gap_len = 3.0;
        let step = dash_len + gap_len;
        let mut d = ship_radius_offset;

        while d < total_arrow_dist {
            let d_end = (d + dash_len).min(total_arrow_dist);
            let p1 = pos + launch_dir * d;
            let p2 = pos + launch_dir * d_end;
            gizmos.line_2d(p1, p2, launch_color);
            d += step;
        }

        // Arrowhead fins at the end of forward launch line
        let fin_left = arrow_end - launch_dir * 5.0 + arrow_right * 3.5;
        let fin_right = arrow_end - launch_dir * 5.0 - arrow_right * 3.5;
        gizmos.line_2d(arrow_end, fin_left, launch_color);
        gizmos.line_2d(arrow_end, fin_right, launch_color);

        // 3. Orange Dotted Rear Line (skipping ship body, extending away from cursor: 320 units)
        let rear_dir = -launch_dir; // Away from cursor where blast fires
        let total_rear_dist = 320.0; // Extends to edge of screen
        let mut d_rear = ship_radius_offset; // Starts outside ship body
        let orange_color = LinearRgba::rgb(1.0, 0.35, 0.2);

        while d_rear < total_rear_dist {
            let d_end = (d_rear + dash_len).min(total_rear_dist);
            let p1 = pos + rear_dir * d_rear;
            let p2 = pos + rear_dir * d_end;
            gizmos.line_2d(p1, p2, orange_color);
            d_rear += step;
        }
    }

    // Draw blasts as glowing red circles
    for transform in blast_q.iter() {
        let pos = transform.translation.truncate();
        gizmos.circle_2d(pos, 2.5, LinearRgba::rgb(1.0, 0.3, 0.2));
    }

    // Draw gravity objects and their pull radii
    for (entity, transform, body) in bodies_q.iter() {
        let pos = transform.translation.truncate();

        // Check if any player has disabled this body
        let is_disabled = player_q.iter().any(|(_, p, _)| p.disabled_gravity_body == Some(entity));

        let pull_color = if is_disabled {
            LinearRgba::rgb(0.5, 0.5, 0.5) // Dim gray pull field when pull disabled by blast
        } else {
            LinearRgba::rgb(0.3, 0.4, 0.8)
        };

        gizmos.circle_2d(pos, body.pull_radius, pull_color);
        gizmos.circle_2d(pos, body.body_radius, LinearRgba::rgb(0.9, 0.5, 0.2));
    }
}
