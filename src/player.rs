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
        player.current_speed = (player.current_speed - 30.0 * dt).max(player.base_speed);

        // Keep player bounded within extended level boundaries (-260 to 260)
        transform.translation.x = transform.translation.x.clamp(-260.0, 260.0);
        transform.translation.y = transform.translation.y.clamp(-260.0, 260.0);
    }
}

/// Handles free flight recoil shooting and orbital tangential ejection blast
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

        if let Some(orbit) = in_orbit {
            // Orbital blast is unaffected by pointer: ship launches 100% tangentially along orbital heading
            let orbital_linear_speed = (orbit.angular_velocity.abs() * orbit.radius).max(player.base_speed);
            let launch_speed = (orbital_linear_speed * 0.85).max(player.base_speed);

            player.current_speed = launch_speed;
            player.disabled_gravity_body = Some(orbit.body_entity);
            commands.entity(player_entity).remove::<InOrbit>();

            // Ship's exact tangential movement heading vector while in orbit
            let ship_tangent_heading = Vec2::new(-orbit.angle.sin(), orbit.angle.cos()) * orbit.angular_velocity.signum();
            let ship_rotation_angle = ship_tangent_heading.y.atan2(ship_tangent_heading.x) - std::f32::consts::FRAC_PI_2;

            // Orient ship strictly in its orbital tangent heading direction
            transform.rotation = Quat::from_rotation_z(ship_rotation_angle);

            // Spawn Blast projectile traveling forward along orbital tangent heading
            commands.spawn((
                Transform::from_translation(transform.translation),
                Blast {
                    timer: Timer::from_seconds(1.2, TimerMode::Once),
                    velocity: ship_tangent_heading * (launch_speed + 140.0),
                },
            ));

            // Nudge ship forward along tangent heading to break orbit capture
            transform.translation += ship_tangent_heading.extend(0.0) * 12.0;
        } else {
            // Free flight blasting: ship turns away from cursor, blast fires out the back toward cursor
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

            // Preserve free flight speed with 15% penalty
            let penalized_speed = (player.current_speed * 0.85).max(player.base_speed);
            player.current_speed = penalized_speed;

            // Recoil direction: Ship turns and launches AWAY from cursor (-cursor_dir)
            let recoil_dir = -cursor_dir;
            let ship_rotation_angle = recoil_dir.y.atan2(recoil_dir.x) - std::f32::consts::FRAC_PI_2;
            transform.rotation = Quat::from_rotation_z(ship_rotation_angle);

            // Spawn Blast projectile out of the BACK of the ship traveling toward cursor
            let blast_spawn_pos = (player_pos - recoil_dir * 8.0).extend(transform.translation.z);
            commands.spawn((
                Transform::from_translation(blast_spawn_pos),
                Blast {
                    timer: Timer::from_seconds(1.2, TimerMode::Once),
                    velocity: cursor_dir * (penalized_speed + 140.0),
                },
            ));

            // Nudge ship forward in recoil direction
            transform.translation += recoil_dir.extend(0.0) * 12.0;

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

/// Render player ship, blasts, dotted targeting lines, and centered post-shot launch arrows using Bevy Gizmos
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

        // 1. Draw Player Spacecraft Triangle centered on transform origin
        let tip = pos + forward * 7.0;
        let left_wing = pos - forward * 7.0 - right * 5.0;
        let right_wing = pos - forward * 7.0 + right * 5.0;

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

        // 2. Extended Dotted Launch Trajectory Arrow centered directly on ship nose tip
        let launch_dir = if let Some(orbit) = in_orbit {
            Vec2::new(-orbit.angle.sin(), orbit.angle.cos()) * orbit.angular_velocity.signum()
        } else if let Some(target) = cursor_world {
            let cursor_dir = (target - pos).normalize_or_zero();
            if cursor_dir != Vec2::ZERO { -cursor_dir } else { forward }
        } else {
            forward
        };

        let arrow_start = tip; // Starts directly at nose tip
        let total_arrow_dist = 56.0; // Extends farther
        let arrow_end = arrow_start + launch_dir * total_arrow_dist;
        let arrow_right = Vec2::new(-launch_dir.y, launch_dir.x);
        let launch_color = LinearRgba::rgb(0.2, 1.0, 0.7);

        // Draw dotted launch line from ship nose tip
        let dash_len = 4.0;
        let gap_len = 3.0;
        let step = dash_len + gap_len;
        let mut d = 0.0;

        while d < total_arrow_dist {
            let d_end = (d + dash_len).min(total_arrow_dist);
            let p1 = arrow_start + launch_dir * d;
            let p2 = arrow_start + launch_dir * d_end;
            gizmos.line_2d(p1, p2, launch_color);
            d += step;
        }

        // Arrowhead left & right fins at the end of the extended dotted line
        let fin_left = arrow_end - launch_dir * 6.0 + arrow_right * 4.0;
        let fin_right = arrow_end - launch_dir * 6.0 - arrow_right * 4.0;
        gizmos.line_2d(arrow_end, fin_left, launch_color);
        gizmos.line_2d(arrow_end, fin_right, launch_color);

        // 3. Dotted Targeting Line towards cursor while in free flight
        if in_orbit.is_none() {
            if let Some(target) = cursor_world {
                let to_target = target - pos;
                let total_dist = to_target.length();
                if total_dist > 4.0 {
                    let dir = to_target / total_dist;
                    let step = dash_len + gap_len;
                    let mut d_target = 6.0;

                    while d_target < total_dist {
                        let d_end = (d_target + dash_len).min(total_dist);
                        let p1 = pos + dir * d_target;
                        let p2 = pos + dir * d_end;
                        gizmos.line_2d(p1, p2, LinearRgba::rgb(1.0, 0.35, 0.2));
                        d_target += step;
                    }
                }
            }
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
