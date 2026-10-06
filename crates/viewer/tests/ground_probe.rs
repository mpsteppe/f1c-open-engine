//! Runtime ground-query acceptance for the configured sample track.
//!
//! Skips only when `F1C_GAME_DIR` is unset. When it is set, the Adelaide SCN
//! and the Ferrari `.veh` must exist and the spawn probe must return a hit; a
//! missing sample or a failed probe fails rather than passing by skipping.
//! The measured numbers are sampled geometry evidence, not original physics.

use std::path::{Path, PathBuf};

use physics_drive::{CarSpec, DriveParams, Sim, Spawn};

fn read_latin1(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("read latin-1 file");
    bytes.into_iter().map(char::from).collect()
}

#[test]
fn adelaide_grid0_spawn_has_a_ground_hit() {
    let Ok(root) = std::env::var("F1C_GAME_DIR") else {
        println!("skipped: F1C_GAME_DIR unset");
        return;
    };
    let root = PathBuf::from(root);
    let scn = find_file(&root, "1994_Adelaide.SCN")
        .expect("configured sample: 1994_Adelaide.SCN not found under F1C_GAME_DIR");
    let veh = find_file(&root, "1994_Ferrari28.veh")
        .expect("configured sample: 1994_Ferrari28.veh not found under F1C_GAME_DIR");

    let scene = formats_scn::parse(&read_latin1(&scn));
    let (mas_paths, missing) = viewer::resolve_scene_mas(&scn, &scene);
    println!("MAS found {}, missing {}", mas_paths.len(), missing.len());
    if !missing.is_empty() {
        println!("missing MAS: {missing:?}");
    }
    let mesh_index = viewer::MeshIndex::build(&mas_paths);

    let build = viewer::build_ground_query(&scene, &mesh_index).expect("ground query build");
    println!(
        "selected {} meshes, excluded {}, unsupported {}",
        build.selected, build.excluded, build.unsupported
    );
    let stats = build.set.stats();
    println!(
        "retained {} triangles, rejected {} (degenerate {}, steep {})",
        stats.retained,
        stats.rejected(),
        stats.degenerate,
        stats.steep
    );
    assert!(build.selected > 0, "no selected meshes");
    assert!(stats.retained > 0, "no retained triangles");

    let (pos, ori) = viewer::resolve_grid(&scn, 0).expect("grid 0");
    let physics = formats_hdv::CarPhysics::load(&veh).expect("load .veh");
    let spec = CarSpec::from_physics(&physics).expect("car spec");
    let spawn = Spawn {
        origin_x: f64::from(pos[0]),
        origin_z: f64::from(pos[2]),
        yaw: f64::from(ori[1]),
        height: f64::from(pos[1]),
    };
    let sim = Sim::new(spec, DriveParams::default(), spawn).expect("sim");
    let [rear_x, rear_z] = sim.rear_axle();

    let result = build
        .set
        .query(rear_x, rear_z, spawn.height, 2.0)
        .expect("query");
    let hit = result.hit.expect("no nearby surface at spawn rear axle");
    println!(
        "spawn rear axle ({rear_x:.3}, {rear_z:.3}): height {:.3} m, \
         reference {:.3} m, difference {:+.3} m, normal ({:.3}, {:.3}, {:.3}), \
         source {}, candidates {}",
        hit.height,
        spawn.height,
        hit.height - spawn.height,
        hit.normal[0],
        hit.normal[1],
        hit.normal[2],
        hit.name,
        result.candidates
    );
    assert!(hit.height.is_finite());
    assert!(hit.normal[1] >= 0.5, "normal not upward: {:?}", hit.normal);
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
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
