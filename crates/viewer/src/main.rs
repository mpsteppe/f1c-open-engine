//! F1C model viewer. A `.mas` argument shows one textured MTS model at a time;
//! a `.veh` argument assembles and shows the whole car (body, wheels,
//! suspension, driver, helmet, wings) in one view, textured; a `.SCN` argument
//! shows the whole track circuit with a fly camera.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    input::mouse::{MouseMotion, MouseWheel},
    mesh::Indices,
    pbr::{DistanceFog, FogFalloff},
    prelude::*,
    render::render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};

use formats_mas::MasArchive;
use ground_query::GroundQuerySet;
use physics_drive::{
    proxy_center, BarrierStop, CarSpec, DriveParams, HeldInput, RoadFollower, Session, Sim, Spawn,
    PROXY_OFFSET_Y, PROXY_RADIUS,
};
use viewer::{
    bounds, build_barrier_query, build_ground_query, car_forward, car_local, decode_rgba,
    diagnostic_barrier_sweep, follow_view_transform, model_names, mts_to_submeshes,
    mts_to_submeshes_at, place_car, resolve_grid, resolve_scene_mas, resolve_search_path,
    resolve_search_path_for_veh, sky_instance_indices, veh_gen_text, MeshIndex, SubMesh,
    TextureIndex,
};

/// The texture name, frame count and blend mode of one material.
#[derive(Clone)]
struct MaterialInfo {
    texture: String,
    frames: u32,
    src_blend: u16,
}

/// One viewable model plus what the viewer needs to draw it.
struct ModelData {
    name: String,
    submeshes: Vec<SubMesh>,
    /// Parallel to `submeshes`: true when the sub-mesh belongs to the sky ring
    /// or dome (drawn unlit, no fog, and following the camera).
    sky: Vec<bool>,
    materials: Vec<MaterialInfo>,
    textures_found: usize,
    textures_total: usize,
}

/// Every viewable model of the archive, in archive order.
#[derive(Resource, Default)]
struct ViewerModels(Vec<ModelData>);

/// Texture lookup, archive and decoded-image caches, plus the grey default.
#[derive(Resource)]
struct Textures {
    index: TextureIndex,
    archives: HashMap<PathBuf, MasArchive>,
    /// Decoded image and whether it has transparent pixels.
    cache: HashMap<String, (Handle<Image>, bool)>,
    grey: Option<Handle<StandardMaterial>>,
}

/// Camera, selection and texture state shared across frames.
#[derive(Resource)]
struct ViewerState {
    index: usize,
    needs_load: bool,
    textures_on: bool,
    center: Vec3,
    radius: f32,
    distance: f32,
    yaw: f32,
    pitch: f32,
    /// Free-fly camera (track mode) instead of an orbit camera.
    fly: bool,
    /// Camera position, free-fly mode only.
    position: Vec3,
    /// Movement speed in world units per second, free-fly mode only.
    speed: f32,
    /// Inferred sign of the car yaw; `R` toggles between `+1` and `-1`.
    yaw_sign: f32,
}

/// Runtime re-placement of the car when its inferred yaw sign is toggled.
#[derive(Resource)]
struct CarPlacement {
    /// Unplaced car sub-meshes in car space, material indices already final.
    base: Vec<SubMesh>,
    /// Index in `ViewerModels.0[0].submeshes` where the car block starts.
    start: usize,
    /// Grid `Pos` (metres, game axes).
    pos: [f32; 3],
    /// Grid `Ori` (radians around X, Y, Z).
    ori: [f32; 3],
}

/// The drive-mode car: entity-local geometry reused across frames.
#[derive(Resource)]
struct DriveCar {
    /// Car model with sub-meshes already shifted so the lowest vertex is at
    /// `y = 0`; a Bevy transform at the mesh origin places and yaws it.
    model: ModelData,
    /// Rebuild the car entities on the first frame and on texture toggle.
    needs_spawn: bool,
}

/// The headless driving session for `--drive`.
#[derive(Resource)]
struct DriveRuntime {
    session: Session,
}

/// The prototype ground query set and fixed reference height for `--ground-probe`.
#[derive(Resource)]
struct GroundProbe {
    set: GroundQuerySet,
    reference_y: f64,
}

/// The prototype ground query set used by the per-step road follower.
#[derive(Resource)]
struct RoadFollowMap {
    set: GroundQuerySet,
}

/// Control keys held this frame, sampled by the input system for the step loop.
#[derive(Resource, Default)]
struct DriveInput {
    held: HeldInput,
}

/// Marks one car part entity in drive mode.
#[derive(Component)]
struct DriveCarPart;

/// Lighting, fog, clear colour and draw distance read from a track `.SCN`.
#[derive(Resource)]
struct TrackLighting {
    /// `L` toggles between the SCN setup and the old default light.
    enabled: bool,
    /// True when the scene named a directional light with a direction.
    has_sun: bool,
    /// Direction the sunlight travels, already converted to viewer axes.
    sun_dir: Vec3,
    sun_color: Color,
    sun_illuminance: f32,
    ambient_color: Color,
    ambient_brightness: f32,
    /// `(colour, start, end)` for linear fog.
    fog: Option<(Color, f32, f32)>,
    /// Camera far plane (metres); at least 3000 so the sky is not cut.
    far: f32,
}

/// Marks the sun directional light so `L` can retune it.
#[derive(Component)]
struct SunLight;

/// Marks a sky sub-mesh; it follows the camera in X and Z every frame.
#[derive(Component)]
struct SkyMesh;

/// Marks an entity showing part of the current model.
#[derive(Component)]
struct Model;

/// The on-screen statistics block.
#[derive(Component)]
struct StatsText;

/// Command-line arguments.
#[derive(Default)]
struct Args {
    path: Option<String>,
    model: Option<String>,
    car: Option<String>,
    grid: Option<u32>,
    drive: bool,
    ground_probe: bool,
    road_follow: bool,
    barrier_stop: bool,
}

/// Split viewer args: positionals plus `--car <veh>` / `--grid <N>` /
/// `--drive` / `--ground-probe` / `--road-follow` / `--barrier-stop`.
fn parse_args(args: &[String]) -> Args {
    let mut out = Args::default();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if let Some(value) = arg.strip_prefix("--car=") {
            out.car = Some(value.to_string());
            index += 1;
        } else if let Some(value) = arg.strip_prefix("--grid=") {
            out.grid = value.parse().ok();
            index += 1;
        } else if arg == "--car" {
            out.car = args.get(index + 1).cloned();
            index += 2;
        } else if arg == "--grid" {
            out.grid = args.get(index + 1).and_then(|value| value.parse().ok());
            index += 2;
        } else if arg == "--drive" {
            out.drive = true;
            index += 1;
        } else if arg == "--ground-probe" {
            out.ground_probe = true;
            index += 1;
        } else if arg == "--road-follow" {
            out.road_follow = true;
            index += 1;
        } else if arg == "--barrier-stop" {
            out.barrier_stop = true;
            index += 1;
        } else if out.path.is_none() {
            out.path = Some(arg.clone());
            index += 1;
        } else if out.model.is_none() {
            out.model = Some(arg.clone());
            index += 1;
        } else {
            index += 1;
        }
    }
    out
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = parse_args(&args);
    let Some(path) = parsed.path.as_ref().map(PathBuf::from) else {
        eprintln!(
            "usage: viewer <MAS path | .veh path | .SCN path> [model name] [--car <veh>] [--grid N] [--drive] [--ground-probe] [--road-follow] [--barrier-stop]"
        );
        return ExitCode::from(2);
    };

    let lower = path.to_string_lossy().to_ascii_lowercase();
    if parsed.ground_probe && !lower.ends_with(".scn") {
        eprintln!("--ground-probe needs a track .SCN");
        return ExitCode::from(2);
    }
    if parsed.road_follow && !lower.ends_with(".scn") {
        eprintln!("--road-follow needs a track .SCN");
        return ExitCode::from(2);
    }
    if parsed.barrier_stop && !lower.ends_with(".scn") {
        eprintln!("--barrier-stop needs a track .SCN");
        return ExitCode::from(2);
    }
    if lower.ends_with(".veh") {
        if parsed.drive || parsed.ground_probe || parsed.road_follow || parsed.barrier_stop {
            eprintln!(
                "--drive, --ground-probe, --road-follow and --barrier-stop need a track .SCN and --car <veh>"
            );
            return ExitCode::from(2);
        }
        run_car(&path)
    } else if lower.ends_with(".scn") {
        if parsed.ground_probe && parsed.car.is_none() {
            eprintln!("--ground-probe needs --car <veh>");
            return ExitCode::from(2);
        }
        if parsed.ground_probe && !parsed.drive {
            eprintln!("--ground-probe needs --drive");
            return ExitCode::from(2);
        }
        if parsed.road_follow && parsed.car.is_none() {
            eprintln!("--road-follow needs --car <veh>");
            return ExitCode::from(2);
        }
        if parsed.road_follow && !parsed.drive {
            eprintln!("--road-follow needs --drive");
            return ExitCode::from(2);
        }
        if parsed.barrier_stop && parsed.car.is_none() {
            eprintln!("--barrier-stop needs --car <veh>");
            return ExitCode::from(2);
        }
        if parsed.barrier_stop && !parsed.drive {
            eprintln!("--barrier-stop needs --drive");
            return ExitCode::from(2);
        }
        if parsed.barrier_stop && !parsed.road_follow {
            eprintln!("--barrier-stop needs --road-follow");
            return ExitCode::from(2);
        }
        if parsed.drive && parsed.car.is_none() {
            eprintln!("--drive needs --car <veh>; the track needs a car to drive");
            return ExitCode::from(2);
        }
        run_track(
            &path,
            parsed.car.as_deref().map(Path::new),
            parsed.grid.unwrap_or(0),
            parsed.drive,
            parsed.ground_probe,
            parsed.road_follow,
            parsed.barrier_stop,
        )
    } else {
        if parsed.drive || parsed.ground_probe || parsed.road_follow || parsed.barrier_stop {
            eprintln!(
                "--drive, --ground-probe, --road-follow and --barrier-stop need a track .SCN and --car <veh>"
            );
            return ExitCode::from(2);
        }
        run_mas(&path.to_string_lossy(), parsed.model)
    }
}

/// `.mas` mode: show one model of the archive.
fn run_mas(path: &str, requested: Option<String>) -> ExitCode {
    let archive = match MasArchive::open(path) {
        Ok(archive) => archive,
        Err(error) => {
            eprintln!("Cannot open {path}: {error}. Check the path.");
            return ExitCode::from(1);
        }
    };

    let mas_path = PathBuf::from(path);
    let texture_paths = resolve_search_path(&mas_path);
    let index = TextureIndex::build(&texture_paths);
    println!("Search path: {}", search_path_names(&texture_paths));

    let mut viewable = Vec::new();
    for name in model_names(&archive) {
        let Some(entry) = archive.find(&name) else {
            continue;
        };
        let Ok(bytes) = archive.read(entry) else {
            eprintln!("skipping {name}: cannot read entry");
            continue;
        };
        if let Some(model) = load_model(name, &bytes, &index) {
            viewable.push(model);
        }
    }

    if viewable.is_empty() {
        eprintln!("No viewable models in {path}.");
        return ExitCode::from(1);
    }

    let index_selected = requested
        .as_ref()
        .and_then(|name| {
            viewable
                .iter()
                .position(|model| model.name.eq_ignore_ascii_case(name))
        })
        .unwrap_or(0);

    println!(
        "viewer: {} viewable model(s); starting at {}",
        viewable.len(),
        viewable[index_selected].name
    );
    for model in &viewable {
        println!(
            "{}: Textures {}/{} found",
            model.name, model.textures_found, model.textures_total
        );
    }

    let state = ViewerState {
        index: index_selected,
        needs_load: true,
        textures_on: true,
        center: Vec3::ZERO,
        radius: 1.0,
        distance: 3.0,
        yaw: 0.7,
        pitch: 0.35,
        fly: false,
        position: Vec3::ZERO,
        speed: 20.0,
        yaw_sign: 1.0,
    };

    run_app(
        format!("F1C Viewer - {path}"),
        ViewerModels(viewable),
        index,
        state,
        None,
        None,
        None,
        None,
        None,
        [20, 20, 30],
    )
}

/// The assembled car: sub-meshes, materials, texture search path and log lines.
struct CarData {
    name: String,
    submeshes: Vec<SubMesh>,
    materials: Vec<MaterialInfo>,
    search_paths: Vec<PathBuf>,
    lines: Vec<String>,
    /// Physics summary block from `formats-hdv`.
    physics_lines: Vec<String>,
    total: usize,
    missing: usize,
}

/// Assemble the whole car from a `.veh` file. `None` when it cannot be read.
fn build_car(veh_path: &Path) -> Option<CarData> {
    let veh_name = file_name(veh_path);
    let veh_text = read_latin1(veh_path)?;
    let gen_string = formats_gen::veh_gen_string(&veh_text).unwrap_or_default();
    let gen_text = veh_gen_text(veh_path).unwrap_or_default();
    let physics_lines = formats_hdv::CarPhysics::load(veh_path)
        .map(|physics| physics.summary_lines())
        .unwrap_or_else(|error| vec![format!("Physics: unavailable ({error})")]);

    let search_paths = resolve_search_path_for_veh(veh_path);
    let mesh_index = MeshIndex::build(&search_paths);
    // BACKFIRE is the exhaust flame effect, only drawn when it fires.
    let meshes: Vec<_> = formats_gen::car_meshes(&gen_text, &gen_string)
        .into_iter()
        .filter(|mesh| mesh.instance != "BACKFIRE")
        .collect();

    let mut archives: HashMap<PathBuf, MasArchive> = HashMap::new();
    let mut submeshes: Vec<SubMesh> = Vec::new();
    let mut materials: Vec<MaterialInfo> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut missing = 0;

    for mesh in &meshes {
        let Some((archive_path, entry_name)) = mesh_index.find(&mesh.mesh) else {
            missing += 1;
            lines.push(format!("{} {} missing", mesh.instance, mesh.mesh));
            continue;
        };
        let from = file_name(&archive_path);
        lines.push(format!(
            "{} {} found (from {})",
            mesh.instance, mesh.mesh, from
        ));

        if !archives.contains_key(&archive_path) {
            match MasArchive::open(&archive_path) {
                Ok(archive) => {
                    archives.insert(archive_path.clone(), archive);
                }
                Err(error) => {
                    eprintln!("skipping {}: cannot open {from}: {error}", mesh.mesh);
                    continue;
                }
            }
        }
        let bytes = {
            let archive = &archives[&archive_path];
            let Some(entry) = archive.find(&entry_name) else {
                continue;
            };
            match archive.read(entry) {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("skipping {}: {error}", mesh.mesh);
                    continue;
                }
            }
        };
        let mts = match formats_mts::parse(&bytes) {
            Ok(mts) => mts,
            Err(error) => {
                eprintln!("skipping {}: {error}", mesh.mesh);
                continue;
            }
        };

        let base = materials.len() as u32;
        for material in &mts.materials {
            materials.push(material_info(material));
        }
        for mut sub in mts_to_submeshes_at(&mts, mts.position) {
            sub.material_index += base;
            submeshes.push(sub);
        }
    }

    Some(CarData {
        name: veh_name,
        submeshes,
        materials,
        search_paths,
        lines,
        physics_lines,
        total: meshes.len(),
        missing,
    })
}

/// `.veh` mode: assemble and show the whole car.
fn run_car(veh_path: &Path) -> ExitCode {
    let Some(car) = build_car(veh_path) else {
        eprintln!("Cannot read {}. Check the path.", file_name(veh_path));
        return ExitCode::from(1);
    };

    println!("Search path: {}", search_path_names(&car.search_paths));
    println!(
        "Car: {}, {} meshes, {} missing",
        car.name, car.total, car.missing
    );
    for line in &car.physics_lines {
        println!("{line}");
    }
    for line in &car.lines {
        println!("{line}");
    }

    let texture_index = TextureIndex::build(&car.search_paths);
    let (textures_found, textures_total) =
        texture_counts(&car.submeshes, &car.materials, &texture_index);
    let model = ModelData {
        name: car.name.clone(),
        submeshes: car.submeshes,
        sky: Vec::new(),
        materials: car.materials,
        textures_found,
        textures_total,
    };

    let state = ViewerState {
        index: 0,
        needs_load: true,
        textures_on: true,
        center: Vec3::ZERO,
        radius: 1.0,
        distance: 3.0,
        yaw: 0.7,
        pitch: 0.35,
        fly: false,
        position: Vec3::ZERO,
        speed: 20.0,
        yaw_sign: 1.0,
    };

    run_app(
        format!("F1C Viewer - {}", car.name),
        ViewerModels(vec![model]),
        texture_index,
        state,
        None,
        None,
        None,
        None,
        None,
        [20, 20, 30],
    )
}

/// `.SCN` mode: show every visible track mesh in one fly-camera view.
///
/// With `--car <veh>` the assembled car is placed on grid slot `grid` of the
/// sibling `.aiw` and the fly camera starts 8 m behind and 3 m above it. `R`
/// toggles the inferred yaw sign at run time.
///
/// With `drive` true the car is validated with `physics-drive` and becomes a
/// dynamic car on the flat plane at the grid's Y; the fly/orbit controls are
/// replaced by the drive controls. A bad car or missing grid fails before the
/// window opens.
#[allow(clippy::too_many_arguments)]
fn run_track(
    scn_path: &Path,
    car_path: Option<&Path>,
    grid: u32,
    drive: bool,
    ground_probe: bool,
    road_follow: bool,
    barrier_stop: bool,
) -> ExitCode {
    let track_name = file_name(scn_path);
    let text = match read_latin1(scn_path) {
        Some(text) => text,
        None => {
            eprintln!("Cannot read {track_name}. Check the path.");
            return ExitCode::from(1);
        }
    };
    let scene = formats_scn::parse(&text);
    let (mas_paths, missing) = resolve_scene_mas(scn_path, &scene);
    println!(
        "Track: {}, MAS: {} found, {} missing",
        track_name,
        mas_paths.len(),
        missing.len()
    );
    for name in &missing {
        println!("missing MAS: {name}");
    }

    // Sun: the first directional light; its direction is the way the light
    // travels in game axes, so viewer Z is negated.
    let sun = scene
        .lights
        .iter()
        .find(|light| light.light_type.eq_ignore_ascii_case("directional"));
    let sun_rgb = sun.and_then(|light| light.color).unwrap_or([255, 255, 255]);
    let sun_dir = sun
        .and_then(|light| light.dir)
        .map(|dir| Vec3::new(dir[0], dir[1], -dir[2]))
        .filter(|dir| dir.length_squared() > 0.0)
        .map(Vec3::normalize);
    match sun_dir {
        Some(dir) => println!(
            "Sun: dir ({:.3}, {:.3}, {:.3}) colour ({}, {}, {})",
            dir.x, dir.y, dir.z, sun_rgb[0], sun_rgb[1], sun_rgb[2]
        ),
        None => println!("Sun: none"),
    }

    let fog = scene
        .fog
        .as_ref()
        .filter(|fog| fog.mode.eq_ignore_ascii_case("linear"));
    let fog_rgb = fog.and_then(|fog| fog.color).unwrap_or([255, 255, 255]);
    match fog {
        Some(fog) => println!(
            "Fog: linear {:.0}-{:.0} colour ({}, {}, {})",
            fog.in_distance, fog.out_distance, fog_rgb[0], fog_rgb[1], fog_rgb[2]
        ),
        None => println!("Fog: none"),
    }

    let ambient_rgb = scene.ambient_color.unwrap_or([255, 255, 255]);
    let clear_rgb = scene.view_color.unwrap_or([160, 200, 235]);
    let far = scene.clip_far.map(|far| far.max(3000.0)).unwrap_or(12000.0);
    let lighting = TrackLighting {
        enabled: true,
        has_sun: sun_dir.is_some(),
        sun_dir: sun_dir.unwrap_or(Vec3::NEG_Z),
        sun_color: srgb8(sun_rgb),
        sun_illuminance: 8_000.0,
        ambient_color: srgb8(ambient_rgb),
        ambient_brightness: 300.0,
        fog: fog.map(|fog| (srgb8(fog_rgb), fog.in_distance, fog.out_distance)),
        far,
    };

    let sky_indices = sky_instance_indices(&scene);
    let mesh_index = MeshIndex::build(&mas_paths);
    let mut texture_paths = mas_paths.clone();

    let mut archives: HashMap<PathBuf, MasArchive> = HashMap::new();
    let mut submeshes: Vec<SubMesh> = Vec::new();
    let mut sky_flags: Vec<bool> = Vec::new();
    let mut materials: Vec<MaterialInfo> = Vec::new();
    let mut shown = 0usize;
    let mut skipped = 0usize;
    let mut missing_meshes = 0usize;
    let mut unsupported = 0usize;
    let mut track_bounds = None;
    let mut sky_ring = false;
    let mut sky_dome = false;

    for (index, instance) in scene.instances.iter().enumerate() {
        let is_sky = sky_indices.contains(&index);
        let visible = instance.render && (!instance.moveable && !instance.animated || is_sky);
        let is_ring = instance.name.eq_ignore_ascii_case("skyboxi");
        let is_dome = instance.name.eq_ignore_ascii_case("clouds");
        for mesh in &instance.meshes {
            if !visible {
                skipped += 1;
                continue;
            }
            let Some((archive_path, entry_name)) = mesh_index.find(mesh) else {
                missing_meshes += 1;
                continue;
            };
            if !archives.contains_key(&archive_path) {
                match MasArchive::open(&archive_path) {
                    Ok(archive) => {
                        archives.insert(archive_path.clone(), archive);
                    }
                    Err(error) => {
                        eprintln!(
                            "skipping {mesh}: cannot open {}: {error}",
                            file_name(&archive_path)
                        );
                        unsupported += 1;
                        continue;
                    }
                }
            }
            let bytes = {
                let archive = &archives[&archive_path];
                let Some(entry) = archive.find(&entry_name) else {
                    missing_meshes += 1;
                    continue;
                };
                match archive.read(entry) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        eprintln!("skipping {mesh}: {error}");
                        unsupported += 1;
                        continue;
                    }
                }
            };
            let mts = match formats_mts::parse(&bytes) {
                Ok(mts) => mts,
                Err(error) => {
                    eprintln!("skipping {mesh}: {error}");
                    unsupported += 1;
                    continue;
                }
            };

            let parts = mts_to_submeshes_at(&mts, mts.position);
            if track_bounds.is_none() && mesh.to_ascii_lowercase().starts_with("track") {
                track_bounds = bounds(&parts);
            }
            let base = materials.len() as u32;
            for material in &mts.materials {
                materials.push(material_info(material));
            }
            for mut part in parts {
                part.material_index += base;
                submeshes.push(part);
                sky_flags.push(is_sky);
            }
            if is_sky {
                sky_ring |= is_ring;
                sky_dome |= is_dome;
            }
            shown += 1;
        }
    }

    println!(
        "Meshes: shown {shown}, skipped {skipped} (hidden/moveable/animated), \
         missing {missing_meshes}, unsupported {unsupported}"
    );
    println!(
        "Sky: {}",
        match (sky_ring, sky_dome) {
            (true, true) => "ring + dome",
            (true, false) => "ring",
            (false, true) => "dome",
            (false, false) => "none",
        }
    );

    if submeshes.is_empty() {
        eprintln!("No viewable track meshes in {track_name}.");
        return ExitCode::from(1);
    }

    let scene_bounds = bounds(&submeshes);
    let target = track_bounds
        .or(scene_bounds)
        .map(|bounds| (Vec3::from(bounds.min) + Vec3::from(bounds.max)) * 0.5)
        .unwrap_or(Vec3::ZERO);
    let radius = scene_bounds
        .map(|bounds| (Vec3::from(bounds.max) - Vec3::from(bounds.min)).length() * 0.5)
        .unwrap_or(100.0)
        .max(10.0);
    // prototype camera start, also the fallback car spot when no grid slot exists.
    let ds07_position = target + Vec3::new(0.0, radius * 1.2, radius * 0.6);
    let ds07_look = (target - ds07_position).normalize_or_zero();
    let mut position = ds07_position;
    let mut yaw = ds07_look.x.atan2(ds07_look.z);
    let mut pitch = ds07_look.y.clamp(-1.0, 1.0).asin();

    let mut car_placement = None;
    let mut drive_setup = None;
    if let Some(veh_path) = car_path {
        if let Some(car) = build_car(veh_path) {
            texture_paths.extend(car.search_paths.iter().cloned());
            if drive {
                match prepare_drive(veh_path, car, scn_path, grid) {
                    Ok(prepared) => drive_setup = Some(prepared),
                    Err(message) => {
                        eprintln!("drive: {message}");
                        return ExitCode::from(1);
                    }
                }
            } else {
                let (car_pos, car_ori, on_grid) = match resolve_grid(scn_path, grid) {
                    Some((pos, ori)) => (pos, ori, true),
                    None => {
                        println!(
                            "No grid {grid} in {}; placing car at the prototype camera start",
                            file_name(&scn_path.with_extension("aiw"))
                        );
                        (ds07_position.to_array(), [0.0, 0.0, 0.0], false)
                    }
                };
                let yaw_sign = 1.0;

                let material_base = materials.len() as u32;
                let mut base = car.submeshes.clone();
                for sub in &mut base {
                    sub.material_index += material_base;
                }
                let mut placed = car.submeshes;
                for sub in &mut placed {
                    sub.material_index += material_base;
                }
                place_car(&mut placed, car_pos, car_ori, yaw_sign);

                let car_start = submeshes.len();
                let placed_len = placed.len();
                submeshes.extend(placed);
                sky_flags.extend(std::iter::repeat_n(false, placed_len));
                for material in &car.materials {
                    materials.push(material.clone());
                }

                if let Some(car_bounds) = bounds(&submeshes[car_start..]) {
                    let center = (Vec3::from(car_bounds.min) + Vec3::from(car_bounds.max)) * 0.5;
                    let forward = Vec3::from(car_forward(car_ori, yaw_sign));
                    position = center - forward * 8.0 + Vec3::Y * 3.0;
                    let direction = (center - position).normalize_or_zero();
                    yaw = direction.x.atan2(direction.z);
                    pitch = direction.y.clamp(-1.0, 1.0).asin();
                }

                let yaw_angle = car_ori[1] * yaw_sign;
                if on_grid {
                    println!(
                        "Car: {} on grid {grid} at ({:.3}, {:.3}, {:.3}), yaw {:.3}",
                        car.name, car_pos[0], car_pos[1], car_pos[2], yaw_angle
                    );
                } else {
                    println!(
                        "Car: {} at ({:.3}, {:.3}, {:.3}), yaw {:.3}",
                        car.name, car_pos[0], car_pos[1], car_pos[2], yaw_angle
                    );
                }
                println!("Yaw sign: +1  (press R to toggle)");
                for line in &car.physics_lines {
                    println!("{line}");
                }

                car_placement = Some(CarPlacement {
                    base,
                    start: car_start,
                    pos: car_pos,
                    ori: car_ori,
                });
            }
        } else {
            eprintln!("Cannot read {}. Check the path.", file_name(veh_path));
            if drive {
                return ExitCode::from(1);
            }
        }
    } else if drive {
        eprintln!("drive: --car <veh> is required with --drive");
        return ExitCode::from(1);
    }

    let mut ground = None;
    let mut road_map = None;
    if ground_probe || road_follow {
        let build = match build_ground_query(&scene, &mesh_index) {
            Ok(build) => build,
            Err(message) => {
                eprintln!("ground probe: {message}");
                return ExitCode::from(1);
            }
        };
        // Road-follow overrides probe-only labelling when both are supplied.
        let label = if road_follow {
            "Road follow"
        } else {
            "Ground probe"
        };
        println!(
            "{label}: selected {} meshes, excluded {}, unsupported {}",
            build.selected, build.excluded, build.unsupported
        );
        let stats = build.set.stats();
        println!(
            "{label}: triangles retained {}, rejected {} (degenerate {}, steep {})",
            stats.retained,
            stats.rejected(),
            stats.degenerate,
            stats.steep
        );
        let (rear_x, rear_z, grid_y) = {
            let Some((_, runtime)) = drive_setup.as_ref() else {
                eprintln!("{label}: --drive setup is required");
                return ExitCode::from(1);
            };
            let [rear_x, rear_z] = runtime.session.sim.rear_axle();
            (rear_x, rear_z, runtime.session.sim.spawn.height)
        };
        if road_follow {
            let sim = &drive_setup.as_ref().unwrap().1.session.sim;
            let follower = match RoadFollower::new(&build.set, sim) {
                Ok(follower) => follower,
                Err(error) => {
                    eprintln!("road follow: {error}");
                    return ExitCode::from(1);
                }
            };
            let sample = follower.surface();
            println!(
                "Road follow: spawn surface at rear axle ({rear_x:.3}, {rear_z:.3}) \
                 height {:.3} m, grid reference {grid_y:.3} m, difference {:+.3} m, \
                 normal ({:.3}, {:.3}, {:.3}), source {}, candidates {}",
                sample.height,
                sample.height - grid_y,
                sample.normal[0],
                sample.normal[1],
                sample.normal[2],
                sample.name,
                sample.candidates
            );
            drive_setup
                .as_mut()
                .unwrap()
                .1
                .session
                .set_road_follow(follower);

            if barrier_stop {
                let barrier = match build_barrier_query(&scene, &mesh_index) {
                    Ok(barrier) => barrier,
                    Err(message) => {
                        eprintln!("barrier stop: {message}");
                        return ExitCode::from(1);
                    }
                };
                println!(
                    "Barrier stop: selected {} meshes, excluded {}, unsupported {}",
                    barrier.selected, barrier.excluded, barrier.unsupported
                );
                let stats = barrier.set.stats();
                println!(
                    "Barrier stop: triangles retained {}, rejected {} (degenerate {}, shallow {})",
                    stats.retained,
                    stats.rejected(),
                    stats.degenerate,
                    stats.horizontal
                );
                match diagnostic_barrier_sweep(&barrier.set) {
                    Ok(diagnostic) => println!(
                        "Barrier stop: diagnostic sweep {} -> {} (occurrence {}) radius {:.2} m, \
                         t {:.4}, candidates {}",
                        sweep_point(diagnostic.start),
                        sweep_point(diagnostic.end),
                        diagnostic.source.occurrence,
                        PROXY_RADIUS,
                        diagnostic.t,
                        diagnostic.candidates
                    ),
                    Err(message) => eprintln!("Barrier stop: diagnostic sweep failed: {message}"),
                }
                let session = &drive_setup.as_ref().unwrap().1.session;
                let follower_ref = session
                    .follower()
                    .expect("road follower is set before the barrier check");
                let center = proxy_center(follower_ref.pose());
                let guard = match BarrierStop::new(barrier.set, follower_ref) {
                    Ok(guard) => guard,
                    Err(error) => {
                        eprintln!("barrier stop: {error}");
                        return ExitCode::from(1);
                    }
                };
                println!(
                    "Barrier stop: proxy centre ({:.3}, {:.3}, {:.3}), radius {:.2} m, \
                     centre offset Y {:.2} m, spawn overlap clear",
                    center[0], center[1], center[2], PROXY_RADIUS, PROXY_OFFSET_Y
                );
                drive_setup
                    .as_mut()
                    .unwrap()
                    .1
                    .session
                    .set_barrier_stop(guard);
            }
            road_map = Some(RoadFollowMap { set: build.set });
        } else {
            let result = match build.set.query(rear_x, rear_z, grid_y, 2.0) {
                Ok(result) => result,
                Err(error) => {
                    eprintln!("ground probe: {error}");
                    return ExitCode::from(1);
                }
            };
            match &result.hit {
                Some(hit) => println!(
                    "Ground probe: spawn hit at rear axle ({rear_x:.3}, {rear_z:.3}) \
                     height {:.3} m, reference {grid_y:.3} m, difference {:+.3} m, \
                     normal ({:.3}, {:.3}, {:.3}), source {}, candidates {}",
                    hit.height,
                    hit.height - grid_y,
                    hit.normal[0],
                    hit.normal[1],
                    hit.normal[2],
                    hit.name,
                    result.candidates
                ),
                None => {
                    eprintln!(
                        "No nearby surface at grid; choose another grid or use flat drive without --ground-probe"
                    );
                    return ExitCode::from(1);
                }
            }
            ground = Some(GroundProbe {
                set: build.set,
                reference_y: grid_y,
            });
        }
    }

    let texture_index = TextureIndex::build(&texture_paths);
    let (textures_found, textures_total) = texture_counts(&submeshes, &materials, &texture_index);
    println!("Textures {textures_found}/{textures_total} found");

    let model = ModelData {
        name: track_name.clone(),
        submeshes,
        sky: sky_flags,
        materials,
        textures_found,
        textures_total,
    };
    let state = ViewerState {
        index: 0,
        needs_load: true,
        textures_on: true,
        center: target,
        radius,
        distance: radius * 3.0,
        yaw,
        pitch,
        fly: true,
        position,
        speed: (radius * 0.5).max(20.0),
        yaw_sign: 1.0,
    };

    run_app(
        format!("F1C Viewer - {track_name}"),
        ViewerModels(vec![model]),
        texture_index,
        state,
        car_placement,
        drive_setup,
        ground,
        road_map,
        Some(lighting),
        clear_rgb,
    )
}

/// Build the validated drive resources for `--drive`, or a clear error.
///
/// Rejects a missing grid slot, an unreadable car, unsupported drivetrain and
/// any missing required physics field. Nothing is placed until this succeeds.
fn prepare_drive(
    veh_path: &Path,
    car: CarData,
    scn_path: &Path,
    grid: u32,
) -> Result<(DriveCar, DriveRuntime), String> {
    let (pos, ori) = resolve_grid(scn_path, grid).ok_or_else(|| {
        format!(
            "grid {grid} not found in {}; use an existing --grid",
            file_name(&scn_path.with_extension("aiw"))
        )
    })?;

    let physics = formats_hdv::CarPhysics::load(veh_path)
        .map_err(|error| format!("cannot read {}: {error}", file_name(veh_path)))?;
    let spec = CarSpec::from_physics(&physics).map_err(|error| error.message)?;
    println!("drive: validated {}", file_name(veh_path));
    println!(
        "  mass {:.0} kg, gears {}, radii front/rear {:.3}/{:.3} m, wheelbase {:.3} m",
        spec.mass,
        spec.gear_count(),
        spec.front_radius,
        spec.rear_radius,
        spec.wheelbase
    );
    println!(
        "  final drive {:.3}, rev limit {:.0} RPM, idle {:.0} RPM, steer lock {:.1} deg",
        spec.final_drive,
        spec.rev_limit,
        spec.idle_rpm,
        spec.steer_lock_rad.to_degrees()
    );

    let spawn = Spawn {
        origin_x: f64::from(pos[0]),
        origin_z: f64::from(pos[2]),
        yaw: f64::from(ori[1]),
        height: f64::from(pos[1]),
    };
    let sim = Sim::new(spec, DriveParams::default(), spawn).map_err(|error| error.message)?;
    println!(
        "  spawn grid {grid} at ({:.3}, {:.3}, {:.3}), yaw {:.3}",
        pos[0], pos[1], pos[2], ori[1]
    );

    let mut local = car.submeshes;
    car_local(&mut local);
    let model = ModelData {
        name: car.name,
        submeshes: local,
        sky: Vec::new(),
        materials: car.materials,
        textures_found: 0,
        textures_total: 0,
    };
    Ok((
        DriveCar {
            model,
            needs_spawn: true,
        },
        DriveRuntime {
            session: Session::new(sim),
        },
    ))
}

/// Colour from three `0..255` channels.
fn srgb8(rgb: [u8; 3]) -> Color {
    Color::srgb(
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
    )
}

/// Open the window and run the render loop for the prepared models.
#[allow(clippy::too_many_arguments)]
fn run_app(
    title: String,
    models: ViewerModels,
    index: TextureIndex,
    state: ViewerState,
    car_placement: Option<CarPlacement>,
    drive: Option<(DriveCar, DriveRuntime)>,
    ground: Option<GroundProbe>,
    road: Option<RoadFollowMap>,
    lighting: Option<TrackLighting>,
    clear_rgb: [u8; 3],
) -> ExitCode {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window { title, ..default() }),
        ..default()
    }))
    .insert_resource(ClearColor(srgb8(clear_rgb)))
    .insert_resource(models)
    .insert_resource(Textures {
        index,
        archives: HashMap::new(),
        cache: HashMap::new(),
        grey: None,
    })
    .insert_resource(state);
    if let Some(ground) = ground {
        app.insert_resource(ground);
    }
    if let Some(road) = road {
        app.insert_resource(road);
    }
    match drive {
        Some((car, runtime)) => {
            app.insert_resource(car)
                .insert_resource(runtime)
                .init_resource::<DriveInput>()
                .add_systems(Startup, setup)
                .add_systems(
                    Update,
                    (
                        apply_state,
                        drive_sync_car,
                        drive_input,
                        drive_frame,
                        apply_lighting,
                        follow_sky,
                    )
                        .chain(),
                );
        }
        None => {
            app.add_systems(Startup, setup).add_systems(
                Update,
                (controls, apply_state, apply_lighting, follow_sky).chain(),
            );
        }
    }
    if let Some(car) = car_placement {
        app.insert_resource(car);
    }
    if let Some(lighting) = lighting {
        app.insert_resource(lighting);
    }
    app.run();

    ExitCode::SUCCESS
}

/// Parse one MTS entry and count its textures. `None` when unviewable.
fn load_model(name: String, bytes: &[u8], index: &TextureIndex) -> Option<ModelData> {
    let mts = match formats_mts::parse(bytes) {
        Ok(mts) => mts,
        Err(error) => {
            eprintln!("skipping {name}: {error}");
            return None;
        }
    };
    let submeshes = mts_to_submeshes(&mts);
    if submeshes.iter().all(|sub| sub.indices.is_empty()) {
        return None;
    }
    let materials: Vec<MaterialInfo> = mts.materials.iter().map(material_info).collect();
    let (textures_found, textures_total) = texture_counts(&submeshes, &materials, index);
    Some(ModelData {
        name,
        submeshes,
        sky: Vec::new(),
        materials,
        textures_found,
        textures_total,
    })
}

fn material_info(material: &formats_mts::Material) -> MaterialInfo {
    MaterialInfo {
        texture: material
            .stages
            .first()
            .map(|stage| stage.texture.clone())
            .unwrap_or_default(),
        frames: material
            .stages
            .first()
            .map(|stage| stage.frames)
            .unwrap_or(1),
        src_blend: material.src_blend,
    }
}

fn search_path_names(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| file_name(path))
        .collect::<Vec<_>>()
        .join(", ")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Format a game-axis point for the diagnostic sweep line.
fn sweep_point(point: [f64; 3]) -> String {
    format!("({:.3}, {:.3}, {:.3})", point[0], point[1], point[2])
}

/// Decode a file as Latin-1. `None` when it cannot be read.
fn read_latin1(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(bytes.into_iter().map(char::from).collect())
}

/// Count distinct used material textures that resolve in the index.
fn texture_counts(
    submeshes: &[SubMesh],
    materials: &[MaterialInfo],
    index: &TextureIndex,
) -> (usize, usize) {
    let mut seen = vec![false; materials.len()];
    let mut found = 0;
    let mut total = 0;
    for sub in submeshes {
        if sub.indices.is_empty() {
            continue;
        }
        let material_index = sub.material_index as usize;
        if seen.get(material_index).copied().unwrap_or(true) {
            continue;
        }
        seen[material_index] = true;
        let material = &materials[material_index];
        if material.texture.is_empty() {
            continue;
        }
        total += 1;
        if index.find(&material.texture, material.frames).is_some() {
            found += 1;
        }
    }
    (found, total)
}

fn setup(mut commands: Commands, state: Res<ViewerState>, lighting: Option<Res<TrackLighting>>) {
    let far = if state.fly {
        lighting
            .as_ref()
            .map(|lighting| lighting.far)
            .unwrap_or(12000.0)
    } else {
        PerspectiveProjection::default().far
    };
    let projection = if state.fly {
        Projection::Perspective(PerspectiveProjection { far, ..default() })
    } else {
        Projection::Perspective(PerspectiveProjection::default())
    };
    let camera_position = if state.fly {
        state.position
    } else {
        Vec3::new(0.0, 0.0, 3.0)
    };
    let scene_lighting = lighting
        .as_deref()
        .is_some_and(|lighting| lighting.enabled && lighting.has_sun);
    let (ambient_color, ambient_brightness) = match lighting.as_deref() {
        Some(lighting) if scene_lighting => (lighting.ambient_color, lighting.ambient_brightness),
        _ => (Color::WHITE, if state.fly { 350.0 } else { 80.0 }),
    };
    let mut camera = commands.spawn((
        Camera3d::default(),
        projection,
        AmbientLight {
            color: ambient_color,
            brightness: ambient_brightness,
            ..default()
        },
        Transform::from_translation(camera_position).looking_at(state.center, Vec3::Y),
    ));
    if state.fly {
        if let Some((color, start, end)) = lighting.as_deref().and_then(|lighting| lighting.fog) {
            camera.insert(DistanceFog {
                color,
                falloff: FogFalloff::Linear { start, end },
                ..default()
            });
        }
    }

    let (sun_color, sun_illuminance, sun_transform) = match lighting.as_deref() {
        Some(lighting) if scene_lighting => (
            lighting.sun_color,
            lighting.sun_illuminance,
            Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, lighting.sun_dir)),
        ),
        _ => (
            Color::WHITE,
            10_000.0,
            Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ),
    };
    commands.spawn((
        DirectionalLight {
            color: sun_color,
            illuminance: sun_illuminance,
            shadow_maps_enabled: !state.fly,
            ..default()
        },
        SunLight,
        sun_transform,
    ));

    // A dark translucent panel keeps the HUD readable over bright scenery.
    commands.spawn((
        Text::new("Loading..."),
        TextFont {
            font_size: 18.0.into(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            padding: UiRect::all(px(8)),
            max_width: px(760),
            ..default()
        },
        StatsText,
    ));
}

/// Move the sky sub-meshes so they stay centred on the camera in X and Z,
/// keeping their own Y.
fn follow_sky(
    camera: Query<&Transform, (With<Camera3d>, Without<SkyMesh>)>,
    mut sky: Query<&mut Transform, With<SkyMesh>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    for mut transform in &mut sky {
        transform.translation.x = camera.translation.x;
        transform.translation.z = camera.translation.z;
    }
}

/// Retune the ambient and sun lights when `L` switches SCN lighting off / on.
fn apply_lighting(
    lighting: Option<Res<TrackLighting>>,
    mut ambient: Query<&mut AmbientLight, With<Camera3d>>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<SunLight>>,
) {
    let Some(lighting) = lighting else {
        return;
    };
    let scn = lighting.enabled && lighting.has_sun;
    if let Ok(mut ambient) = ambient.single_mut() {
        if scn {
            ambient.color = lighting.ambient_color;
            ambient.brightness = lighting.ambient_brightness;
        } else {
            ambient.color = Color::WHITE;
            ambient.brightness = 350.0;
        }
    }
    if let Ok((mut light, mut transform)) = sun.single_mut() {
        if scn {
            light.color = lighting.sun_color;
            light.illuminance = lighting.sun_illuminance;
            *transform =
                Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, lighting.sun_dir));
        } else {
            light.color = Color::WHITE;
            light.illuminance = 10_000.0;
            *transform = Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn controls(
    input: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    time: Res<Time>,
    mut models: ResMut<ViewerModels>,
    mut state: ResMut<ViewerState>,
    car_placement: Option<Res<CarPlacement>>,
    lighting: Option<ResMut<TrackLighting>>,
    mut exit: MessageWriter<AppExit>,
) {
    let count = models.0.len();
    if input.just_pressed(KeyCode::ArrowRight) {
        state.index = (state.index + 1) % count;
        state.needs_load = true;
    }
    if input.just_pressed(KeyCode::ArrowLeft) {
        state.index = (state.index + count - 1) % count;
        state.needs_load = true;
    }
    if input.just_pressed(KeyCode::KeyT) {
        state.textures_on = !state.textures_on;
        state.needs_load = true;
    }
    if input.just_pressed(KeyCode::KeyR) {
        if let Some(car) = car_placement.as_ref() {
            state.yaw_sign = -state.yaw_sign;
            let mut placed = car.base.clone();
            place_car(&mut placed, car.pos, car.ori, state.yaw_sign);
            if let Some(model) = models.0.first_mut() {
                for (offset, sub) in placed.into_iter().enumerate() {
                    if let Some(target) = model.submeshes.get_mut(car.start + offset) {
                        *target = sub;
                    }
                }
            }
            state.needs_load = true;
            println!("Yaw sign: {:+}", state.yaw_sign as i32);
        }
    }
    if input.just_pressed(KeyCode::KeyL) {
        if let Some(mut lighting) = lighting {
            lighting.enabled = !lighting.enabled;
            println!(
                "Lighting: {}",
                if lighting.enabled { "SCN" } else { "default" }
            );
        }
    }
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }

    if state.fly {
        for event in motion.read() {
            if buttons.pressed(MouseButton::Right) {
                state.yaw -= event.delta.x * 0.005;
                state.pitch = (state.pitch - event.delta.y * 0.005).clamp(-1.5, 1.5);
            }
        }

        for event in wheel.read() {
            state.speed = (state.speed * 0.9f32.powf(event.y)).clamp(1.0, 100_000.0);
        }

        let forward = fly_direction(state.yaw, state.pitch);
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let boost = if input.pressed(KeyCode::ShiftLeft) || input.pressed(KeyCode::ShiftRight) {
            5.0
        } else {
            1.0
        };
        let step = state.speed * boost * time.delta_secs();
        let mut delta = Vec3::ZERO;
        if input.pressed(KeyCode::KeyW) {
            delta += forward;
        }
        if input.pressed(KeyCode::KeyS) {
            delta -= forward;
        }
        if input.pressed(KeyCode::KeyD) {
            delta += right;
        }
        if input.pressed(KeyCode::KeyA) {
            delta -= right;
        }
        if input.pressed(KeyCode::KeyE) {
            delta += Vec3::Y;
        }
        if input.pressed(KeyCode::KeyQ) {
            delta -= Vec3::Y;
        }
        state.position += delta * step;
        return;
    }

    let dragging = buttons.pressed(MouseButton::Left);
    for event in motion.read() {
        if dragging {
            state.yaw -= event.delta.x * 0.005;
            state.pitch = (state.pitch + event.delta.y * 0.005).clamp(-1.5, 1.5);
        }
    }

    let min_distance = (state.radius * 0.5).max(0.1);
    let max_distance = (state.radius * 20.0).max(min_distance);
    for event in wheel.read() {
        state.distance = (state.distance * 0.9f32.powf(event.y)).clamp(min_distance, max_distance);
    }
}

/// Unit look direction from yaw and pitch (matches the orbit camera).
fn fly_direction(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    )
}

#[allow(clippy::too_many_arguments)]
fn apply_state(
    mut commands: Commands,
    models: Res<ViewerModels>,
    mut state: ResMut<ViewerState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<Textures>,
    model_query: Query<Entity, With<Model>>,
    mut camera_query: Query<&mut Transform, (With<Camera3d>, Without<Model>)>,
    mut text_query: Query<&mut Text, With<StatsText>>,
) {
    if state.needs_load {
        for entity in &model_query {
            commands.entity(entity).despawn();
        }

        let model = &models.0[state.index];
        for (sub_index, submesh) in model.submeshes.iter().enumerate() {
            if submesh.indices.is_empty() {
                continue;
            }
            let is_sky = model.sky.get(sub_index).copied().unwrap_or(false);
            let material = material_for(
                model,
                submesh,
                state.textures_on,
                is_sky,
                &mut textures,
                &mut images,
                &mut materials,
            );
            let mut entity = commands.spawn((
                Mesh3d(meshes.add(build_mesh(submesh))),
                MeshMaterial3d(material),
                Model,
            ));
            if is_sky {
                entity.insert(SkyMesh);
            }
        }

        if !state.fly {
            if let Some(bounds) = bounds(&model.submeshes) {
                let min = Vec3::from(bounds.min);
                let max = Vec3::from(bounds.max);
                state.center = (min + max) * 0.5;
                state.radius = ((max - min).length() * 0.5).max(0.1);
            }
            state.distance = state.radius * 3.0;
            state.yaw = 0.7;
            state.pitch = 0.35;
        }
        state.needs_load = false;
    }

    if let Ok(mut transform) = camera_query.single_mut() {
        if state.fly {
            let direction = fly_direction(state.yaw, state.pitch);
            transform.translation = state.position;
            if direction.length_squared() > 0.0 {
                transform.look_at(state.position + direction, Vec3::Y);
            }
        } else {
            let direction = fly_direction(state.yaw, state.pitch);
            transform.translation = state.center + direction * state.distance;
            transform.look_at(state.center, Vec3::Y);
        }
    }

    if let Ok(mut text) = text_query.single_mut() {
        let model = &models.0[state.index];
        let triangles: usize = model
            .submeshes
            .iter()
            .map(|sub| sub.indices.len() / 3)
            .sum();
        let texture_line = if state.textures_on {
            format!(
                "Textures {}/{} found  T: toggle",
                model.textures_found, model.textures_total
            )
        } else {
            "Textures off  T: toggle".to_string()
        };
        let controls_line = if state.fly {
            "WASD: fly  Q/E: down/up  Right-drag: look  Shift: 5x  Wheel: speed  R: yaw sign"
        } else {
            "Left/Right: change model  Drag: orbit  Wheel: zoom"
        };
        text.0 = format!(
            "Model {}/{}: {}\nTriangles {}\n{}\n{}",
            state.index + 1,
            models.0.len(),
            model.name,
            thousands(triangles),
            texture_line,
            controls_line,
        );
    }
}

/// Rebuild the car entities from local geometry when needed, preserving the
/// simulation. Called on the first frame and after a texture toggle.
#[allow(clippy::too_many_arguments)]
fn drive_sync_car(
    mut commands: Commands,
    mut car: ResMut<DriveCar>,
    state: Res<ViewerState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<Textures>,
    parts: Query<Entity, With<DriveCarPart>>,
) {
    if !car.needs_spawn {
        return;
    }
    for entity in &parts {
        commands.entity(entity).despawn();
    }
    let model = &car.model;
    for submesh in &model.submeshes {
        if submesh.indices.is_empty() {
            continue;
        }
        let material = material_for(
            model,
            submesh,
            state.textures_on,
            false,
            &mut textures,
            &mut images,
            &mut materials,
        );
        commands.spawn((
            Mesh3d(meshes.add(build_mesh(submesh))),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::default(),
            DriveCarPart,
        ));
    }
    car.needs_spawn = false;
}

/// Sample the drive keys: queue gear edges, handle reset/pause/textures/lighting
/// and store the held pedals for the step loop.
#[allow(clippy::too_many_arguments)]
fn drive_input(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut runtime: ResMut<DriveRuntime>,
    mut state: ResMut<ViewerState>,
    mut car: ResMut<DriveCar>,
    lighting: Option<ResMut<TrackLighting>>,
    mut input: ResMut<DriveInput>,
    mut exit: MessageWriter<AppExit>,
) {
    // Focus loss pauses and clears input; the player must press P to resume.
    let focused = windows.iter().next().map(|w| w.focused).unwrap_or(true);
    if !focused {
        runtime.session.set_paused(true);
        input.held = HeldInput::default();
        return;
    }

    if keys.just_pressed(KeyCode::KeyT) {
        state.textures_on = !state.textures_on;
        state.needs_load = true;
        car.needs_spawn = true;
        println!("Textures: {}", if state.textures_on { "on" } else { "off" });
    }
    if keys.just_pressed(KeyCode::KeyL) {
        if let Some(mut lighting) = lighting {
            lighting.enabled = !lighting.enabled;
            println!(
                "Lighting: {}",
                if lighting.enabled { "SCN" } else { "default" }
            );
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }

    if keys.just_pressed(KeyCode::KeyP) {
        runtime.session.toggle_pause();
        input.held = HeldInput::default();
        return;
    }
    // Reset is allowed while paused and keeps the pause state.
    if keys.just_pressed(KeyCode::KeyR) {
        runtime.session.reset();
    }
    if runtime.session.paused {
        input.held = HeldInput::default();
        return;
    }

    // Shifts act once per press; simultaneous E/Q cancels.
    let up = keys.just_pressed(KeyCode::KeyE);
    let down = keys.just_pressed(KeyCode::KeyQ);
    if up && !down {
        runtime.session.request_shift_up();
    }
    if down && !up {
        runtime.session.request_shift_down();
    }

    input.held = HeldInput {
        throttle: keys.pressed(KeyCode::KeyW),
        brake: keys.pressed(KeyCode::KeyS),
        left: keys.pressed(KeyCode::KeyA),
        right: keys.pressed(KeyCode::KeyD),
    };
}

/// Advance the fixed-step simulation, then move the car, chase camera and HUD.
#[allow(clippy::too_many_arguments)]
fn drive_frame(
    time: Res<Time>,
    input: Res<DriveInput>,
    mut runtime: ResMut<DriveRuntime>,
    car: Res<DriveCar>,
    ground: Option<Res<GroundProbe>>,
    road: Option<Res<RoadFollowMap>>,
    mut parts: Query<&mut Transform, (With<DriveCarPart>, Without<Camera3d>)>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<DriveCarPart>)>,
    mut text: Query<&mut Text, With<StatsText>>,
) {
    match road.as_ref() {
        Some(map) => {
            runtime
                .session
                .advance_with_ground(time.delta_secs() as f64, input.held, &map.set);
        }
        None => {
            runtime
                .session
                .advance(time.delta_secs() as f64, input.held);
        }
    }

    // Road-follow renders the accepted tangent-plane pose; plain drive keeps the
    // flat grid placement.
    let transform = if let Some(follower) = runtime.session.follower() {
        let pose = follower.pose();
        let view = follow_view_transform(pose.origin, pose.left, pose.up, pose.forward);
        Transform {
            translation: Vec3::from(view.translation),
            rotation: Quat::from_mat3(&Mat3::from_cols(
                Vec3::from(view.x),
                Vec3::from(view.y),
                Vec3::from(view.z),
            )),
            ..default()
        }
    } else {
        let sim = &runtime.session.sim;
        let origin = sim.mesh_origin();
        let yaw = sim.state.yaw as f32;
        Transform {
            translation: Vec3::new(
                origin[0] as f32,
                sim.spawn.height as f32,
                -(origin[1] as f32),
            ),
            rotation: Quat::from_rotation_y(-yaw),
            ..default()
        }
    };
    for mut part in &mut parts {
        *part = transform;
    }

    // The chase camera follows the horizontal heading, not the slope.
    let yaw = runtime.session.sim.state.yaw as f32;
    let forward = Vec3::new(-yaw.sin(), 0.0, yaw.cos());
    let target = transform.translation + Vec3::Y;
    let position = transform.translation - forward * 8.0 + Vec3::Y * 3.0;
    if let Ok(mut camera) = camera.single_mut() {
        camera.translation = position;
        camera.look_at(target, Vec3::Y);
    }

    let ground_line = ground.as_ref().map(|probe| {
        let [rear_x, rear_z] = runtime.session.sim.rear_axle();
        match probe.set.query(rear_x, rear_z, probe.reference_y, 2.0) {
            Ok(result) => match &result.hit {
                Some(hit) => format!(
                    "Ground probe only — car stays on flat plane\n\
                     Ground {:.2} m   ref {:.2} m   difference {:+.2} m   {}   candidates {}",
                    hit.height,
                    probe.reference_y,
                    hit.height - probe.reference_y,
                    hit.name,
                    result.candidates
                ),
                None => {
                    "Ground probe only — car stays on flat plane\nNo nearby surface".to_string()
                }
            },
            Err(error) => {
                format!("Ground probe only — car stays on flat plane\nGround probe error: {error}")
            }
        }
    });

    if let Ok(mut text) = text.single_mut() {
        let state = &runtime.session.sim.state;
        let paused = if runtime.session.paused {
            "PAUSED"
        } else {
            "running"
        };
        let controls_line = format!(
            "W throttle {}  S brake {}   A/D steer",
            if input.held.throttle { 1 } else { 0 },
            if input.held.brake { 1 } else { 0 }
        );
        let footer =
            format!("E/Q shift  R reset  P pause  T textures  L lighting  Esc exit   {paused}");
        let mut lines = Vec::new();
        if let Some(follower) = runtime.session.follower() {
            let sample = follower.surface();
            let status = if follower.is_lost() {
                "LOST"
            } else {
                "following"
            };
            let barrier = runtime.session.barrier();
            if barrier.is_some() {
                lines.push("Barrier-stop prototype — sphere proxy, no damage".to_string());
            } else {
                lines.push("Road-follow prototype — no suspension or collisions".to_string());
            }
            lines.push(car.model.name.clone());
            lines.push(format!(
                "{:.0} km/h   {:.0} RPM   gear {}",
                state.speed * 3.6,
                state.rpm,
                state.gear.label()
            ));
            lines.push(format!(
                "surface {:.2} m   src {}   candidates {}   {status}",
                sample.height, sample.name, sample.candidates
            ));
            if let Some(barrier) = barrier {
                if let Some(contact) = barrier.last_contact() {
                    lines.push(format!(
                        "last barrier {} (occurrence {})   t {:.4}",
                        contact.name, contact.source.occurrence, contact.t
                    ));
                }
            }
            lines.push(controls_line);
            lines.push(footer);
            if runtime
                .session
                .barrier()
                .is_some_and(|barrier| barrier.is_stopped())
            {
                lines.push("Barrier stopped — R to reset".to_string());
            } else if follower.is_lost() {
                lines.push("Surface lost — R to reset".to_string());
            }
        } else {
            lines.push("Drive prototype — flat ground, no collisions".to_string());
            lines.push(car.model.name.clone());
            lines.push(format!(
                "{:.0} km/h   {:.0} RPM   gear {}",
                state.speed * 3.6,
                state.rpm,
                state.gear.label()
            ));
            lines.push(controls_line);
            lines.push(footer);
            if let Some(line) = ground_line {
                lines.push(line);
            }
        }
        text.0 = lines.join("\n");
    }
}

/// Build a `StandardMaterial` for one sub-mesh: textured when the material has
/// a resolvable texture and textures are on, grey otherwise. `unlit` marks sky
/// sub-meshes, which ignore the sun and fog so the horizon stays bright.
fn material_for(
    model: &ModelData,
    submesh: &SubMesh,
    textures_on: bool,
    unlit: bool,
    textures: &mut Textures,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    let info = model.materials.get(submesh.material_index as usize);
    let texture = if textures_on {
        info.and_then(|info| resolve_texture(textures, images, &info.texture, info.frames))
    } else {
        None
    };

    if let Some((texture, has_alpha)) = texture {
        let blend = info.is_some_and(|info| info.src_blend == 5);
        return materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(texture),
            perceptual_roughness: 0.8,
            unlit,
            fog_enabled: !unlit,
            // Opaque blend + alpha texture = alpha test (tree cards).
            alpha_mode: if blend {
                AlphaMode::Blend
            } else if has_alpha {
                AlphaMode::Mask(0.5)
            } else {
                AlphaMode::Opaque
            },
            ..default()
        });
    }

    if unlit {
        return materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.6, 0.6),
            unlit: true,
            fog_enabled: false,
            ..default()
        });
    }

    textures
        .grey
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.6, 0.6, 0.6),
                perceptual_roughness: 0.8,
                ..default()
            })
        })
        .clone()
}

/// Resolve, read, decode and cache one texture. Returns `None` when the name
/// is empty, unknown, or the file cannot be read or decoded.
/// Find, read and decode one texture to `(width, height, RGBA bytes)`.
fn load_rgba(textures: &mut Textures, name: &str, frames: u32) -> Option<(u32, u32, Vec<u8>)> {
    let (archive_path, entry_name) = textures.index.find(name, frames)?;
    if !textures.archives.contains_key(&archive_path) {
        let archive = MasArchive::open(&archive_path).ok()?;
        textures.archives.insert(archive_path.clone(), archive);
    }
    let archive = textures.archives.get(&archive_path)?;
    let bytes = archive.read(archive.find(&entry_name)?).ok()?;
    decode_rgba(&bytes, name).ok()
}

/// True when every pixel was colour-keyed (decoded as transparent black).
fn is_solid_magenta(rgba: &[u8]) -> bool {
    !rgba.is_empty() && rgba.as_chunks::<4>().0.iter().all(|p| p == &[0, 0, 0, 0])
}

fn resolve_texture(
    textures: &mut Textures,
    images: &mut Assets<Image>,
    name: &str,
    frames: u32,
) -> Option<(Handle<Image>, bool)> {
    if name.is_empty() {
        return None;
    }
    let key = name.to_ascii_lowercase();
    if let Some(cached) = textures.cache.get(&key) {
        return Some(cached.clone());
    }

    let mut decoded = load_rgba(textures, name, frames)?;
    // Some animated textures keep a solid magenta placeholder in frame 00
    // (e.g. TRE90G00 tire tread); show frame 01 instead.
    if frames > 1 && is_solid_magenta(&decoded.2) {
        if let Some(next) = load_rgba(textures, &format!("{name}01"), 1) {
            decoded = next;
        }
    }
    let (width, height, rgba) = decoded;
    let has_alpha = rgba.as_chunks::<4>().0.iter().any(|p| p[3] < 255);
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    // Game UVs leave 0..1 (e.g. v in -1..0) and rely on wrap addressing.

    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    let handle = images.add(image);
    textures.cache.insert(key, (handle.clone(), has_alpha));
    Some((handle, has_alpha))
}

fn build_mesh(submesh: &SubMesh) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, submesh.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, submesh.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, submesh.uvs.clone())
    .with_inserted_indices(Indices::U32(submesh.indices.clone()))
}

/// Group digits with commas: 1234 -> "1,234".
fn thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (position, digit) in digits.chars().enumerate() {
        if position > 0 && (digits.len() - position).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}
