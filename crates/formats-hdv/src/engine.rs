//! The engine `.ini` file referenced by `[ENGINE] Normal` of the `.hdv`.
//!
//! The file has no sections: it is a single list of `Key=Value` lines. The one
//! repeated key is `RPMTorque=(rpm, min_torque, max_torque)`, kept in file
//! order; `min_torque` is the engine-drag side and `max_torque` the full
//! throttle side. `RevLimitRange` / `RevLimitSetting` resolve to the rev limit
//! in RPM (see [`crate::ini::Section::ranged`]).

use crate::ini::{parse_f64, parse_tuple, parse_tuple3, Ini};

/// Parsed engine `.ini` file.
#[derive(Debug, Clone, Default)]
pub struct EngineFile {
    /// `(rpm, min_torque, max_torque)` triples in file order.
    pub rpm_torque: Vec<(f64, f64, f64)>,
    /// `RPMTorque` lines that did not decode into three numbers. Kept so the
    /// drive configuration can refuse to silently drop a malformed row.
    pub rpm_torque_invalid: usize,
    /// `EngineInertia`, the rotational inertia of the engine components.
    pub inertia: Option<f64>,
    /// Rev limit in RPM, resolved from `RevLimitRange` / `RevLimitSetting`.
    pub rev_limit: Option<f64>,
    /// `IdleRPMLogic`, the `(low, high)` RPM band the idle tries to hold.
    pub idle_rpm: Option<(f64, f64)>,
}

impl EngineFile {
    /// Parse the whole engine `.ini` text.
    pub fn parse(text: &str) -> Self {
        let ini = Ini::parse(text);
        // Engine files carry no section header, so every pair is unnamed.
        let root = ini.section("");

        let mut rpm_torque = Vec::new();
        let mut rpm_torque_invalid = 0usize;
        if let Some(section) = root {
            for value in section.values("RPMTorque") {
                match parse_tuple3(value) {
                    Some(v) => rpm_torque.push((v[0], v[1], v[2])),
                    None => rpm_torque_invalid += 1,
                }
            }
        }

        EngineFile {
            rpm_torque,
            rpm_torque_invalid,
            inertia: root
                .and_then(|s| s.value("EngineInertia"))
                .and_then(parse_f64),
            rev_limit: root.and_then(|s| s.ranged("RevLimit")).map(|r| r.value),
            idle_rpm: root
                .and_then(|s| s.value("IdleRPMLogic"))
                .and_then(parse_tuple)
                .and_then(|v| (v.len() >= 2).then(|| (v[0], v[1]))),
        }
    }

    /// The highest full-throttle torque and the RPM where it occurs.
    pub fn peak_torque(&self) -> Option<(f64, f64)> {
        self.rpm_torque
            .iter()
            .map(|&(rpm, _, max)| (max, rpm))
            .max_by(|a, b| a.0.total_cmp(&b.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENGINE: &str = "\
RPMTorque=( 0.0, -10.0, -10.0)
RPMTorque=( 500.0, -12.5, 8.0)
RPMTorque=(9000.0, -80.0, 250.0)
RPMTorque=(15000.0, -120.0, -1.0)
EngineInertia=0.05000
IdleRPMLogic=(2000.00, 3000.0000)
RevLimitRange=(10000, 200, 6)
RevLimitSetting=5
";

    #[test]
    fn keeps_every_torque_point_in_order() {
        let engine = EngineFile::parse(ENGINE);
        assert_eq!(engine.rpm_torque.len(), 4);
        assert_eq!(engine.rpm_torque[0], (0.0, -10.0, -10.0));
        assert_eq!(engine.rpm_torque[2], (9000.0, -80.0, 250.0));
    }

    #[test]
    fn reads_inertia_and_rev_limit() {
        let engine = EngineFile::parse(ENGINE);
        assert_eq!(engine.inertia, Some(0.05));
        assert_eq!(engine.rev_limit, Some(11000.0));
    }

    #[test]
    fn reads_idle_band_and_peak_torque() {
        let engine = EngineFile::parse(ENGINE);
        assert_eq!(engine.idle_rpm, Some((2000.0, 3000.0)));
        assert_eq!(engine.peak_torque(), Some((250.0, 9000.0)));
    }

    #[test]
    fn counts_malformed_torque_rows() {
        let engine = EngineFile::parse("RPMTorque=(0, -1, -2)\nRPMTorque=garbage\nRPMTorque=(1)\n");
        assert_eq!(engine.rpm_torque.len(), 1);
        assert_eq!(engine.rpm_torque_invalid, 2);
    }
}
