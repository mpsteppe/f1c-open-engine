//! Headless runtime validation: load a real car and validate it for driving.
//!
//! Skip when `F1C_GAME_DIR` is unset. When set, the known DS11 acceptance car
//! `1994_Ferrari28.veh` is loaded and validated; a missing car is reported as
//! skipped rather than failing, so the suite still passes on a machine without
//! the game installed.

use std::path::{Path, PathBuf};

use formats_hdv::CarPhysics;
use physics_drive::CarSpec;

#[test]
fn validates_ferrari28_when_game_dir_is_set() {
    let Ok(root) = std::env::var("F1C_GAME_DIR") else {
        println!("skipped: F1C_GAME_DIR unset");
        return;
    };
    let Some(veh) = find_veh(Path::new(&root), "1994_Ferrari28.veh") else {
        println!("skipped: 1994_Ferrari28.veh not found under {root}");
        return;
    };

    let physics = CarPhysics::load(&veh).expect("load .veh");
    assert!(
        physics.missing.is_empty(),
        "missing linked files: {:?}",
        physics.missing
    );
    let spec = CarSpec::from_physics(&physics).expect("drive validation");
    println!(
        "validated {}: mass {:.0} kg, gears {}, final drive {:.3}, rev limit {:.0}, \
         idle {:.0}, radii front/rear {:.3}/{:.3}, wheelbase {:.3}, steer lock {:.1} deg",
        veh.display(),
        spec.mass,
        spec.gear_count(),
        spec.final_drive,
        spec.rev_limit,
        spec.idle_rpm,
        spec.front_radius,
        spec.rear_radius,
        spec.wheelbase,
        spec.steer_lock_rad.to_degrees(),
    );
    assert!(spec.mass > 0.0);
    assert!(spec.gear_count() >= 1);
    assert!(spec.wheelbase > 0.0);
    assert!(spec.front_radius > 0.0 && spec.rear_radius > 0.0);
}

/// Depth-first search for a file name (case-insensitive) under `dir`.
fn find_veh(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_veh(&path, name) {
                return Some(found);
            }
        } else if path
            .file_name()
            .is_some_and(|file| file.to_string_lossy().eq_ignore_ascii_case(name))
        {
            return Some(path);
        }
    }
    None
}
