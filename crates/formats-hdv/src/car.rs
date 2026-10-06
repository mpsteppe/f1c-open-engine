//! The car physics loader: from a `.veh` path, read the `.hdv` and every file
//! it links to, then expose the resolved values the engine will use.
//!
//! Linked file names are resolved case-insensitively in the team folder first
//! and then in the nearest ancestor folder named `Vehicles` (the
//! `SeasonData\Vehicles` root). A missing or unreadable linked file is recorded
//! in [`CarPhysics::missing`]; the loader never panics.

use std::path::{Path, PathBuf};

use crate::engine::EngineFile;
use crate::gears::GearFile;
use crate::hdv::{Hdv, Wheel, WheelDrive};
use crate::ini::{parse_text, Ini};
use crate::pm::PmFile;
use crate::tbc::TbcFile;

/// A linked file that could not be found or read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingFile {
    /// Which link failed: `HDV`, `engine`, `gears`, `tires` or `suspension`.
    pub role: &'static str,
    /// The name as written in the `.veh` / `.hdv`, or a placeholder.
    pub name: String,
}

/// The load returned an error only when the `.veh` itself cannot be read.
#[derive(Debug)]
pub struct LoadError {
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LoadError {}

/// One car's parsed physics: the `.hdv` plus its four linked files.
#[derive(Debug)]
pub struct CarPhysics {
    pub veh_path: PathBuf,
    pub hdv: Option<Hdv>,
    pub engine: Option<EngineFile>,
    pub gears: Option<GearFile>,
    pub tires: Option<TbcFile>,
    pub suspension: Option<PmFile>,
    pub missing: Vec<MissingFile>,
}

impl CarPhysics {
    /// Load a car from its `.veh` path.
    ///
    /// `Err` only when the `.veh` file cannot be read.
    pub fn load(veh_path: &Path) -> Result<Self, LoadError> {
        let text = read_latin1(veh_path).ok_or_else(|| LoadError {
            message: format!("cannot read {}", veh_path.display()),
        })?;
        let veh = Ini::parse(&text);
        let dirs = search_dirs(veh_path);
        let mut missing = Vec::new();

        let hdv = match linked_name(&veh, "HDVehicle") {
            Some(name) => {
                load_text(&dirs, &name, "hdv", "HDV", &mut missing).map(|t| Hdv::parse(&t))
            }
            None => {
                missing.push(MissingFile {
                    role: "HDV",
                    name: "HDVehicle".to_string(),
                });
                None
            }
        };

        let mut engine = None;
        let mut gears = None;
        let mut tires = None;
        let mut suspension = None;
        if let Some(hdv) = hdv.as_ref() {
            engine = load_key(hdv.engine_name(), &dirs, "ini", "engine", &mut missing)
                .map(|t| EngineFile::parse(&t));
            gears = load_key(hdv.gear_file(), &dirs, "ini", "gears", &mut missing)
                .map(|t| GearFile::parse(&t));
            tires = load_key(hdv.tire_brand(), &dirs, "tbc", "tires", &mut missing)
                .map(|t| TbcFile::parse(&t));
            suspension = load_key(
                hdv.physical_model_file(),
                &dirs,
                "pm",
                "suspension",
                &mut missing,
            )
            .map(|t| PmFile::parse(&t));
        }

        Ok(CarPhysics {
            veh_path: veh_path.to_path_buf(),
            hdv,
            engine,
            gears,
            tires,
            suspension,
            missing,
        })
    }

    /// `[GENERAL] Mass` in kg.
    pub fn mass(&self) -> Option<f64> {
        self.hdv.as_ref()?.mass()
    }

    /// `[GENERAL] Inertia` in kg m^2.
    pub fn inertia(&self) -> Option<[f64; 3]> {
        self.hdv.as_ref()?.inertia()
    }

    /// `[GENERAL] CGHeight` in metres.
    pub fn cg_height(&self) -> Option<f64> {
        self.hdv.as_ref()?.cg_height()
    }

    /// Which wheels are driven.
    pub fn wheel_drive(&self) -> Option<WheelDrive> {
        self.hdv.as_ref()?.wheel_drive()
    }

    /// Resolved forward gear ratios, in gear order.
    pub fn gear_ratios(&self) -> Vec<f64> {
        let (Some(hdv), Some(gears)) = (self.hdv.as_ref(), self.gears.as_ref()) else {
            return Vec::new();
        };
        hdv.gear_settings()
            .iter()
            .filter_map(|&setting| gears.gear_ratio(setting))
            .collect()
    }

    /// Resolved reverse gear ratio.
    pub fn reverse_ratio(&self) -> Option<f64> {
        let hdv = self.hdv.as_ref()?;
        let gears = self.gears.as_ref()?;
        gears.gear_ratio(hdv.reverse_setting()?)
    }

    /// Resolved final drive ratio (bevel times the selected final ratio).
    pub fn final_drive(&self) -> Option<f64> {
        let hdv = self.hdv.as_ref()?;
        let gears = self.gears.as_ref()?;
        gears.final_drive(hdv.final_drive_setting()?)
    }

    /// The engine torque curve as `(rpm, min_torque, max_torque)` triples.
    pub fn torque_curve(&self) -> &[(f64, f64, f64)] {
        self.engine
            .as_ref()
            .map(|engine| engine.rpm_torque.as_slice())
            .unwrap_or(&[])
    }

    /// Rev limit in RPM.
    pub fn rev_limit(&self) -> Option<f64> {
        self.engine.as_ref()?.rev_limit
    }

    /// Highest full-throttle torque and the RPM where it occurs.
    pub fn peak_torque(&self) -> Option<(f64, f64)> {
        self.engine.as_ref()?.peak_torque()
    }

    /// Steering lock in degrees.
    pub fn steer_lock_deg(&self) -> Option<f64> {
        self.hdv.as_ref()?.steer_lock_deg()
    }

    /// Maximum brake torque of one wheel.
    pub fn brake_torque(&self, wheel: Wheel) -> Option<f64> {
        self.hdv.as_ref()?.brake_torque(wheel)
    }

    /// Tire brand file name (without extension).
    pub fn tire_brand(&self) -> Option<String> {
        self.hdv.as_ref()?.tire_brand()
    }

    /// Display name of the selected tire compound.
    pub fn tire_compound_name(&self) -> Option<&str> {
        let hdv = self.hdv.as_ref()?;
        let tires = self.tires.as_ref()?;
        tires.compound_name(hdv.tire_compound_setting()?)
    }

    /// Suspension physical-model file name.
    pub fn physical_model_file(&self) -> Option<String> {
        self.hdv.as_ref()?.physical_model_file()
    }

    /// A short multi-line summary for the viewer and reports.
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("Physics: {}", file_name(&self.veh_path))];

        lines.push(format!("  Mass: {}", format_number(self.mass(), " kg", 1)));
        lines.push(format!(
            "  CG height: {}",
            format_number(self.cg_height(), " m", 3)
        ));

        let gears = self.gear_ratios();
        let gear_list = if gears.is_empty() {
            "unknown".to_string()
        } else {
            gears
                .iter()
                .map(|ratio| format!("{ratio:.3}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let final_drive = format_number(self.final_drive(), "", 3);
        let reverse = format_number(self.reverse_ratio(), "", 3);
        lines.push(format!(
            "  Gears: {} [{gear_list}] final {final_drive}, reverse {reverse}",
            gears.len()
        ));

        let torque = match self.peak_torque() {
            Some((torque, rpm)) => format!("{torque:.1} Nm @ {rpm:.0} RPM"),
            None => "unknown".to_string(),
        };
        lines.push(format!("  Peak torque: {torque}"));
        lines.push(format!(
            "  Rev limit: {}",
            format_number(self.rev_limit(), " RPM", 0)
        ));

        let compound = self.tire_compound_name().unwrap_or("unknown");
        match self.hdv.as_ref().and_then(Hdv::tire_compound_setting) {
            Some(index) => {
                let brand = self.tire_brand().unwrap_or_else(|| "?".to_string());
                lines.push(format!("  Tire: {compound} ({brand} index {index})"));
            }
            None => lines.push(format!("  Tire: {compound}")),
        }

        let missing = if self.missing.is_empty() {
            "none".to_string()
        } else {
            self.missing
                .iter()
                .map(|entry| format!("{} ({})", entry.role, entry.name))
                .collect::<Vec<_>>()
                .join(", ")
        };
        lines.push(format!("  Missing: {missing}"));
        lines
    }
}

/// All search directories for a `.veh`: the team folder, then `Vehicles`.
fn search_dirs(veh_path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(team) = veh_path.parent() {
        dirs.push(team.to_path_buf());
        if let Some(vehicles) = find_dir_named(team, "Vehicles") {
            if vehicles != team {
                dirs.push(vehicles);
            }
        }
    }
    dirs
}

/// Nearest ancestor of `dir` (including itself) named `name`, case-insensitive.
fn find_dir_named(dir: &Path, name: &str) -> Option<PathBuf> {
    dir.ancestors()
        .find(|ancestor| {
            ancestor
                .file_name()
                .is_some_and(|file| file.to_string_lossy().eq_ignore_ascii_case(name))
        })
        .map(Path::to_path_buf)
}

/// Value of `key` in the unnamed root of a `.veh` (`HDVehicle`, `Graphics`).
fn linked_name(veh: &Ini, key: &str) -> Option<String> {
    veh.value("", key)
        .map(parse_text)
        .filter(|value| !value.is_empty())
}

/// Read a linked file named by an optional `.hdv` key, recording a miss.
fn load_key(
    name: Option<String>,
    dirs: &[PathBuf],
    ext: &str,
    role: &'static str,
    missing: &mut Vec<MissingFile>,
) -> Option<String> {
    match name {
        Some(name) => load_text(dirs, &name, ext, role, missing),
        None => {
            missing.push(MissingFile {
                role,
                name: format!("<no {role} key>"),
            });
            None
        }
    }
}

/// Find and read one linked file, recording a miss. `name` may lack `ext`.
fn load_text(
    dirs: &[PathBuf],
    name: &str,
    ext: &str,
    role: &'static str,
    missing: &mut Vec<MissingFile>,
) -> Option<String> {
    let Some(path) = resolve_linked(dirs, name, ext) else {
        missing.push(MissingFile {
            role,
            name: name.to_string(),
        });
        return None;
    };
    match read_latin1(&path) {
        Some(text) => Some(text),
        None => {
            missing.push(MissingFile {
                role,
                name: name.to_string(),
            });
            None
        }
    }
}

/// Resolve `name` in each directory, adding `.ext` when `name` has none.
fn resolve_linked(dirs: &[PathBuf], name: &str, ext: &str) -> Option<PathBuf> {
    let has_extension = Path::new(name).extension().is_some();
    let with_ext = format!("{name}.{ext}");
    for dir in dirs {
        let candidates: &[&str] = if has_extension {
            &[name]
        } else {
            &[with_ext.as_str(), name]
        };
        for candidate in candidates {
            if let Some(path) = find_file_in(dir, candidate) {
                return Some(path);
            }
        }
    }
    None
}

/// Case-insensitive file lookup inside one directory.
fn find_file_in(dir: &Path, name: &str) -> Option<PathBuf> {
    let read_dir = std::fs::read_dir(dir).ok()?;
    for entry in read_dir.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(name)
        {
            return Some(entry.path());
        }
    }
    None
}

/// Read a file as Latin-1. `None` when it cannot be read.
fn read_latin1(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(bytes.into_iter().map(char::from).collect())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Format an optional number with a unit, or `unknown`.
fn format_number(value: Option<f64>, unit: &str, decimals: usize) -> String {
    match value {
        Some(number) => format!("{number:.decimals$}{unit}"),
        None => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VEH: &str = "\
DefaultLivery=\"COMET\"
HDVehicle=comet_gt.hdv
Graphics=comet_generic.gen
Number=7
";

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
FinalDriveSetting=0
ReverseSetting=3
ForwardGears=2
Gear1Setting=0
Gear2Setting=1

[SUSPENSION]
PhysicalModelFile=comet_susp.pm

[FRONTLEFT]
BrakeTorque=2000.0
";

    const ENGINE: &str = "\
RPMTorque=(0.0, -10.0, -10.0)
RPMTorque=(9000.0, -80.0, 250.0)
EngineInertia=0.05000
RevLimitRange=(10000, 200, 6)
RevLimitSetting=5
";

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

    const TBC: &str = "\
[COMPOUND]
Name=\"Prime Compound\"
WetWeather=1
Front:
Radius=0.300

[COMPOUND]
Name=\"Option Compound\"
WetWeather=1
Front:
Radius=0.300
";

    const PM: &str = "\
[BODY]
name=chassis mass=(320.0) inertia=(100.0,150.0,20.0)
pos=(0.0,0.0,0.0) ori=(0.0,0.0,0.0)

[BAR]
posbody=chassis negbody=upright pos=(0.1, 0.2, 0.3)
";

    /// A per-test scratch root under `%TEMP%\f1c_openengine`.
    fn temp_root() -> PathBuf {
        std::env::temp_dir().join("f1c_openengine")
    }

    /// Write the invented fixtures into a directory unique to `test` so parallel
    /// tests never share a fixture path. Outputs are left in place.
    fn fixture(test: &str) -> PathBuf {
        let dir = temp_root().join(format!("hdv_car_{test}"));
        std::fs::create_dir_all(&dir).unwrap();
        let files = [
            ("comet.veh", VEH),
            ("comet_gt.hdv", HDV),
            ("comet_v8.ini", ENGINE),
            ("comet_gears.ini", GEARS),
            ("comet_tires.tbc", TBC),
            ("comet_susp.pm", PM),
        ];
        for (name, text) in files {
            std::fs::write(dir.join(name), text).unwrap();
        }
        dir.join("comet.veh")
    }

    #[test]
    fn loads_every_linked_file() {
        let car = CarPhysics::load(&fixture("loads_every_linked_file")).unwrap();
        assert!(car.hdv.is_some());
        assert!(car.engine.is_some());
        assert!(car.gears.is_some());
        assert!(car.tires.is_some());
        assert!(car.suspension.is_some());
        assert!(car.missing.is_empty());
    }

    #[test]
    fn resolves_values_across_files() {
        let car = CarPhysics::load(&fixture("resolves_values_across_files")).unwrap();
        assert_eq!(car.mass(), Some(750.0));
        assert_eq!(car.inertia(), Some([500.0, 600.0, 120.0]));
        assert_eq!(car.wheel_drive(), Some(WheelDrive::Rear));
        assert_eq!(car.rev_limit(), Some(11000.0));
        assert_eq!(car.peak_torque(), Some((250.0, 9000.0)));
        assert_eq!(car.steer_lock_deg(), Some(6.0));
        assert_eq!(car.brake_torque(Wheel::FrontLeft), Some(2000.0));
        assert_eq!(car.tire_compound_name(), Some("Option Compound"));
    }

    #[test]
    fn resolves_gears_and_final_drive() {
        let car = CarPhysics::load(&fixture("resolves_gears_and_final_drive")).unwrap();
        let gears = car.gear_ratios();
        assert_eq!(gears.len(), 2);
        assert_eq!(gears[0], 34.0 / 12.0);
        assert_eq!(gears[1], 30.0 / 11.0);
        assert_eq!(car.reverse_ratio(), Some(1.0));
        let expected = 28.0 / 20.0 * 40.0 / 10.0;
        assert_eq!(car.final_drive(), Some(expected));
    }

    #[test]
    fn missing_linked_file_is_reported_not_fatal() {
        let dir = temp_root().join("hdv_car_missing_linked");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("car.veh"), "HDVehicle=absent.hdv\n").unwrap();
        let car = CarPhysics::load(&dir.join("car.veh")).unwrap();
        assert!(car.hdv.is_none());
        assert_eq!(
            car.missing,
            vec![MissingFile {
                role: "HDV",
                name: "absent.hdv".to_string(),
            }]
        );
    }

    #[test]
    fn summary_lists_acceptance_fields() {
        let car = CarPhysics::load(&fixture("summary_lists_acceptance_fields")).unwrap();
        let summary = car.summary_lines().join("\n");
        assert!(summary.contains("Mass:"));
        assert!(summary.contains("Gears:"));
        assert!(summary.contains("Peak torque:"));
        assert!(summary.contains("Rev limit:"));
        assert!(summary.contains("Tire:"));
        assert!(summary.contains("Missing:"));
    }
}
