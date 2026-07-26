use bevy::prelude::*;
use crate::components::*;
use crate::states::GameState;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiScale>();
        app.add_systems(Update, update_ui_scale_system);

        app.add_systems(OnEnter(GameState::Title), setup_title_ui);
        app.add_systems(OnExit(GameState::Title), cleanup_ui::<TitleUiNode>);

        app.add_systems(OnEnter(GameState::LevelSelect), setup_level_select_ui);
        app.add_systems(OnExit(GameState::LevelSelect), cleanup_ui::<LevelSelectUiNode>);

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

/// Dynamically updates UiScale based on window height relative to 320px game world PPU
pub fn update_ui_scale_system(
    windows: Query<&Window>,
    mut ui_scale: ResMut<UiScale>,
) {
    if let Some(window) = windows.iter().next() {
        let scale = (window.height() / 320.0).max(0.1);
        if (ui_scale.0 - scale).abs() > 0.001 {
            ui_scale.0 = scale;
        }
    }
}

#[derive(Component)]
pub struct TitleUiNode;

#[derive(Component)]
pub struct LevelSelectUiNode;

#[derive(Component)]
pub struct PlayingUiNode;

#[derive(Component)]
pub struct GameOverUiNode;

#[derive(Component)]
pub struct VictoryUiNode;

#[derive(Component)]
pub struct AmmoBarNode;

#[derive(Component)]
pub struct AmmoSegmentNode {
    pub index: u32,
}

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

pub fn setup_title_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let font = asset_server.load::<Font>("Porto_Buena.otf");

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
            BackgroundColor(Palette::Void.srgba(230)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("ORBITAL ESCAPE"),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::MINT),
            ));

            parent.spawn((
                Text::new("\n[A / D] Turn & Adjust Orbit\n[LEFT CLICK] Blast & Eject Orbit\n"),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::SLATE),
            ));

            parent.spawn((
                Text::new("PRESS [SPACE] OR CLICK FOR LEVEL SELECT"),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::PLUM),
            ));
        });
}

pub fn setup_level_select_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let font = asset_server.load::<Font>("Porto_Buena.otf");

    commands
        .spawn((
            LevelSelectUiNode,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Palette::Void.srgba(235)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("SELECT LEVEL"),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::MINT),
            ));

            let levels = [
                "[1] TUTORIAL 1: LAUNCH & STEER",
                "[2] TUTORIAL 2: GRAVITY SLINGSHOT",
                "[3] TUTORIAL 3: MOMENTUM BOOST",
                "[4] TUTORIAL 4: GRAVITY STUN",
                "[5] CHALLENGE 5: CHAIN SLINGSHOT",
            ];

            for lvl in levels {
                parent.spawn((
                    Text::new(format!("\n{}", lvl)),
                    TextFont {
                        font: font.clone().into(),
                        font_size: FontSize::Px(8.0),
                        ..default()
                    },
                    TextColor(Palette::SLATE),
                ));
            }

            parent.spawn((
                Text::new("\nPRESS [1 - 5] TO START  |  [ESC] MAIN MENU"),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::PLUM),
            ));
        });
}

pub fn setup_playing_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    level_info: Res<LevelInfo>,
) {
    let font = asset_server.load::<Font>("Porto_Buena.otf");
    let ammo_full: Handle<Image> = asset_server.load("ammo_full.png");
    let ammo_empty: Handle<Image> = asset_server.load("ammo_empty.png");

    let initial_ammo: u32 = match level_info.current_level {
        1 => 5,
        2 => 6,
        3 => 6,
        4 => 7,
        _ => 8,
    };
    let max_ammo: u32 = initial_ammo;

    commands
        .spawn((
            PlayingUiNode,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
        ))
        .with_children(|parent| {
            // Header bar (Timer centered at top)
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        TimerTextNode,
                        Text::new("--"),
                        TextFont {
                            font: font.clone().into(),
                            font_size: FontSize::Px(8.0),
                            ..default()
                        },
                        TextColor(Palette::SLATE),
                    ));
                });

            // Ammo Bar Container centered vertically on Right Edge (11x7 px segments, 1px vertical overlap, scaled dynamically via UiScale)
            parent
                .spawn((
                    AmmoBarNode,
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(8.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|bar| {
                    for i in 0..max_ammo {
                        let empty_count = max_ammo.saturating_sub(initial_ammo);
                        let is_full = i >= empty_count && i < max_ammo;
                        let img = if is_full { ammo_full.clone() } else { ammo_empty.clone() };

                        bar.spawn((
                            AmmoSegmentNode { index: i },
                            ImageNode {
                                image: img,
                                ..default()
                            },
                            Node {
                                width: Val::Px(11.0),
                                height: Val::Px(7.0),
                                margin: UiRect {
                                    top: Val::Px(if i == 0 { 0.0 } else { -1.0 }),
                                    ..default()
                                },
                                ..default()
                            },
                        ));
                    }
                });

            // Dynamic level tutorial hint footer
            let tutorial_hint = match level_info.current_level {
                1 => "TUTORIAL 1: Press [A/D] to steer. Left-click to blast away and thrust forward!",
                2 => "TUTORIAL 2: Fly near planets to capture into orbit and slingshot at high speed!",
                3 => "TUTORIAL 3: Blast when aligned with orbit direction for a 2.2x speed boost!",
                4 => "TUTORIAL 4: Blasting near a planet temporarily stuns its gravitational pull!",
                _ => "CHALLENGE: Chain multiple planet orbits for maximum momentum & speed!",
            };

            parent.spawn((
                Text::new(tutorial_hint),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::PLUM),
            ));
        });
}

pub fn update_hud_system(
    asset_server: Res<AssetServer>,
    level_info: Res<LevelInfo>,
    player_q: Query<&Player>,
    mut timer_text_q: Query<(&mut Text, &mut TextColor), With<TimerTextNode>>,
    mut segment_q: Query<(&AmmoSegmentNode, &mut ImageNode)>,
) {
    // Update live countdown timer with discrete palette color steps (PLUM -> SLATE -> MINT)
    for (mut text, mut color) in timer_text_q.iter_mut() {
        **text = format_time(level_info.level_timer);

        let ratio = level_info.level_timer / level_info.initial_time.max(0.1);
        if ratio > 0.5 {
            *color = TextColor(Palette::PLUM);
        } else if ratio > 0.25 {
            *color = TextColor(Palette::SLATE);
        } else {
            *color = TextColor(Palette::MINT);
        }
    }

    // Update visual ammo bar segments (filling bottom-to-top)
    if let Some(player) = player_q.iter().next() {
        let ammo_full: Handle<Image> = asset_server.load("ammo_full.png");
        let ammo_empty: Handle<Image> = asset_server.load("ammo_empty.png");

        let empty_count = player.max_ammo.saturating_sub(player.ammo);

        for (segment, mut image_node) in segment_q.iter_mut() {
            let is_full = segment.index >= empty_count && segment.index < player.max_ammo;
            let target_img = if is_full { ammo_full.clone() } else { ammo_empty.clone() };
            if image_node.image != target_img {
                image_node.image = target_img;
            }
        }
    }
}

pub fn setup_gameover_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    level_info: Res<LevelInfo>,
) {
    let font = asset_server.load::<Font>("Porto_Buena.otf");

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
            BackgroundColor(Palette::Void.srgba(235)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(reason),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::CRIMSON),
            ));

            parent.spawn((
                Text::new("\nPRESS [SPACE] TO RETRY  |  [ESC] LEVEL SELECT"),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::SLATE),
            ));
        });
}

pub fn setup_victory_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    level_info: Res<LevelInfo>,
) {
    let font = asset_server.load::<Font>("Porto_Buena.otf");

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
            BackgroundColor(Palette::Void.srgba(235)),
        ))
        .with_children(|parent| {
            let msg = if level_info.current_level > level_info.total_levels {
                "MISSION COMPLETE! ALL TUTORIALS CLEARED!"
            } else {
                "LEVEL CLEARED!"
            };

            parent.spawn((
                Text::new(msg),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::MINT),
            ));

            parent.spawn((
                Text::new(format!("\nTIME REMAINING: {}", format_time(level_info.last_remaining_time))),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::SLATE),
            ));

            parent.spawn((
                Text::new("\nPRESS [SPACE] TO CONTINUE  |  [ESC] LEVEL SELECT"),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Palette::PLUM),
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
    match current_state.get() {
        GameState::Title => {
            if keyboard_input.just_pressed(KeyCode::Space) || mouse_input.just_pressed(MouseButton::Left) {
                next_state.set(GameState::LevelSelect);
            }
        }
        GameState::LevelSelect => {
            if keyboard_input.just_pressed(KeyCode::Digit1) || keyboard_input.just_pressed(KeyCode::Numpad1) {
                level_info.current_level = 1;
                next_state.set(GameState::Playing);
            } else if keyboard_input.just_pressed(KeyCode::Digit2) || keyboard_input.just_pressed(KeyCode::Numpad2) {
                level_info.current_level = 2;
                next_state.set(GameState::Playing);
            } else if keyboard_input.just_pressed(KeyCode::Digit3) || keyboard_input.just_pressed(KeyCode::Numpad3) {
                level_info.current_level = 3;
                next_state.set(GameState::Playing);
            } else if keyboard_input.just_pressed(KeyCode::Digit4) || keyboard_input.just_pressed(KeyCode::Numpad4) {
                level_info.current_level = 4;
                next_state.set(GameState::Playing);
            } else if keyboard_input.just_pressed(KeyCode::Digit5) || keyboard_input.just_pressed(KeyCode::Numpad5) {
                level_info.current_level = 5;
                next_state.set(GameState::Playing);
            } else if keyboard_input.just_pressed(KeyCode::Escape) {
                next_state.set(GameState::Title);
            }
        }
        GameState::GameOver => {
            if keyboard_input.just_pressed(KeyCode::Space) || mouse_input.just_pressed(MouseButton::Left) {
                next_state.set(GameState::Playing);
            } else if keyboard_input.just_pressed(KeyCode::Escape) {
                next_state.set(GameState::LevelSelect);
            }
        }
        GameState::Victory => {
            if keyboard_input.just_pressed(KeyCode::Space) || mouse_input.just_pressed(MouseButton::Left) {
                if level_info.current_level > level_info.total_levels {
                    level_info.current_level = 1;
                    next_state.set(GameState::LevelSelect);
                } else {
                    next_state.set(GameState::Playing);
                }
            } else if keyboard_input.just_pressed(KeyCode::Escape) {
                next_state.set(GameState::LevelSelect);
            }
        }
        _ => {}
    }
}
