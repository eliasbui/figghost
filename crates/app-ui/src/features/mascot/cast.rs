//! The characters: which sheet each one is drawn from and how it moves. Uses nothing from the rest of the
//! app, so `examples/mascot_check.rs` can include it next to `scene.rs`.

use super::scene::{Haul, Style};
use macos_sys::mascot::{Guard, Ray};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Character {
    #[default]
    Spider,
    Frog,
    Ufo,
    Ghost,
    Wizard,
}

impl Character {
    pub const ALL: [Character; 5] = [
        Character::Spider,
        Character::Frog,
        Character::Ufo,
        Character::Ghost,
        Character::Wizard,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Character::Spider => "spider",
            Character::Frog => "frog",
            Character::Ufo => "ufo",
            Character::Ghost => "ghost",
            Character::Wizard => "wizard",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == text)
    }

    pub fn png(self) -> &'static [u8] {
        match self {
            Character::Spider => include_bytes!("../../../assets/mascots/spider.png"),
            Character::Frog => include_bytes!("../../../assets/mascots/frog.png"),
            Character::Ufo => include_bytes!("../../../assets/mascots/ufo.png"),
            Character::Ghost => include_bytes!("../../../assets/mascots/ghost.png"),
            Character::Wizard => include_bytes!("../../../assets/mascots/wizard.png"),
        }
    }

    /// How it reaches, carries and guards. The frog and the wizard were drawn on a finer grid, so they are
    /// drawn smaller to come out the size of the others.
    pub fn style(self) -> Style {
        let (ray, strike, haul, guard, scale) = match self {
            Character::Spider => (Ray::Silk, Ray::Silk, Haul::Tugs, Guard::Box, 2.0),
            Character::Frog => (Ray::Tongue, Ray::Tongue, Haul::Yank, Guard::Box, 1.5),
            Character::Ufo => (Ray::Beam, Ray::Beam, Haul::Float, Guard::Cone, 2.0),
            Character::Ghost => (Ray::Wisp, Ray::Wisp, Haul::Float, Guard::Bubble, 2.0),
            Character::Wizard => (Ray::Magic, Ray::Bolt, Haul::Float, Guard::Ring, 1.5),
        };
        Style {
            ray,
            strike,
            haul,
            guard,
            hangs: self == Character::Spider,
            scale,
        }
    }
}
