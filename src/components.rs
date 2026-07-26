use bevy::prelude::*;

/// Trait for objects that exert gravity
pub trait Gravitational {
    fn pull_radius(&self) -> f32;
    fn body_radius(&self) -> f32;
    fn gravity_strength(&self) -> f32;
}

/// Trait for interactive world elements
pub trait InteractiveElement {
    fn on_player_contact(&self) -> InteractionResult;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionResult {
    DestroyPlayer,
    ReachGoal,
    None,
}

#[derive(Component)]
pub struct Player {
    pub ammo: u32,
    pub max_ammo: u32,
    pub turn_speed: f32, // Slow turning speed
    pub base_speed: f32,
    pub current_speed: f32, // Dynamic flight speed (retains orbital launch speed)
    pub disabled_gravity_body: Option<Entity>, // Planet pull disabled until leaving pull radius
    pub tilt: f32, // Visual lean/tilt in direction of steering input
}

impl Default for Player {
    fn default() -> Self {
        Self {
            ammo: 8,
            max_ammo: 8,
            turn_speed: 2.2, // wide, slow turning
            base_speed: 90.0,
            current_speed: 90.0,
            disabled_gravity_body: None,
            tilt: 0.0,
        }
    }
}

#[derive(Component, Debug, Clone)]
pub struct GravitationalBody {
    pub pull_radius: f32,
    pub body_radius: f32,
    pub gravity_force: f32,
}

impl Gravitational for GravitationalBody {
    fn pull_radius(&self) -> f32 {
        self.pull_radius
    }
    fn body_radius(&self) -> f32 {
        self.body_radius
    }
    fn gravity_strength(&self) -> f32 {
        self.gravity_force
    }
}

#[derive(Component, Debug)]
pub struct InOrbit {
    pub body_entity: Entity,
    pub radius: f32,
    pub angle: f32,
    pub angular_momentum: f32, // L = r * v_tangential (conserved)
    pub radial_velocity: f32,  // dr/dt
}

#[derive(Component)]
pub struct Blast {
    pub timer: Timer,
    pub velocity: Vec2,
}

#[derive(Component)]
pub struct CelestialObject {
    pub is_hazard: bool,
}

#[derive(Component)]
pub struct GoalZone;

#[derive(Component)]
pub struct PullRadiusSprite {
    pub body_entity: Entity,
}

#[derive(Component)]
pub struct AimReticleSprite;

#[derive(Component)]
pub struct OrbitTrail;

#[derive(Resource, Default)]
pub struct LevelInfo {
    pub current_level: u32,
    pub total_levels: u32,
    pub level_timer: f32,
    pub initial_time: f32,
    pub last_remaining_time: f32,
}

/// General 5-Color Palette used across all sprites, UI, gizmos, and backgrounds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    Void,    // #120021 (Dark background)
    Mint,    // #AEEFB9 (Bright primary)
    Slate,   // #727272 (Mid gray)
    Plum,    // #341E44 (Dark purple)
    Crimson, // #FF3333 (Bright red/warning)
}

impl Palette {
    pub const VOID: Color = Color::srgb_u8(0x12, 0x00, 0x21);
    pub const MINT: Color = Color::srgb_u8(0xAE, 0xEF, 0xB9);
    pub const SLATE: Color = Color::srgb_u8(0x72, 0x72, 0x72);
    pub const PLUM: Color = Color::srgb_u8(0x34, 0x1E, 0x44);
    pub const CRIMSON: Color = Color::srgb_u8(0xFF, 0x33, 0x33);

    pub fn color(&self) -> Color {
        match self {
            Palette::Void => Self::VOID,
            Palette::Mint => Self::MINT,
            Palette::Slate => Self::SLATE,
            Palette::Plum => Self::PLUM,
            Palette::Crimson => Self::CRIMSON,
        }
    }

    pub fn srgba(&self, alpha: u8) -> Color {
        match self {
            Palette::Void => Color::srgba_u8(0x12, 0x00, 0x21, alpha),
            Palette::Mint => Color::srgba_u8(0xAE, 0xEF, 0xB9, alpha),
            Palette::Slate => Color::srgba_u8(0x72, 0x72, 0x72, alpha),
            Palette::Plum => Color::srgba_u8(0x34, 0x1E, 0x44, alpha),
            Palette::Crimson => Color::srgba_u8(0xFF, 0x33, 0x33, alpha),
        }
    }
}

use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

/// Resource managing generated pixel-art circle and reticle sprite textures
#[derive(Resource, Default)]
pub struct CircleAssets {
    pub handles: HashMap<(u32, [u8; 4]), Handle<Image>>,
    pub goal_zone: Handle<Image>,
    pub aim_reticle: Handle<Image>,
}

impl CircleAssets {
    pub fn get_or_create(
        &mut self,
        images: &mut Assets<Image>,
        radius: u32,
        color: Color,
    ) -> Handle<Image> {
        let rgba = color.to_srgba();
        let key_color = [
            (rgba.red * 255.0) as u8,
            (rgba.green * 255.0) as u8,
            (rgba.blue * 255.0) as u8,
            (rgba.alpha * 255.0) as u8,
        ];
        let key = (radius, key_color);

        if let Some(handle) = self.handles.get(&key) {
            handle.clone()
        } else {
            let handle = create_pixel_circle_image(images, radius, color);
            self.handles.insert(key, handle.clone());
            handle
        }
    }
}

/// Generates a pixel-perfect 1px thick circle Image texture using Wikipedia's Midpoint Circle Algorithm
pub fn create_pixel_circle_image(
    images: &mut Assets<Image>,
    radius: u32,
    color: Color,
) -> Handle<Image> {
    let r = radius as i32;
    let size = (radius * 2 + 3) as usize;
    let mut data = vec![0u8; size * size * 4];

    let cx = (size / 2) as i32;
    let cy = (size / 2) as i32;

    let rgba = color.to_srgba();
    let r_u8 = (rgba.red * 255.0) as u8;
    let g_u8 = (rgba.green * 255.0) as u8;
    let b_u8 = (rgba.blue * 255.0) as u8;
    let a_u8 = (rgba.alpha * 255.0) as u8;

    let mut plot = |x: i32, y: i32| {
        if x >= 0 && x < size as i32 && y >= 0 && y < size as i32 {
            let idx = ((y as usize) * size + (x as usize)) * 4;
            data[idx] = r_u8;
            data[idx + 1] = g_u8;
            data[idx + 2] = b_u8;
            data[idx + 3] = a_u8;
        }
    };

    let mut x = r;
    let mut y = 0;
    let mut decision_over_2 = 1 - x;

    while x >= y {
        plot(cx + x, cy + y);
        plot(cx + y, cy + x);
        plot(cx - y, cy + x);
        plot(cx - x, cy + y);
        plot(cx - x, cy - y);
        plot(cx - y, cy - x);
        plot(cx + y, cy - x);
        plot(cx + x, cy - y);

        y += 1;
        if decision_over_2 <= 0 {
            decision_over_2 += 2 * y + 1;
        } else {
            x -= 1;
            decision_over_2 += 2 * (y - x) + 1;
        }
    }

    let extent = Extent3d {
        width: size as u32,
        height: size as u32,
        depth_or_array_layers: 1,
    };

    let image = Image::new(
        extent,
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );

    images.add(image)
}

/// Generates concentric goal zone pixel-art circles into a single Image texture
pub fn create_goal_zone_image(
    images: &mut Assets<Image>,
) -> Handle<Image> {
    let max_r = 16i32;
    let size = (max_r * 2 + 3) as usize;
    let mut data = vec![0u8; size * size * 4];

    let cx = (size / 2) as i32;
    let cy = (size / 2) as i32;

    let rgba = Palette::MINT.to_srgba();
    let r_u8 = (rgba.red * 255.0) as u8;
    let g_u8 = (rgba.green * 255.0) as u8;
    let b_u8 = (rgba.blue * 255.0) as u8;
    let a_u8 = (rgba.alpha * 255.0) as u8;

    let mut plot = |x: i32, y: i32| {
        if x >= 0 && x < size as i32 && y >= 0 && y < size as i32 {
            let idx = ((y as usize) * size + (x as usize)) * 4;
            data[idx] = r_u8;
            data[idx + 1] = g_u8;
            data[idx + 2] = b_u8;
            data[idx + 3] = a_u8;
        }
    };

    for &r in &[16i32, 10i32, 4i32] {
        let mut x = r;
        let mut y = 0;
        let mut decision_over_2 = 1 - x;

        while x >= y {
            plot(cx + x, cy + y);
            plot(cx + y, cy + x);
            plot(cx - y, cy + x);
            plot(cx - x, cy + y);
            plot(cx - x, cy - y);
            plot(cx - y, cy - x);
            plot(cx + y, cy - x);
            plot(cx + x, cy - y);

            y += 1;
            if decision_over_2 <= 0 {
                decision_over_2 += 2 * y + 1;
            } else {
                x -= 1;
                decision_over_2 += 2 * (y - x) + 1;
            }
        }
    }

    let extent = Extent3d {
        width: size as u32,
        height: size as u32,
        depth_or_array_layers: 1,
    };

    let image = Image::new(
        extent,
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );

    images.add(image)
}

/// Generates a pixel-art aiming arrow & trajectory line sprite Image
pub fn create_aim_reticle_image(
    images: &mut Assets<Image>,
) -> Handle<Image> {
    let width = 15usize;
    let height = 240usize;
    let mut data = vec![0u8; width * height * 4];

    let cx = (width / 2) as i32;
    let cy = (height / 2) as i32;

    let mint = Palette::MINT.to_srgba();
    let mint_u8 = [
        (mint.red * 255.0) as u8,
        (mint.green * 255.0) as u8,
        (mint.blue * 255.0) as u8,
        (mint.alpha * 255.0) as u8,
    ];

    let slate = Palette::SLATE.to_srgba();
    let slate_u8 = [
        (slate.red * 255.0) as u8,
        (slate.green * 255.0) as u8,
        (slate.blue * 255.0) as u8,
        (slate.alpha * 255.0) as u8,
    ];

    let mut plot = |dx: i32, dy: i32, color_bytes: [u8; 4]| {
        let x = cx + dx;
        let y = cy - dy; // Invert Y so +dy is UP (+Y in 2D world space)
        if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
            let idx = ((y as usize) * width + (x as usize)) * 4;
            data[idx] = color_bytes[0];
            data[idx + 1] = color_bytes[1];
            data[idx + 2] = color_bytes[2];
            data[idx + 3] = color_bytes[3];
        }
    };

    // 1. Forward dotted line (+Y) in MINT
    let mut dy = 14i32;
    let max_forward = 35i32;
    while dy < max_forward {
        for step in 0..4 {
            if dy + step < max_forward {
                plot(0, dy + step, mint_u8);
            }
        }
        dy += 7;
    }

    // 2. Arrowhead fins at top (+Y) in MINT
    let tip_y = max_forward;
    plot(0, tip_y, mint_u8);
    plot(-1, tip_y - 1, mint_u8);
    plot(1, tip_y - 1, mint_u8);
    plot(-2, tip_y - 2, mint_u8);
    plot(2, tip_y - 2, mint_u8);
    plot(-3, tip_y - 3, mint_u8);
    plot(3, tip_y - 3, mint_u8);

    // 3. Rear dotted line (-Y) in SLATE
    let mut ry = 14i32;
    let max_rear = cy - 2;
    while ry < max_rear {
        for step in 0..4 {
            if ry + step < max_rear {
                plot(0, -(ry + step), slate_u8);
            }
        }
        ry += 7;
    }

    let extent = Extent3d {
        width: width as u32,
        height: height as u32,
        depth_or_array_layers: 1,
    };

    let image = Image::new(
        extent,
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );

    images.add(image)
}
