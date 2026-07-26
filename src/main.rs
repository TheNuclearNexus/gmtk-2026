use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_rapier2d::prelude::*;

mod components;
mod gravity;
mod level;
mod player;
mod states;
mod ui;

use gravity::GravityPlugin;
use level::LevelPlugin;
use player::PlayerPlugin;
use states::GameState;
use ui::UiPlugin;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Orbital Escape - GMTK Game".into(),
                        resolution: WindowResolution::new(640, 640), // Scales 320x320 render to screen
                        canvas: Some("#bevy".into()),               // WASM target support
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .insert_resource(ClearColor(components::Palette::VOID))
        .init_resource::<components::CircleAssets>()
        .init_state::<GameState>()
        .add_plugins((GravityPlugin, PlayerPlugin, LevelPlugin, UiPlugin))
        .add_systems(Startup, setup_camera)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 320.0,
                height: 320.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
