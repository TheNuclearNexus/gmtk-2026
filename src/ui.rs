use bevy::prelude::*;
use crate::components::*;
use crate::states::GameState;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Title), setup_title_ui);
        app.add_systems(OnExit(GameState::Title), cleanup_ui::<TitleUiNode>);

        app.add_systems(OnEnter(GameState::Playing), setup_playing_ui);
        app.add_systems(OnExit(GameState::Playing), cleanup_ui::<PlayingUiNode>);
        app.add_systems(Update, update_hud_system.run_if(in_state(GameState::Playing)));

        app.add_systems(OnEnter(GameState::GameOver), setup_gameover_ui);
        app.add_systems(OnExit(GameState::GameOver), cleanup_ui::<GameOverUiNode>);

        app.add_systems(OnEnter(GameState::Victory), setup_victory_ui);
        app.add_systems(OnExit(GameState::Victory), cleanup_ui::<VictoryUiNode>);

        app.add_systems(
            Update,
            handle_menu_input_system.run_if(not(in_state(GameState::Playing))),
        );
    }
}

#[derive(Component)]
pub struct TitleUiNode;

#[derive(Component)]
pub struct PlayingUiNode;

#[derive(Component)]
pub struct GameOverUiNode;

#[derive(Component)]
pub struct VictoryUiNode;

#[derive(Component)]
pub struct AmmoTextNode;

#[derive(Component)]
pub struct TimerTextNode;

fn cleanup_ui<T: Component>(mut commands: Commands, query: Query<Entity, With<T>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn format_time(secs: f32) -> String {
    let seconds = (secs.max(0.0)) as u32;
    let millis = ((secs.max(0.0) * 100.0) % 100.0) as u32;
    format!("{:02}.{:02}s", seconds, millis)
}

pub fn setup_title_ui(mut commands: Commands) {
    commands
        .spawn((
            TitleUiNode,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.1, 0.9)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("ORBITAL ESCAPE"),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.8, 1.0)),
            ));

            parent.spawn((
                Text::new("\n[A / D] Turn & Adjust Orbit\n[LEFT CLICK] Blast & Eject Orbit\n"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.8, 0.9)),
            ));

            parent.spawn((
                Text::new("PRESS [SPACE] OR CLICK TO START"),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.3, 1.0, 0.5)),
            ));
        });
}

pub fn setup_playing_ui(mut commands: Commands, level_info: Res<LevelInfo>) {
    commands
        .spawn((
            PlayingUiNode,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
        ))
        .with_children(|parent| {
            // Header bar
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        Text::new(format!("LEVEL {} / {}", level_info.current_level, level_info.total_levels)),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(Color::srgb(0.9, 0.9, 1.0)),
                    ));

                    header.spawn((
                        TimerTextNode,
                        Text::new("TIME: --"),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(Color::srgb(1.0, 0.8, 0.2)),
                    ));

                    header.spawn((
                        AmmoTextNode,
                        Text::new("AMMO: --"),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(Color::srgb(0.2, 1.0, 0.4)),
                    ));
                });

            // Footer hint
            parent.spawn((
                Text::new("Reach the Green Beacon before time runs out! Avoid planet surfaces."),
                TextFont { font_size: FontSize::Px(9.0), ..default() },
                TextColor(Color::srgba(0.8, 0.8, 0.8, 0.6)),
            ));
        });
}

pub fn update_hud_system(
    level_info: Res<LevelInfo>,
    player_q: Query<(&Player, Option<&InOrbit>)>,
    mut ammo_text_q: Query<(&mut Text, &mut TextColor), With<AmmoTextNode>>,
    mut timer_text_q: Query<(&mut Text, &mut TextColor), (With<TimerTextNode>, Without<AmmoTextNode>)>,
) {
    // Update live countdown timer
    for (mut text, mut color) in timer_text_q.iter_mut() {
        **text = format!("TIME: {}", format_time(level_info.level_timer));
        if level_info.level_timer <= 5.0 {
            *color = TextColor(Color::srgb(1.0, 0.2, 0.2)); // Warning Red
        } else {
            *color = TextColor(Color::srgb(1.0, 0.8, 0.2)); // Normal Gold
        }
    }

    // Update ammo & orbit status
    if let Some((player, in_orbit)) = player_q.iter().next() {
        for (mut text, mut color) in ammo_text_q.iter_mut() {
            let status = if in_orbit.is_some() { " (ORBITING)" } else { "" };
            **text = format!("AMMO: {}/{}{}", player.ammo, player.max_ammo, status);
            if player.ammo == 0 {
                *color = TextColor(Color::srgb(1.0, 0.2, 0.2));
            } else {
                *color = TextColor(Color::srgb(0.2, 1.0, 0.4));
            }
        }
    }
}

pub fn setup_gameover_ui(mut commands: Commands, level_info: Res<LevelInfo>) {
    let reason = if level_info.level_timer <= 0.0 {
        "TIME EXPIRED!"
    } else {
        "SHIP DESTROYED!"
    };

    commands
        .spawn((
            GameOverUiNode,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.15, 0.02, 0.02, 0.92)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(reason),
                TextFont { font_size: FontSize::Px(22.0), ..default() },
                TextColor(Color::srgb(1.0, 0.2, 0.2)),
            ));

            parent.spawn((
                Text::new("\nPRESS [SPACE] TO RETRY"),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });
}

pub fn setup_victory_ui(mut commands: Commands, level_info: Res<LevelInfo>) {
    commands
        .spawn((
            VictoryUiNode,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.12, 0.04, 0.92)),
        ))
        .with_children(|parent| {
            let msg = if level_info.current_level >= level_info.total_levels {
                "MISSION COMPLETE! ALL LEVELS CLEARED!"
            } else {
                "LEVEL CLEARED!"
            };

            parent.spawn((
                Text::new(msg),
                TextFont { font_size: FontSize::Px(18.0), ..default() },
                TextColor(Color::srgb(0.3, 1.0, 0.5)),
            ));

            parent.spawn((
                Text::new(format!("\nTIME REMAINING: {}", format_time(level_info.last_remaining_time))),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(1.0, 0.8, 0.2)),
            ));

            parent.spawn((
                Text::new("\nPRESS [SPACE] TO CONTINUE"),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });
}

pub fn handle_menu_input_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    current_state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut level_info: ResMut<LevelInfo>,
) {
    let triggered = keyboard_input.just_pressed(KeyCode::Space) || mouse_input.just_pressed(MouseButton::Left);

    if triggered {
        match current_state.get() {
            GameState::Title => {
                level_info.current_level = 1;
                next_state.set(GameState::Playing);
            }
            GameState::GameOver => {
                next_state.set(GameState::Playing);
            }
            GameState::Victory => {
                if level_info.current_level >= level_info.total_levels {
                    level_info.current_level = 1;
                    next_state.set(GameState::Title);
                } else {
                    next_state.set(GameState::Playing);
                }
            }
            _ => {}
        }
    }
}
