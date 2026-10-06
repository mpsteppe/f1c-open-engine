//! Validated driving configuration derived from a loaded [`CarPhysics`].
//!
//! Everything the fixed-step model needs is checked here once, before the
//! viewer opens, so a bad car fails with one clear message instead of a silent
//! fallback. The numbers come from the game text files parsed by `formats-hdv`;
//! no value is invented for a required field.

use formats_hdv::ini::parse_usize;
use formats_hdv::{CarPhysics, Wheel, WheelDrive};

/// Highest number of forward gears the prototype accepts.
pub const MAX_FORWARD_GEARS: usize = 20;
/// Steering lock must stay below this many degrees.
pub const MAX_STEER_LOCK_DEG: f64 = 80.0;

/// A configuration problem, naming the file or field that must be fixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub message: String,
}

impl ConfigError {
    /// Build an error from any message.
    pub fn new(message: impl Into<String>) -> Self {
        ConfigError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Where and how the car starts on the selected grid slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spawn {
    /// Grid `Pos` X, the mesh origin in game axes.
    pub origin_x: f64,
    /// Grid `Pos` Z, the mesh origin in game axes.
    pub origin_z: f64,
    /// Grid `Ori` Y yaw in radians (game sign; positive turns right).
    pub yaw: f64,
    /// Grid `Pos` Y, the height of the flat ground plane.
    pub height: f64,
}

/// Every validated value the fixed-step model reads, in SI units.
#[derive(Debug, Clone, PartialEq)]
pub struct CarSpec {
    /// Total mass in kg.
    pub mass: f64,
    /// Resolved forward gear ratios, in gear order.
    pub forward_gears: Vec<f64>,
    /// Final drive ratio (bevel times the selected final ratio).
    pub final_drive: f64,
    /// `(rpm, min_torque, max_torque)` samples, source order, strictly rising RPM.
    pub torque: Vec<(f64, f64, f64)>,
    /// Rev limit in RPM.
    pub rev_limit: f64,
    /// Prototype idle speed in RPM (mean of the idle band).
    pub idle_rpm: f64,
    /// Front tire radius in metres.
    pub front_radius: f64,
    /// Rear tire radius in metres.
    pub rear_radius: f64,
    /// Steering lock in radians.
    pub steer_lock_rad: f64,
    /// Brake torque per corner, `[FL, FR, RL, RR]`, in Nm.
    pub brake_torque: [f64; 4],
    /// Wheelbase in metres (rear axle Z minus front axle Z).
    pub wheelbase: f64,
    /// Rear-axle local centre `(x, z)` in game axes.
    pub rear_axle: [f64; 2],
}

impl CarSpec {
    /// Validate a loaded car and return its driving configuration.
    ///
    /// The error names the missing link, bad field or unsupported value.
    pub fn from_physics(physics: &CarPhysics) -> Result<Self, ConfigError> {
        let hdv = physics
            .hdv
            .as_ref()
            .ok_or_else(|| ConfigError::new("missing HDV file (HDVehicle link)"))?;
        let engine = physics
            .engine
            .as_ref()
            .ok_or_else(|| ConfigError::new("missing engine .ini file ([ENGINE] Normal link)"))?;
        let gears = physics.gears.as_ref().ok_or_else(|| {
            ConfigError::new("missing gear ratios .ini file ([DRIVELINE] GearFile link)")
        })?;
        let tires = physics
            .tires
            .as_ref()
            .ok_or_else(|| ConfigError::new("missing tire .tbc file ([GENERAL] TireBrand link)"))?;
        let suspension = physics.suspension.as_ref().ok_or_else(|| {
            ConfigError::new("missing suspension .pm file ([SUSPENSION] PhysicalModelFile link)")
        })?;

        let mass = finite_positive(hdv.mass(), "HDV [GENERAL] Mass (kg)")?;

        match hdv.wheel_drive() {
            Some(WheelDrive::Rear) => {}
            Some(WheelDrive::Front) => {
                return Err(ConfigError::new(
                    "unsupported drivetrain WheelDrive=FRONT; prototype supports REAR only",
                ))
            }
            Some(WheelDrive::Four) => {
                return Err(ConfigError::new(
                    "unsupported drivetrain WheelDrive=FOUR; prototype supports REAR only",
                ))
            }
            None => {
                return Err(ConfigError::new(
                    "missing or invalid HDV [DRIVELINE] WheelDrive (REAR, FOUR or FRONT)",
                ))
            }
        }

        let declared = hdv
            .forward_gears()
            .ok_or_else(|| ConfigError::new("missing HDV [DRIVELINE] ForwardGears"))?;
        if declared == 0 || declared > MAX_FORWARD_GEARS {
            return Err(ConfigError::new(format!(
                "HDV ForwardGears={declared} out of range (1..={MAX_FORWARD_GEARS})"
            )));
        }
        let driveline = hdv
            .ini
            .section("DRIVELINE")
            .ok_or_else(|| ConfigError::new("missing HDV [DRIVELINE] section"))?;
        let mut forward_gears = Vec::with_capacity(declared);
        for gear in 1..=declared {
            let key = format!("Gear{gear}Setting");
            let raw = driveline
                .value(&key)
                .ok_or_else(|| ConfigError::new(format!("missing HDV [DRIVELINE] {key}")))?;
            let index = parse_usize(raw)
                .ok_or_else(|| ConfigError::new(format!("invalid HDV [DRIVELINE] {key}={raw}")))?;
            let ratio = gears.gear_ratio(index).ok_or_else(|| {
                ConfigError::new(format!(
                    "HDV {key}={index} out of range of the gear list ({} entries)",
                    gears.gear_pairs.len()
                ))
            })?;
            if !ratio.is_finite() || ratio <= 0.0 {
                return Err(ConfigError::new(format!(
                    "HDV {key}={index} resolves to a non-positive gear ratio"
                )));
            }
            forward_gears.push(ratio);
        }

        let final_setting = hdv
            .final_drive_setting()
            .ok_or_else(|| ConfigError::new("missing HDV [DRIVELINE] FinalDriveSetting"))?;
        let final_drive = gears.final_drive(final_setting).ok_or_else(|| {
            ConfigError::new(format!(
                "HDV FinalDriveSetting={final_setting} out of range of the final-drive list"
            ))
        })?;
        if !final_drive.is_finite() || final_drive <= 0.0 {
            return Err(ConfigError::new(
                "final drive resolves to a non-positive ratio",
            ));
        }

        if engine.rpm_torque_invalid > 0 {
            return Err(ConfigError::new(format!(
                "engine has {} malformed RPMTorque row(s); no row is dropped silently",
                engine.rpm_torque_invalid
            )));
        }
        if engine.rpm_torque.len() < 2 {
            return Err(ConfigError::new("engine needs at least two RPMTorque rows"));
        }
        let mut previous_rpm = -1.0;
        for (row, &(rpm, min_torque, max_torque)) in engine.rpm_torque.iter().enumerate() {
            if !rpm.is_finite() || !min_torque.is_finite() || !max_torque.is_finite() {
                return Err(ConfigError::new(format!(
                    "engine RPMTorque row {row} has a non-finite value"
                )));
            }
            if rpm < 0.0 {
                return Err(ConfigError::new(format!(
                    "engine RPMTorque row {row} has a negative RPM"
                )));
            }
            if rpm <= previous_rpm {
                return Err(ConfigError::new(format!(
                    "engine RPMTorque RPM must strictly increase (row {row}, {rpm} RPM)"
                )));
            }
            previous_rpm = rpm;
        }

        let rev_limit = finite_positive(engine.rev_limit, "engine rev limit (RPM)")?;
        let (idle_low, idle_high) = engine
            .idle_rpm
            .ok_or_else(|| ConfigError::new("missing engine IdleRPMLogic (low, high)"))?;
        if !idle_low.is_finite()
            || !idle_high.is_finite()
            || idle_low <= 0.0
            || idle_low > idle_high
        {
            return Err(ConfigError::new(format!(
                "engine IdleRPMLogic=({idle_low}, {idle_high}) must satisfy 0 < low <= high"
            )));
        }
        if idle_high >= rev_limit {
            return Err(ConfigError::new(format!(
                "engine idle high {idle_high} RPM must be below the rev limit {rev_limit} RPM"
            )));
        }
        let idle_rpm = (idle_low + idle_high) / 2.0;

        let compound_index = hdv
            .tire_compound_setting()
            .ok_or_else(|| ConfigError::new("missing HDV [GENERAL] TireCompoundSetting"))?;
        let compound = tires.compound(compound_index).ok_or_else(|| {
            ConfigError::new(format!(
                "HDV TireCompoundSetting={compound_index} out of range of the .tbc compounds"
            ))
        })?;
        let front_radius = finite_positive(compound.front_radius, "tire Front: Radius (m)")?;
        let rear_radius = finite_positive(compound.rear_radius, "tire Rear: Radius (m)")?;

        let steer_lock_deg = hdv
            .steer_lock_deg()
            .ok_or_else(|| ConfigError::new("missing HDV [CONTROLS] SteerLock range/setting"))?;
        if !steer_lock_deg.is_finite()
            || steer_lock_deg <= 0.0
            || steer_lock_deg >= MAX_STEER_LOCK_DEG
        {
            return Err(ConfigError::new(format!(
                "HDV SteerLock={steer_lock_deg} degrees must be positive and below {MAX_STEER_LOCK_DEG}"
            )));
        }

        let mut brake_torque = [0.0; 4];
        let mut any_positive = false;
        for (slot, wheel) in Wheel::ALL.iter().enumerate() {
            let value = hdv.brake_torque(*wheel).ok_or_else(|| {
                ConfigError::new(format!("missing HDV [{}] BrakeTorque", wheel.label()))
            })?;
            if !value.is_finite() || value < 0.0 {
                return Err(ConfigError::new(format!(
                    "HDV [{}] BrakeTorque={value} must be finite and non-negative",
                    wheel.label()
                )));
            }
            any_positive |= value > 0.0;
            brake_torque[slot] = value;
        }
        if !any_positive {
            return Err(ConfigError::new("all four HDV BrakeTorque values are zero"));
        }

        reject_wheelbase_override(hdv, "LeftWheelBase")?;
        reject_wheelbase_override(hdv, "RightWheelBase")?;

        let wheel = |name: &str| -> Result<[f64; 3], ConfigError> {
            let position = suspension
                .body(name)
                .and_then(|body| body.pos)
                .ok_or_else(|| {
                    ConfigError::new(format!("missing PM [BODY] {name} with a pos=(x, y, z)"))
                })?;
            if !position.iter().all(|value| value.is_finite()) {
                return Err(ConfigError::new(format!(
                    "PM [BODY] {name} pos must contain finite coordinates"
                )));
            }
            Ok(position)
        };
        let front_left = wheel("fl_wheel")?;
        let front_right = wheel("fr_wheel")?;
        let rear_left = wheel("rl_wheel")?;
        let rear_right = wheel("rr_wheel")?;

        let front_z = (front_left[2] + front_right[2]) / 2.0;
        let rear_x = (rear_left[0] + rear_right[0]) / 2.0;
        let rear_z = (rear_left[2] + rear_right[2]) / 2.0;
        let wheelbase = rear_z - front_z;
        if !rear_x.is_finite() || !rear_z.is_finite() {
            return Err(ConfigError::new("PM rear axle centre is not finite"));
        }
        if !wheelbase.is_finite() || wheelbase <= 0.0 {
            return Err(ConfigError::new(format!(
                "PM wheelbase (rear axle Z {rear_z} - front axle Z {front_z}) must be finite positive"
            )));
        }

        let spec = CarSpec {
            mass,
            forward_gears,
            final_drive,
            torque: engine.rpm_torque.clone(),
            rev_limit,
            idle_rpm,
            front_radius,
            rear_radius,
            steer_lock_rad: steer_lock_deg.to_radians(),
            brake_torque,
            wheelbase,
            rear_axle: [rear_x, rear_z],
        };
        // A linear curve can be positive inside the operating band even when
        // neither source sample lies in it. Include the interpolated boundaries.
        let positive_torque = spec.torque_at(idle_rpm).1 > 0.0
            || spec.torque_at(rev_limit).1 > 0.0
            || spec
                .torque
                .iter()
                .any(|&(rpm, _, torque)| rpm >= idle_rpm && rpm <= rev_limit && torque > 0.0);
        if !positive_torque {
            return Err(ConfigError::new(
                "engine has no positive max torque between idle and the rev limit",
            ));
        }
        Ok(spec)
    }

    /// Linearly interpolate `(min_torque, max_torque)` at `rpm`.
    ///
    /// Lookup is clamped to the curve endpoints.
    pub fn torque_at(&self, rpm: f64) -> (f64, f64) {
        let first = self.torque[0];
        if rpm <= first.0 {
            return (first.1, first.2);
        }
        let last = self.torque[self.torque.len() - 1];
        if rpm >= last.0 {
            return (last.1, last.2);
        }
        for pair in self.torque.windows(2) {
            let (r0, min0, max0) = pair[0];
            let (r1, min1, max1) = pair[1];
            if rpm >= r0 && rpm <= r1 {
                let factor = (rpm - r0) / (r1 - r0);
                return (min0 + (min1 - min0) * factor, max0 + (max1 - max0) * factor);
            }
        }
        (last.1, last.2)
    }

    /// Number of forward gears.
    pub fn gear_count(&self) -> usize {
        self.forward_gears.len()
    }
}

/// A required value must be present, finite and strictly positive.
fn finite_positive(value: Option<f64>, what: &str) -> Result<f64, ConfigError> {
    match value {
        Some(number) if number.is_finite() && number > 0.0 => Ok(number),
        Some(number) => Err(ConfigError::new(format!(
            "{what} is not a finite positive number (got {number})"
        ))),
        None => Err(ConfigError::new(format!("missing {what}"))),
    }
}

/// Reject a non-zero wheelbase override; prototype cannot honour it.
fn reject_wheelbase_override(hdv: &formats_hdv::Hdv, key: &str) -> Result<(), ConfigError> {
    let Some(section) = hdv.ini.section("SUSPENSION") else {
        return Ok(());
    };
    let Some(raw) = section.value(key) else {
        return Ok(());
    };
    let value = raw.trim().parse::<f64>().unwrap_or(f64::NAN);
    if !value.is_finite() || value != 0.0 {
        return Err(ConfigError::new(format!(
            "HDV {key}={raw} override is unsupported in prototype (reject rather than ignore)"
        )));
    }
    Ok(())
}
