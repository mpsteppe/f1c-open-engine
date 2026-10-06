//! Runtime barrier-stop acceptance for the configured sample track.
//!
//! Skips only when `F1C_GAME_DIR` is unset. When it is set, the Adelaide SCN and
//! the Ferrari `.veh` must exist, the barrier set must build, the spawn proxy
//! must be clear, and a constructed sweep to a loaded retained triangle's face
//! interior must report contact. A missing sample fails rather than passing by
//! skipping. The diagnostic sweep proves the geometry query; it is **not**
//! evidence the car reaches that wall in a drive. These are sampled geometry
//! numbers, not original collision behaviour.

use std::path::{Path, PathBuf};

use physics_drive::{
    proxy_center, BarrierStop, CarSpec, DriveParams, HeldInput, RoadFollower, Session, Sim, Spawn,
    FIXED_STEP, PROXY_RADIUS,
};

const SCENARIO_STEPS: u32 = 600;

fn read_latin1(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("read latin-1 file");
    bytes.into_iter().map(char::from).collect()
}

#[test]
fn adelaide_grid0_barrier_stop_preflight_and_sweep() {
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

    let ground = viewer::build_ground_query(&scene, &mesh_index).expect("ground query build");
    let barrier = viewer::build_barrier_query(&scene, &mesh_index).expect("barrier query build");
    println!(
        "barrier: selected {} meshes, excluded {}, unsupported {}",
        barrier.selected, barrier.excluded, barrier.unsupported
    );
    let stats = barrier.set.stats();
    println!(
        "barrier: triangles retained {}, rejected {} (degenerate {}, shallow {})",
        stats.retained,
        stats.rejected(),
        stats.degenerate,
        stats.horizontal
    );
    assert!(barrier.selected > 0, "no selected barrier meshes");
    assert!(stats.retained > 0, "no retained barrier triangles");

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
    let follower = RoadFollower::new(&ground.set, &sim).expect("road follower at spawn");

    let center = proxy_center(follower.pose());
    let overlap = barrier
        .set
        .sweep_sphere(center, center, PROXY_RADIUS)
        .expect("spawn overlap query");
    println!(
        "barrier preflight: proxy centre ({:.3}, {:.3}, {:.3}), radius {:.2} m, \
         spawn clear {}",
        center[0],
        center[1],
        center[2],
        PROXY_RADIUS,
        overlap.hit.is_none()
    );
    assert!(overlap.hit.is_none(), "spawn proxy overlaps a barrier");
    let _guard = BarrierStop::new(barrier.set.clone(), &follower).expect("clear spawn");

    let diagnostic = viewer::diagnostic_barrier_sweep(&barrier.set).unwrap_or_else(|message| {
        panic!("diagnostic sweep to the first retained triangle: {message}")
    });
    println!(
        "barrier diagnostic sweep: {} (occurrence {}) from ({:.3}, {:.3}, {:.3}) to \
         ({:.3}, {:.3}, {:.3}), t {:.4}, candidates {}",
        diagnostic.name,
        diagnostic.source.occurrence,
        diagnostic.start[0],
        diagnostic.start[1],
        diagnostic.start[2],
        diagnostic.end[0],
        diagnostic.end[1],
        diagnostic.end[2],
        diagnostic.t,
        diagnostic.candidates
    );
    assert!(diagnostic.t >= 0.0 && diagnostic.t <= 1.0);
    assert!(diagnostic.t.is_finite());

    // Bounded straight-throttle scenario: report whether the proxy contacts a
    // barrier, without claiming the car reaches any particular wall.
    let mut session = Session::new(sim);
    session.set_road_follow(follower);
    session.set_barrier_stop(_guard);
    let held = HeldInput {
        throttle: true,
        ..HeldInput::default()
    };
    let mut accepted = 0u32;
    for step in 0..SCENARIO_STEPS {
        let ran = session.advance_with_ground(FIXED_STEP, held, &ground.set);
        if ran == 0 {
            println!("stopped at step {step} (paused or latched)");
            break;
        }
        if session.barrier().unwrap().is_stopped() {
            println!("barrier stopped at step {step}");
            break;
        }
        if session.follower().unwrap().is_lost() {
            println!("surface lost at step {step}");
            break;
        }
        accepted += 1;
    }
    let barrier = session.barrier().unwrap();
    let contact = barrier.last_contact().map(|contact| {
        format!(
            "{} (occurrence {}) t {:.4}",
            contact.name, contact.source.occurrence, contact.t
        )
    });
    println!(
        "barrier scenario: accepted {accepted} steps, status {:?}, last contact {}",
        barrier.status(),
        contact.as_deref().unwrap_or("none")
    );
    if barrier.is_stopped() {
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
