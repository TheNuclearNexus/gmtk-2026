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
pub struct OrbitTrail;

#[derive(Resource, Default)]
pub struct LevelInfo {
    pub current_level: u32,
    pub total_levels: u32,
    pub level_timer: f32,
    pub initial_time: f32,
    pub last_remaining_time: f32,
}
