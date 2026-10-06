//! Parsers for the gMotor car physics text files: `.hdv`, the engine `.ini`,
//! the gear ratios `.ini`, the tire brand `.tbc` and the suspension `.pm`.
//!
//! The format is described in `specs/PHYSICS_FILES.md`. This crate only reads
//! and exposes values; it does not simulate the car. Start with
//! [`CarPhysics::load`], which follows the `.veh` -> `.hdv` -> linked files
//! chain and reports missing files instead of failing.

pub mod car;
pub mod engine;
pub mod gears;
pub mod hdv;
pub mod ini;
pub mod pm;
pub mod tbc;

pub use car::{CarPhysics, LoadError, MissingFile};
pub use engine::EngineFile;
pub use gears::GearFile;
pub use hdv::{Hdv, Wheel, WheelDrive};
pub use ini::{Ini, Ranged, Section};
pub use pm::{PmBody, PmFile};
pub use tbc::{Compound, TbcFile};
