//! Runtime road-follow acceptance for the configured sample track.
//!
//! Skips only when `F1C_GAME_DIR` is unset. When it is set, the Adelaide SCN and
//! the Ferrari `.veh` must exist and the spawn follower must initialize; a
//! missing sample fails rather than passing by skipping. The run is a bounded
//! straight-throttle scenario and reports either accepted poses or a correctly
//! latched surface loss. These are sampled geometry numbers, not original
//! physics.

use std::path::{Path, PathBuf};

use physics_drive::{
    CarSpec, DriveParams, HeldInput, RoadFollower, Session, Sim, Spawn, FIXED_STEP,
};

const SCENARIO_STEPS: u32 = 600;

fn read_latin1(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("read latin-1 file");
    bytes.into_iter().map(char::from).collect()
}

#[test]
fn adelaide_grid0_initializes_road_following() {
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
    let stats = build.set.stats();
    println!(
        "selected {} meshes, excluded {}, unsupported {}; retained {} triangles, \
         rejected {} (degenerate {}, steep {})",
        build.selected,
        build.excluded,
        build.unsupported,
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

    let follower = RoadFollower::new(&build.set, &sim).expect("road follower at spawn");
    let sample = follower.surface();
    println!(
        "spawn surface: height {:.3} m, grid reference {:.3} m, difference {:+.3} m, \
         source {}, candidates {}",
        sample.height,
        spawn.height,
        sample.height - spawn.height,
        sample.name,
        sample.candidates
    );

    let mut session = Session::new(sim);
    session.set_road_follow(follower);
    let held = HeldInput {
        throttle: true,
        ..HeldInput::default()
    };

    let mut accepted = 0u32;
    let mut max_jump: f64 = 0.0;
    let mut min_height = f64::INFINITY;
    let mut max_height = f64::NEG_INFINITY;
    let mut previous = session.follower().unwrap().surface().height;
    for step in 0..SCENARIO_STEPS {
        let ran = session.advance_with_ground(FIXED_STEP, held, &build.set);
        if ran == 0 {
            println!("stopped at step {step}: no further steps (paused or lost)");
            break;
        }
        let follower = session.follower().unwrap();
        if follower.is_lost() {
            println!("surface lost at step {step}");
            break;
        }
        accepted += 1;
        let height = follower.surface().height;
        min_height = min_height.min(height);
        max_height = max_height.max(height);
        let jump = (height - previous).abs();
        max_jump = max_jump.max(jump);
        assert!(
            jump <= physics_drive::MAX_STEP_JUMP + 1e-9,
            "step {step}: height jump {jump:.4} m exceeds the bound"
        );
        previous = height;
    }

    let follower = session.follower().unwrap();
    let final_sample = follower.surface();
    let status = if follower.is_lost() {
        "LOST"
    } else {
        "following"
    };
    let (low, high) = if max_height.is_finite() {
        (min_height, max_height)
    } else {
        (0.0, 0.0)
    };
    println!(
        "scenario: {:.3} s, straight full throttle; accepted {accepted} steps; \
         height range {low:.3}..{high:.3} m (range {:.3} m), max jump {max_jump:.4} m; \
         final {status}, source {}, last height {:.3} m",
        f64::from(accepted) * FIXED_STEP,
        high - low,
        final_sample.name,
        final_sample.height
    );

    assert!(accepted >= 1, "follower never accepted a step");
    if follower.is_lost() {
        assert_eq!(session.sim.state.speed, 0.0);
    }
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
