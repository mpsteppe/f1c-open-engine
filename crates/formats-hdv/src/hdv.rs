//! The `.hdv` high-detail vehicle parameter file.
//!
//! The file is INI-like (see [`crate::ini`]): `[SECTION]` headers, `Key=Value`
//! lines and `//` comments. Units are SI, engine speed is RPM and angles are
//! degrees; the axes are +x left, +y up, +z rear. Only the keys the engine
//! needs are decoded; the full section list is kept in [`Hdv::ini`].

use crate::ini::{parse_f64, parse_text, parse_tuple3, parse_usize, Ini, Section};

/// Which wheels the driveline drives (`[DRIVELINE] WheelDrive`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelDrive {
    Rear,
    Four,
    Front,
}

/// One corner of the car, used to look up per-wheel values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wheel {
    FrontLeft,
    FrontRight,
    RearLeft,
    RearRight,
}

impl Wheel {
    /// All four corners, front axle first then rear, left before right.
    pub const ALL: [Wheel; 4] = [
        Wheel::FrontLeft,
        Wheel::FrontRight,
        Wheel::RearLeft,
        Wheel::RearRight,
    ];

    /// The matching `[SECTION]` name in the `.hdv` file.
    fn section(self) -> &'static str {
        match self {
            Wheel::FrontLeft => "FRONTLEFT",
            Wheel::FrontRight => "FRONTRIGHT",
            Wheel::RearLeft => "REARLEFT",
            Wheel::RearRight => "REARRIGHT",
        }
    }

    /// A short label for reports: `FL`, `FR`, `RL`, `RR`.
    pub fn label(self) -> &'static str {
        match self {
            Wheel::FrontLeft => "FL",
            Wheel::FrontRight => "FR",
            Wheel::RearLeft => "RL",
            Wheel::RearRight => "RR",
        }
    }
}

/// Parsed `.hdv` file: the generic section list plus typed accessors.
#[derive(Debug, Clone)]
pub struct Hdv {
    pub ini: Ini,
}

impl Hdv {
    /// Parse the whole `.hdv` text.
    pub fn parse(text: &str) -> Self {
        Hdv {
            ini: Ini::parse(text),
        }
    }

    fn section(&self, name: &str) -> Option<&Section> {
        self.ini.section(name)
    }

    /// `[GENERAL] Mass` in kg, the mass without fuel.
    pub fn mass(&self) -> Option<f64> {
        parse_f64(self.section("GENERAL")?.value("Mass")?)
    }

    /// `[GENERAL] Inertia` in kg m^2, without fuel.
    pub fn inertia(&self) -> Option<[f64; 3]> {
        parse_tuple3(self.section("GENERAL")?.value("Inertia")?)
    }

    /// `[GENERAL] CGHeight` in metres above the reference plane.
    pub fn cg_height(&self) -> Option<f64> {
        parse_f64(self.section("GENERAL")?.value("CGHeight")?)
    }

    /// `[GENERAL] TireBrand`, the `.tbc` file name without extension.
    pub fn tire_brand(&self) -> Option<String> {
        let value = parse_text(self.section("GENERAL")?.value("TireBrand")?);
        (!value.is_empty()).then_some(value)
    }

    /// `[GENERAL] TireCompoundSetting`, the compound index inside the brand.
    pub fn tire_compound_setting(&self) -> Option<usize> {
        parse_usize(self.section("GENERAL")?.value("TireCompoundSetting")?)
    }

    /// `[SUSPENSION] PhysicalModelFile`, the `.pm` file name.
    pub fn physical_model_file(&self) -> Option<String> {
        let value = parse_text(self.section("SUSPENSION")?.value("PhysicalModelFile")?);
        (!value.is_empty()).then_some(value)
    }

    /// `[ENGINE] Normal`, the unrestricted engine file name without extension.
    pub fn engine_name(&self) -> Option<String> {
        let value = parse_text(self.section("ENGINE")?.value("Normal")?);
        (!value.is_empty()).then_some(value)
    }

    /// `[DRIVELINE] GearFile`, the gear ratio file name.
    pub fn gear_file(&self) -> Option<String> {
        let value = parse_text(self.section("DRIVELINE")?.value("GearFile")?);
        (!value.is_empty()).then_some(value)
    }

    /// `[DRIVELINE] WheelDrive`: rear, four or front.
    pub fn wheel_drive(&self) -> Option<WheelDrive> {
        let raw = parse_text(self.section("DRIVELINE")?.value("WheelDrive")?);
        match raw.to_ascii_uppercase().as_str() {
            "REAR" => Some(WheelDrive::Rear),
            "FOUR" => Some(WheelDrive::Four),
            "FRONT" => Some(WheelDrive::Front),
            _ => None,
        }
    }

    /// `[DRIVELINE] ForwardGears`, the number of forward gears.
    pub fn forward_gears(&self) -> Option<usize> {
        parse_usize(self.section("DRIVELINE")?.value("ForwardGears")?)
    }

    /// Every `GearNSetting` from 1 up to `ForwardGears`, in gear order.
    ///
    /// Stops at the first missing or malformed setting.
    pub fn gear_settings(&self) -> Vec<usize> {
        let Some(forward) = self.forward_gears() else {
            return Vec::new();
        };
        let Some(section) = self.section("DRIVELINE") else {
            return Vec::new();
        };
        let mut settings = Vec::new();
        for gear in 1..=forward {
            let Some(value) = section.value(&format!("Gear{gear}Setting")) else {
                break;
            };
            let Some(setting) = parse_usize(value) else {
                break;
            };
            settings.push(setting);
        }
        settings
    }

    /// `[DRIVELINE] FinalDriveSetting`, the final drive list index.
    pub fn final_drive_setting(&self) -> Option<usize> {
        parse_usize(self.section("DRIVELINE")?.value("FinalDriveSetting")?)
    }

    /// `[DRIVELINE] ReverseSetting`, the reverse gear list index.
    pub fn reverse_setting(&self) -> Option<usize> {
        parse_usize(self.section("DRIVELINE")?.value("ReverseSetting")?)
    }

    /// `[CONTROLS] SteerLock` in degrees, resolved from its range and setting.
    pub fn steer_lock_deg(&self) -> Option<f64> {
        self.section("CONTROLS")?
            .ranged("SteerLock")
            .map(|r| r.value)
    }

    /// `[WHEEL] BrakeTorque`, maximum brake torque at zero wear, per wheel.
    pub fn brake_torque(&self, wheel: Wheel) -> Option<f64> {
        parse_f64(self.section(wheel.section())?.value("BrakeTorque")?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HDV: &str = "\
[GENERAL]
Mass=750
Inertia=(500.0, 600.0, 120.0)
CGHeight=0.300
TireBrand=comet_tires
TireCompoundSetting=1

[CONTROLS]
SteerLockRange=(4.0, 0.25, 20)
SteerLockSetting=8

[ENGINE]
Normal=comet_v8

[DRIVELINE]
WheelDrive=REAR
GearFile=comet_gears.ini
FinalDriveSetting=1
ReverseSetting=7
ForwardGears=3
Gear1Setting=3
Gear2Setting=23
Gear3Setting=34

[SUSPENSION]
PhysicalModelFile=comet_susp.pm

[FRONTLEFT]
BrakeTorque=2000.0
";

    #[test]
    fn reads_general_values() {
        let hdv = Hdv::parse(HDV);
        assert_eq!(hdv.mass(), Some(750.0));
        assert_eq!(hdv.inertia(), Some([500.0, 600.0, 120.0]));
        assert_eq!(hdv.cg_height(), Some(0.300));
        assert_eq!(hdv.tire_brand().as_deref(), Some("comet_tires"));
        assert_eq!(hdv.tire_compound_setting(), Some(1));
    }

    #[test]
    fn reads_driveline_values() {
        let hdv = Hdv::parse(HDV);
        assert_eq!(hdv.wheel_drive(), Some(WheelDrive::Rear));
        assert_eq!(hdv.gear_file().as_deref(), Some("comet_gears.ini"));
        assert_eq!(hdv.forward_gears(), Some(3));
        assert_eq!(hdv.gear_settings(), vec![3, 23, 34]);
        assert_eq!(hdv.final_drive_setting(), Some(1));
        assert_eq!(hdv.reverse_setting(), Some(7));
    }

    #[test]
    fn reads_linked_names_and_steer_lock() {
        let hdv = Hdv::parse(HDV);
        assert_eq!(hdv.engine_name().as_deref(), Some("comet_v8"));
        assert_eq!(hdv.physical_model_file().as_deref(), Some("comet_susp.pm"));
        assert_eq!(hdv.steer_lock_deg(), Some(6.0));
    }

    #[test]
    fn reads_per_wheel_brake_torque() {
        let hdv = Hdv::parse(HDV);
        assert_eq!(hdv.brake_torque(Wheel::FrontLeft), Some(2000.0));
        assert_eq!(hdv.brake_torque(Wheel::RearRight), None);
    }

    #[test]
    fn gear_settings_stop_at_a_gap() {
        let hdv = Hdv::parse("[DRIVELINE]\nForwardGears=4\nGear1Setting=1\nGear2Setting=2\n");
        assert_eq!(hdv.gear_settings(), vec![1, 2]);
    }

    #[test]
    fn wheel_drive_variants() {
        assert_eq!(
            Hdv::parse("[DRIVELINE]\nWheelDrive=FOUR\n").wheel_drive(),
            Some(WheelDrive::Four)
        );
        assert_eq!(
            Hdv::parse("[DRIVELINE]\nWheelDrive=front\n").wheel_drive(),
            Some(WheelDrive::Front)
        );
        assert_eq!(
            Hdv::parse("[DRIVELINE]\nWheelDrive=SIDE\n").wheel_drive(),
            None
        );
    }
}
