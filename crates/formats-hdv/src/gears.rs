//! The gear ratios `.ini` file referenced by `[DRIVELINE] GearFile`.
//!
//! Two sections, each holding repeated tooth-count pairs:
//! `[GEAR_RATIOS]` with `ratio=(drive, driven)` lines and `[FINAL_DRIVE]` with a
//! single `bevel=(drive, driven)` plus `ratio=` lines. A ratio's value is
//! `driven / drive`. The file notes that each list is sorted after reading, so
//! both lists are sorted here by descending ratio; `GearNSetting`,
//! `ReverseSetting` and `FinalDriveSetting` are indexes into those lists.
//! The final drive is the bevel ratio times the selected final-drive ratio
//! (for example `(30, 42)` and `(13, 56)` give `42/30 * 56/13 = 6.031`).

use crate::ini::{parse_tuple, Ini};

/// Parsed gear ratios file.
#[derive(Debug, Clone, Default)]
pub struct GearFile {
    /// `[GEAR_RATIOS]` tooth pairs, sorted by descending ratio.
    pub gear_pairs: Vec<(f64, f64)>,
    /// `[FINAL_DRIVE] bevel` tooth pair, the fixed multiplier.
    pub bevel: Option<(f64, f64)>,
    /// `[FINAL_DRIVE] ratio` tooth pairs, sorted by descending ratio.
    pub final_pairs: Vec<(f64, f64)>,
}

impl GearFile {
    /// Parse the whole gear ratios text.
    pub fn parse(text: &str) -> Self {
        let ini = Ini::parse(text);
        let gear_pairs = pairs(&ini, "GEAR_RATIOS", "ratio");
        let final_pairs = pairs(&ini, "FINAL_DRIVE", "ratio");
        let bevel = ini
            .section("FINAL_DRIVE")
            .and_then(|section| section.value("bevel"))
            .and_then(parse_pair);
        GearFile {
            gear_pairs,
            bevel,
            final_pairs,
        }
    }

    /// Resolved gear ratio (driven / drive) for a list index.
    pub fn gear_ratio(&self, setting: usize) -> Option<f64> {
        self.gear_pairs
            .get(setting)
            .map(|&(drive, driven)| driven / drive)
    }

    /// Resolved final-drive ratio for a list index, before the bevel.
    pub fn final_ratio(&self, setting: usize) -> Option<f64> {
        self.final_pairs
            .get(setting)
            .map(|&(drive, driven)| driven / drive)
    }

    /// The selected final-drive ratio times the bevel ratio.
    pub fn final_drive(&self, setting: usize) -> Option<f64> {
        let ratio = self.final_ratio(setting)?;
        Some(ratio * self.bevel_factor())
    }

    /// The bevel ratio `driven / drive`, or `1.0` when absent.
    pub fn bevel_factor(&self) -> f64 {
        self.bevel
            .map(|(drive, driven)| driven / drive)
            .unwrap_or(1.0)
    }
}

/// Every `key=` pair of one section, sorted by descending ratio.
fn pairs(ini: &Ini, section: &str, key: &str) -> Vec<(f64, f64)> {
    let mut pairs: Vec<(f64, f64)> = ini.values(section, key).filter_map(parse_pair).collect();
    pairs.sort_by(|a, b| (b.1 / b.0).total_cmp(&(a.1 / a.0)));
    pairs
}

/// Parse a two-number tuple such as `(12, 34)`, allowing spaces.
fn parse_pair(value: &str) -> Option<(f64, f64)> {
    let values = parse_tuple(value)?;
    if values.len() != 2 {
        return None;
    }
    Some((values[0], values[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEARS: &str = "\
[GEAR_RATIOS]
ratio=(10, 25)
ratio=(11, 30)
ratio=(12, 34)
ratio=(20, 20)

[FINAL_DRIVE]
bevel=(20, 28)
ratio=(10, 40)
ratio=(10, 35)
";

    #[test]
    fn sorts_gears_by_descending_ratio() {
        let gears = GearFile::parse(GEARS);
        let values: Vec<f64> = gears.gear_pairs.iter().map(|&(a, b)| b / a).collect();
        assert_eq!(values, vec![34.0 / 12.0, 30.0 / 11.0, 25.0 / 10.0, 1.0]);
    }

    #[test]
    fn indexes_resolve_to_ratios() {
        let gears = GearFile::parse(GEARS);
        assert_eq!(gears.gear_ratio(0), Some(34.0 / 12.0));
        assert_eq!(gears.gear_ratio(9), None);
    }

    #[test]
    fn final_drive_is_bevel_times_selected_ratio() {
        let gears = GearFile::parse(GEARS);
        assert_eq!(gears.bevel_factor(), 28.0 / 20.0);
        assert_eq!(gears.final_ratio(0), Some(40.0 / 10.0));
        let expected = 28.0 / 20.0 * 40.0 / 10.0;
        assert_eq!(gears.final_drive(0), Some(expected));
    }

    #[test]
    fn bevel_absent_means_factor_one() {
        let gears = GearFile::parse("[FINAL_DRIVE]\nratio=(10, 35)\n");
        assert_eq!(gears.bevel_factor(), 1.0);
        assert_eq!(gears.final_drive(0), Some(35.0 / 10.0));
    }
}
