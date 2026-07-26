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
            )
                .run_if(in_state(GameState::Playing)),
        );

        // Anti-jitter: reticle moves in PostUpdate right before camera follow system
        app.add_systems(
            PostUpdate,
            (update_aim_reticle_system, camera_follow_system)
                .chain()
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
        player.current_speed = (player.current_speed - 20.0 * dt).max(player.base_speed);
    }
}

/// Handles blast shooting AWAY from cursor, propelling ship TOWARDS cursor
pub fn player_shooting_system(
    mut commands: Commands,
    mut circle_assets: ResMut<CircleAssets>,
    mut images: ResMut<Assets<Image>>,
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
    let blast_img = circle_assets.get_or_create(&mut images, 3, Palette::CRIMSON);

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
            let launch_speed = (current_orbital_speed * boost_multiplier).clamp(player.base_speed, 480.0);

            player.current_speed = launch_speed;
            player.disabled_gravity_body = Some(orbit.body_entity);
            commands.entity(player_entity).remove::<InOrbit>();
        } else {
            // Free flight: blast provides immediate +50.0 speed boost!
            let launch_speed = (player.current_speed + 50.0).clamp(player.base_speed, 480.0);
            player.current_speed = launch_speed;
        }

        // Orient ship facing towards launch direction (towards cursor)
        let ship_rotation_angle = ship_launch_dir.y.atan2(ship_launch_dir.x) - std::f32::consts::FRAC_PI_2;
        transform.rotation = Quat::from_rotation_z(ship_rotation_angle);

        // Spawn Blast projectile traveling AWAY from cursor out of the rear of the ship
        let blast_spawn_pos = (player_pos + projectile_dir * 10.0).extend(transform.translation.z);
        commands.spawn((
            Transform::from_translation(blast_spawn_pos),
            Sprite {
                image: blast_img.clone(),
                ..default()
            },
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

/// Updates aim reticle sprite transform, rotation, and visibility based on player position & ammo count
pub fn update_aim_reticle_system(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    player_q: Query<(&Transform, &Player)>,
    mut reticle_q: Query<(&mut Transform, &mut Visibility), (With<AimReticleSprite>, Without<Player>)>,
) {
    let cursor_world = get_cursor_world_pos(&windows, &camera_q);

    if let Some((player_transform, player)) = player_q.iter().next() {
        let player_pos = player_transform.translation.truncate();

        let launch_dir = if let Some(target) = cursor_world {
            let dir = (target - player_pos).normalize_or_zero();
            if dir != Vec2::ZERO {
                dir
            } else {
                (player_transform.rotation * Vec3::Y).truncate()
            }
        } else {
            (player_transform.rotation * Vec3::Y).truncate()
        };

        let angle = launch_dir.y.atan2(launch_dir.x) - std::f32::consts::FRAC_PI_2;
        let target_visibility = if player.ammo == 0 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };

        for (mut reticle_transform, mut visibility) in reticle_q.iter_mut() {
            reticle_transform.translation = player_pos.extend(-0.05);
            reticle_transform.rotation = Quat::from_rotation_z(angle);
            if *visibility != target_visibility {
                *visibility = target_visibility;
            }
        }
    }
}
