//! Pure geometry and texture-lookup helpers for the F1C viewer.
//!
//! No Bevy types live here so the conversion can be unit-tested on its own.
//! See `specs/MTS_FORMAT.md`: F1C models are Direct3D left-handed; viewers
//! convert by negating Z; the mirror alone fixes winding (owner visual check, 2026-10-06). UVs are used as-is
//! (Direct3D and wgpu both put the texture origin at the top-left).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use formats_gen::{search_path, veh_graphics, GenDir};
use formats_mas::MasArchive;
use formats_mts::Mts;
use formats_scn::Scene;

pub use ground_query::{
    BarrierFace, BarrierSet, BarrierStats, BuildStats, GroundError, GroundQuerySet, GroupGeometry,
    Hit, MeshGeometry, QueryResult, SourceId, SweepHit, SweepResult,
};

/// One geometry group of a model, ready for a Bevy mesh.
///
/// `material_index` refers into the model's material list. Positions and
/// normals are already converted to right-handed space; `uvs` is untouched.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubMesh {
    pub material_index: u32,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// Convert every group of one model into its own sub-mesh, converting from the
/// file's left-handed space to right-handed: negate the Z of positions and
/// normals. Triangle order is kept: mirroring Z already turns Direct3D clockwise
/// front faces into counter-clockwise front faces.
pub fn mts_to_submeshes(mts: &Mts) -> Vec<SubMesh> {
    mts_to_submeshes_at(mts, [0.0, 0.0, 0.0])
}

/// Like [`mts_to_submeshes`] but adds the model's placement `offset` (from the
/// MTS geometry header) to every vertex before converting to right-handed
/// space, so parts with local geometry land in car space.
pub fn mts_to_submeshes_at(mts: &Mts, offset: [f32; 3]) -> Vec<SubMesh> {
    mts.groups
        .iter()
        .map(|group| {
            let mut sub = SubMesh {
                material_index: group.material_index,
                positions: Vec::with_capacity(group.vertices.len()),
                normals: Vec::with_capacity(group.vertices.len()),
                uvs: Vec::with_capacity(group.vertices.len()),
                indices: Vec::with_capacity(group.triangles.len() * 3),
            };
            for vertex in &group.vertices {
                sub.positions.push([
                    vertex.position[0] + offset[0],
                    vertex.position[1] + offset[1],
                    -(vertex.position[2] + offset[2]),
                ]);
                sub.normals
                    .push([vertex.normal[0], vertex.normal[1], -vertex.normal[2]]);
                sub.uvs.push(vertex.uv0);
            }
            for triangle in &group.triangles {
                sub.indices.push(u32::from(triangle[0]));
                sub.indices.push(u32::from(triangle[1]));
                sub.indices.push(u32::from(triangle[2]));
            }
            sub
        })
        .collect()
}

/// Indices of the sky instances to draw: the `skyboxi` ring and its `clouds`
/// child. Other moveable instances stay skipped. Empty when there is no ring.
pub fn sky_instance_indices(scene: &Scene) -> Vec<usize> {
    let Some(ring) = scene
        .instances
        .iter()
        .position(|instance| instance.name.eq_ignore_ascii_case("skyboxi"))
    else {
        return Vec::new();
    };
    let mut indices = vec![ring];
    for (index, instance) in scene.instances.iter().enumerate() {
        if instance.parent == Some(ring) && instance.name.eq_ignore_ascii_case("clouds") {
            indices.push(index);
        }
    }
    indices
}

/// Resolved DS12 ground query set plus selection counts for reporting.
pub struct GroundBuild {
    /// Built query set of retained selected triangles.
    pub set: GroundQuerySet,
    /// Mesh occurrences with explicit `HATTarget=True`.
    pub selected: usize,
    /// Mesh occurrences that were not selected.
    pub excluded: usize,
    /// Selected occurrences in unsupported contexts (zero on success).
    pub unsupported: usize,
}

/// Grid `Pos` and `Ori` of a `.SCN`'s sibling `.aiw`, if both exist.
pub fn resolve_grid(scn_path: &Path, grid: u32) -> Option<([f32; 3], [f32; 3])> {
    let aiw_path = scn_path.with_extension("aiw");
    let text = read_latin1(&aiw_path)?;
    formats_aiw::grid_slots(&text)
        .into_iter()
        .find(|slot| slot.index == grid)
        .map(|slot| (slot.pos, slot.ori))
}

/// How the DS12 selection rule treats one mesh occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshSelection {
    /// Explicit `HATTarget=True` in a supported context.
    Selected,
    /// Not explicitly selected, including absent/false/invalid `HATTarget`.
    Excluded,
    /// Selected but nested, moveable, animated or sky: unsupported.
    Unsupported,
}

/// Classify one occurrence under the conservative DS12 selection rule.
///
/// `Render` is deliberately not a parameter: a hidden (`Render=False`) mesh is
/// still selected. `CollTarget` is deliberately not a parameter either; only an
/// explicit `HATTarget=True` selects, so `CollTarget`-only is excluded. Callers
/// pass `hat_true` from the parsed `HATTarget` flag.
pub fn classify_mesh(
    hat_true: bool,
    nested: bool,
    moveable: bool,
    animated: bool,
    sky: bool,
) -> MeshSelection {
    if !hat_true {
        MeshSelection::Excluded
    } else if nested || moveable || animated || sky {
        MeshSelection::Unsupported
    } else {
        MeshSelection::Selected
    }
}

/// Build the DS12 ground query set from the selected scene meshes.
///
/// Selection is the conservative DS12 prototype rule: an explicit
/// `HATTarget=True` on a top-level, non-moveable, non-animated, non-sky mesh
/// occurrence. `Render=False` does not exclude a selected mesh; `CollTarget`
/// alone never selects one. A selected occurrence in an unsupported context
/// fails with a named error rather than being guessed.
pub fn build_ground_query(scene: &Scene, mesh_index: &MeshIndex) -> Result<GroundBuild, String> {
    let sky = sky_instance_indices(scene);
    let mut meshes: Vec<MeshGeometry> = Vec::new();
    let mut selected = 0usize;
    let mut excluded = 0usize;
    let mut unsupported = 0usize;
    let mut unsupported_error: Option<String> = None;
    let mut occurrence = 0u32;
    let mut archives: HashMap<PathBuf, MasArchive> = HashMap::new();

    for (index, instance) in scene.instances.iter().enumerate() {
        let is_sky = sky.contains(&index);
        for mesh_ref in &instance.mesh_refs {
            let identity = occurrence;
            occurrence += 1;
            let selection = classify_mesh(
                mesh_ref.hat_target.is_true(),
                instance.parent.is_some(),
                instance.moveable,
                instance.animated,
                is_sky,
            );
            match selection {
                MeshSelection::Excluded => {
                    excluded += 1;
                    continue;
                }
                MeshSelection::Unsupported => {
                    unsupported += 1;
                    unsupported_error.get_or_insert_with(|| {
                        format!(
                            "selected mesh {} uses HATTarget=True on unsupported \
                             nested/moveable/animated/sky geometry",
                            mesh_ref.name
                        )
                    });
                    continue;
                }
                MeshSelection::Selected => {}
            }
            selected += 1;
            meshes.push(load_ground_mesh(
                mesh_index,
                &mut archives,
                &mesh_ref.name,
                identity,
            )?);
        }
    }

    if let Some(message) = unsupported_error {
        return Err(format!(
            "{message} ({unsupported} unsupported occurrence(s))"
        ));
    }

    let set = GroundQuerySet::build(meshes).map_err(|error| error.to_string())?;
    Ok(GroundBuild {
        set,
        selected,
        excluded,
        unsupported,
    })
}

/// Read one selected mesh through the registered runtime loading path and
/// convert it to game-world ground geometry.
fn load_ground_mesh(
    mesh_index: &MeshIndex,
    archives: &mut HashMap<PathBuf, MasArchive>,
    name: &str,
    occurrence: u32,
) -> Result<MeshGeometry, String> {
    let (archive_path, entry_name) = mesh_index
        .find(name)
        .ok_or_else(|| format!("selected mesh {name} not found in the track archives"))?;
    if !archives.contains_key(&archive_path) {
        let archive = MasArchive::open(&archive_path).map_err(|error| {
            format!(
                "selected mesh {name}: cannot open {}: {error}",
                archive_path.display()
            )
        })?;
        archives.insert(archive_path.clone(), archive);
    }
    let archive = &archives[&archive_path];
    let entry = archive
        .find(&entry_name)
        .ok_or_else(|| format!("selected mesh {name} missing entry {entry_name}"))?;
    let bytes = archive
        .read(entry)
        .map_err(|error| format!("selected mesh {name}: {error}"))?;
    let mts =
        formats_mts::parse(&bytes).map_err(|error| format!("selected mesh {name}: {error}"))?;
    let groups = mts_to_submeshes_at(&mts, mts.position)
        .iter()
        .enumerate()
        .map(|(group, part)| GroupGeometry {
            group: group as u32,
            positions: ground_query::unmirror_positions(&part.positions),
            indices: part.indices.clone(),
        })
        .collect();
    Ok(MeshGeometry {
        occurrence,
        name: name.to_string(),
        groups,
    })
}

/// How the DS14 barrier selection rule treats one mesh occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierSelection {
    /// Explicit `CollTarget=True` on a visible, supported instance.
    Selected,
    /// Not explicitly selected, or hidden (`Render=False`), which includes the
    /// timing triggers the prototype deliberately omits.
    Excluded,
    /// Selected but nested, moveable, animated or sky: unsupported.
    Unsupported,
}

/// Classify one occurrence under the conservative DS14 barrier rule.
///
/// Only an explicit `CollTarget=True` on a `Render=True` occurrence selects.
/// `HATTarget` is deliberately not a parameter: a HAT-only mesh never selects.
/// `Render=False` occurrences (including hidden timing geometry) are excluded,
/// which can omit real invisible barriers and must be documented.
pub fn classify_barrier(
    coll_true: bool,
    render: bool,
    nested: bool,
    moveable: bool,
    animated: bool,
    sky: bool,
) -> BarrierSelection {
    if !coll_true || !render {
        BarrierSelection::Excluded
    } else if nested || moveable || animated || sky {
        BarrierSelection::Unsupported
    } else {
        BarrierSelection::Selected
    }
}

/// Resolved DS14 barrier set plus selection counts for reporting.
pub struct BarrierBuild {
    /// Built set of retained steep barrier triangles.
    pub set: BarrierSet,
    /// Mesh occurrences with explicit `CollTarget=True` in a supported context.
    pub selected: usize,
    /// Mesh occurrences that were not selected.
    pub excluded: usize,
    /// Selected occurrences in unsupported contexts (zero on success).
    pub unsupported: usize,
}

/// Build the DS14 barrier set from the selected scene meshes.
///
/// Selection is explicit `CollTarget=True` on a visible (`Render=True`),
/// top-level, non-moveable, non-animated, non-sky mesh occurrence. `HATTarget`
/// alone never selects. A selected occurrence in an unsupported context fails
/// with a named error rather than being guessed, and zero retained triangles is
/// reported by [`BarrierSet::build`].
pub fn build_barrier_query(scene: &Scene, mesh_index: &MeshIndex) -> Result<BarrierBuild, String> {
    let sky = sky_instance_indices(scene);
    let mut meshes: Vec<MeshGeometry> = Vec::new();
    let mut selected = 0usize;
    let mut excluded = 0usize;
    let mut unsupported = 0usize;
    let mut unsupported_error: Option<String> = None;
    let mut occurrence = 0u32;
    let mut archives: HashMap<PathBuf, MasArchive> = HashMap::new();

    for (index, instance) in scene.instances.iter().enumerate() {
        let is_sky = sky.contains(&index);
        for mesh_ref in &instance.mesh_refs {
            let identity = occurrence;
            occurrence += 1;
            let selection = classify_barrier(
                mesh_ref.coll_target.is_true(),
                instance.render,
                instance.parent.is_some(),
                instance.moveable,
                instance.animated,
                is_sky,
            );
            match selection {
                BarrierSelection::Excluded => {
                    excluded += 1;
                    continue;
                }
                BarrierSelection::Unsupported => {
                    unsupported += 1;
                    unsupported_error.get_or_insert_with(|| {
                        format!(
                            "selected barrier mesh {} uses CollTarget=True on unsupported \
                             nested/moveable/animated/sky geometry",
                            mesh_ref.name
                        )
                    });
                    continue;
                }
                BarrierSelection::Selected => {}
            }
            selected += 1;
            meshes.push(load_ground_mesh(
                mesh_index,
                &mut archives,
                &mesh_ref.name,
                identity,
            )?);
        }
    }

    if let Some(message) = unsupported_error {
        return Err(format!(
            "{message} ({unsupported} unsupported occurrence(s))"
        ));
    }

    let set = BarrierSet::build(meshes).map_err(|error| error.to_string())?;
    Ok(BarrierBuild {
        set,
        selected,
        excluded,
        unsupported,
    })
}

/// A reproducible headless sweep to a loaded retained triangle's face interior.
///
/// This proves the geometry query against real selected data; it is a
/// diagnostic, not evidence the car reaches that wall in a drive.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticSweep {
    /// Display name of the source mesh.
    pub name: String,
    /// Stable identity of the swept triangle.
    pub source: SourceId,
    /// Constructed sweep start centre.
    pub start: [f64; 3],
    /// Constructed sweep end centre.
    pub end: [f64; 3],
    /// Earliest contact fraction in `[0, 1]`.
    pub t: f64,
    /// Candidate triangles tested.
    pub candidates: usize,
}

/// Sweep the `0.75 m` sphere to the face interior of a loaded barrier triangle.
///
/// The segment runs from two metres off a retained face along its normal to the
/// face centroid, so a clean hit lands on that face's interior before the
/// centre reaches the surface. The first face that produces such a clean hit
/// (matching source, positive `t`) is reported; a named error is returned when
/// no retained triangle can be swept cleanly. This is a diagnostic, not
/// evidence the car reaches any wall in a drive.
pub fn diagnostic_barrier_sweep(set: &BarrierSet) -> Result<DiagnosticSweep, String> {
    const SEARCH_LIMIT: usize = 4096;
    let mut fallback: Option<DiagnosticSweep> = None;
    for index in 0..set.total_triangles().min(SEARCH_LIMIT) {
        let Some(face) = set.face(index) else {
            break;
        };
        let centroid = [
            (face.vertices[0][0] + face.vertices[1][0] + face.vertices[2][0]) / 3.0,
            (face.vertices[0][1] + face.vertices[1][1] + face.vertices[2][1]) / 3.0,
            (face.vertices[0][2] + face.vertices[1][2] + face.vertices[2][2]) / 3.0,
        ];
        let start = [
            centroid[0] + face.normal[0] * 2.0,
            centroid[1] + face.normal[1] * 2.0,
            centroid[2] + face.normal[2] * 2.0,
        ];
        let result = match set.sweep_sphere(start, centroid, 0.75) {
            Ok(result) => result,
            Err(_) => continue,
        };
        let Some(hit) = result.hit else {
            continue;
        };
        // A positive hit time proves the sweep approached rather than overlapped.
        if hit.t <= 1e-6 {
            continue;
        }
        let sweep = DiagnosticSweep {
            name: hit.name,
            source: hit.source,
            start,
            end: centroid,
            t: hit.t,
            candidates: result.candidates,
        };
        if hit.source == face.source {
            return Ok(sweep);
        }
        fallback.get_or_insert(sweep);
    }
    fallback.ok_or_else(|| "no retained barrier triangle produced a face sweep".to_string())
}

/// Bounding box over every position of every sub-mesh. `None` when empty.
pub fn bounds(submeshes: &[SubMesh]) -> Option<Bounds> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut any = false;
    for sub in submeshes {
        for position in &sub.positions {
            any = true;
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
        }
    }
    any.then_some(Bounds { min, max })
}

/// Place an assembled car on the track at `pos` with orientation `ori`.
///
/// The sub-meshes arrive in car space after the standard Z mirror (see
/// [`mts_to_submeshes_at`]). Positions are un-mirrored back to game axes,
/// rotated by `Rz * Rx * Ry` (Y first, `yaw_sign` flips the yaw), translated
/// by `pos`, lifted so the lowest vertex touches `pos[1]`, and mirrored again.
/// Normals are rotated but not translated. See the DS08 handoff for the rule.
pub fn place_car(submeshes: &mut [SubMesh], pos: [f32; 3], ori: [f32; 3], yaw_sign: f32) {
    let orientation = [ori[0], ori[1] * yaw_sign, ori[2]];
    let rotation = placement_rotation(orientation);

    let mut placed: Vec<Vec<[f32; 3]>> = Vec::with_capacity(submeshes.len());
    let mut lowest = f32::INFINITY;
    for sub in submeshes.iter() {
        let mut points = Vec::with_capacity(sub.positions.len());
        for position in &sub.positions {
            let game = [position[0], position[1], -position[2]];
            let rotated = rotate_point(&rotation, game);
            let moved = [
                rotated[0] + pos[0],
                rotated[1] + pos[1],
                rotated[2] + pos[2],
            ];
            lowest = lowest.min(moved[1]);
            points.push(moved);
        }
        placed.push(points);
    }

    let lift = if lowest.is_finite() {
        pos[1] - lowest
    } else {
        0.0
    };
    for (sub, points) in submeshes.iter_mut().zip(placed) {
        for (out, moved) in sub.positions.iter_mut().zip(points) {
            *out = [moved[0], moved[1] + lift, -moved[2]];
        }
        for normal in sub.normals.iter_mut() {
            let game = [normal[0], normal[1], -normal[2]];
            let rotated = rotate_point(&rotation, game);
            *normal = [rotated[0], rotated[1], -rotated[2]];
        }
    }
}

/// Shift an assembled car so its lowest vertex sits at `y = 0`, keeping X and Z.
///
/// The result is entity-local geometry for drive mode: applying a Bevy
/// transform at the mesh origin then places and yaws it without rebuilding.
pub fn car_local(submeshes: &mut [SubMesh]) {
    let mut lowest = f32::INFINITY;
    for sub in submeshes.iter() {
        for position in &sub.positions {
            lowest = lowest.min(position[1]);
        }
    }
    if !lowest.is_finite() {
        return;
    }
    for sub in submeshes.iter_mut() {
        for position in &mut sub.positions {
            position[1] -= lowest;
        }
    }
}

/// Viewer-space placement of a DS13 road-follow pose.
///
/// Game axes go in; the fields are the mesh translation and the three local
/// axes of a rotation matrix, already reflected by negating Z exactly once.
/// The viewer local `+Z` is the forward tangent, so these columns are applied
/// to viewer-mirrored local car geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FollowView {
    /// Viewer-space mesh origin.
    pub translation: [f32; 3],
    /// Image of the local `+X` (left) axis.
    pub x: [f32; 3],
    /// Image of the local `+Y` (up) axis.
    pub y: [f32; 3],
    /// Image of the local `+Z` (forward tangent) axis.
    pub z: [f32; 3],
}

/// Reflect one road-follow pose into viewer axes exactly once.
pub fn follow_view_transform(
    origin: [f64; 3],
    left: [f64; 3],
    up: [f64; 3],
    forward: [f64; 3],
) -> FollowView {
    let reflect = |v: [f64; 3]| [v[0] as f32, v[1] as f32, -(v[2] as f32)];
    FollowView {
        translation: reflect(origin),
        x: reflect(left),
        y: reflect(up),
        z: reflect(forward),
    }
}

/// Unit forward direction of a placed car (`-z` in game axes, mirrored).
pub fn car_forward(ori: [f32; 3], yaw_sign: f32) -> [f32; 3] {
    let orientation = [ori[0], ori[1] * yaw_sign, ori[2]];
    let rotation = placement_rotation(orientation);
    let forward = rotate_point(&rotation, [0.0, 0.0, -1.0]);
    [forward[0], forward[1], -forward[2]]
}
/// Row-major placement rotation `Rz * Rx * Ry` (Y applied first).
fn placement_rotation(ori: [f32; 3]) -> [[f32; 3]; 3] {
    let (sx, cx) = ori[0].sin_cos();
    let (sy, cy) = ori[1].sin_cos();
    let (sz, cz) = ori[2].sin_cos();
    [
        [cz * cy - sz * sx * sy, -sz * cx, cz * sy + sz * sx * cy],
        [sz * cy + cz * sx * sy, cz * cx, sz * sy - cz * sx * cy],
        [-cx * sy, sx, cx * cy],
    ]
}

/// Apply a row-major 3x3 rotation to a point.
fn rotate_point(rotation: &[[f32; 3]; 3], point: [f32; 3]) -> [f32; 3] {
    [
        rotation[0][0] * point[0] + rotation[0][1] * point[1] + rotation[0][2] * point[2],
        rotation[1][0] * point[0] + rotation[1][1] * point[1] + rotation[1][2] * point[2],
        rotation[2][0] * point[0] + rotation[2][1] * point[1] + rotation[2][2] * point[2],
    ]
}

/// Names of the MTS entries in an archive, sorted case-insensitively.
pub fn model_names(archive: &MasArchive) -> Vec<String> {
    let mut names: Vec<String> = archive
        .entries()
        .iter()
        .filter(|entry| entry.name.to_ascii_lowercase().ends_with(".mts"))
        .map(|entry| entry.name.clone())
        .collect();
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names
}

/// A lookup table from texture file name to the archive that holds it.
///
/// Built from an ordered list of MAS paths (put the model's own archive
/// first). Unreadable archives are skipped. On duplicate names the first
/// archive in the list wins.
#[derive(Debug, Default)]
pub struct TextureIndex {
    entries: HashMap<String, (PathBuf, String)>,
}

impl TextureIndex {
    /// Index every entry of every readable archive in `paths`.
    pub fn build(paths: &[PathBuf]) -> Self {
        let mut index = TextureIndex::default();
        for path in paths {
            let Ok(archive) = MasArchive::open(path) else {
                continue;
            };
            for entry in archive.entries() {
                let key = entry.name.to_ascii_lowercase();
                index
                    .entries
                    .entry(key)
                    .or_insert_with(|| (path.clone(), entry.name.clone()));
            }
        }
        index
    }

    /// Resolve a texture name to `(archive path, entry name)`.
    ///
    /// Tries the name as given, then with `.BMP`, then with `.TGA`. When the
    /// stage is animated (`frames > 1`) the first frame is tried first:
    /// `NAME00.BMP`, then `NAME00.TGA`. Matching is case-insensitive.
    pub fn find(&self, name: &str, frames: u32) -> Option<(PathBuf, String)> {
        let mut candidates = Vec::new();
        if frames > 1 {
            candidates.push(format!("{name}00.BMP"));
            candidates.push(format!("{name}00.TGA"));
        }
        candidates.push(name.to_string());
        candidates.push(format!("{name}.BMP"));
        candidates.push(format!("{name}.TGA"));
        for candidate in &candidates {
            if let Some(found) = self.entries.get(&candidate.to_ascii_lowercase()) {
                return Some(found.clone());
            }
        }
        None
    }

    /// Number of indexed texture names.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index holds no texture names.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A lookup table from an MTS mesh name to the archive that holds it.
///
/// Built from an ordered list of MAS paths (put the model's own archive
/// first). Unreadable archives are skipped. Exact names win; when
/// `NAME.MTS` is absent, the first entry ending in `-NAME.MTS` is accepted
/// (this install ships some files renamed, e.g. `412T1-GB28VA.MTS`).
#[derive(Debug, Default)]
pub struct MeshIndex {
    entries: Vec<(String, PathBuf, String)>,
    exact: HashMap<String, usize>,
}

impl MeshIndex {
    /// Index every `.mts` entry of every readable archive in `paths`.
    pub fn build(paths: &[PathBuf]) -> Self {
        let mut index = MeshIndex::default();
        for path in paths {
            let Ok(archive) = MasArchive::open(path) else {
                continue;
            };
            for entry in archive.entries() {
                if !entry.name.to_ascii_lowercase().ends_with(".mts") {
                    continue;
                }
                let key = entry.name.to_ascii_lowercase();
                let position = index.entries.len();
                index.exact.entry(key).or_insert(position);
                index.entries.push((
                    entry.name.to_ascii_lowercase(),
                    path.clone(),
                    entry.name.clone(),
                ));
            }
        }
        index
    }

    /// Resolve a mesh name to `(archive path, entry name)`.
    pub fn find(&self, name: &str) -> Option<(PathBuf, String)> {
        let lower = name.to_ascii_lowercase();
        if let Some(&position) = self.exact.get(&lower) {
            let (_, path, entry) = &self.entries[position];
            return Some((path.clone(), entry.clone()));
        }
        let suffix = format!("-{lower}");
        self.entries
            .iter()
            .find(|(key, _, _)| key.ends_with(&suffix))
            .map(|(_, path, entry)| (path.clone(), entry.clone()))
    }

    /// Number of indexed mesh names.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index holds no mesh names.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Decode a BMP or TGA image to `(width, height, RGBA bytes)`.
///
/// TGA has no signature magic, so `image::load_from_memory` cannot detect it;
/// when the automatic format guess fails the bytes are retried as TGA.
pub fn decode_rgba(bytes: &[u8], name: &str) -> Result<(u32, u32, Vec<u8>), String> {
    let decoded = image::load_from_memory(bytes)
        .or_else(|_| image::load_from_memory_with_format(bytes, image::ImageFormat::Tga))
        .map_err(|error| format!("{name}: {error}"))?;
    let mut rgba = decoded.to_rgba8();
    // Pure magenta (255, 0, 255) is the colour key for transparent pixels.
    for pixel in rgba.pixels_mut() {
        if pixel.0[..3] == [255, 0, 255] {
            // Black, so filtering does not bleed magenta into edges.
            pixel.0 = [0, 0, 0, 0];
        }
    }
    let (width, height) = rgba.dimensions();
    Ok((width, height, rgba.into_raw()))
}

/// Resolve the texture search path for a MAS archive.
///
/// When the MAS folder holds a `.veh` with a `Graphics=` value, the named
/// `.gen` in the nearest ancestor folder named `Vehicles` is read and its
/// `SearchPath=`/`MASFile=` entries are mapped to real files (case-insensitive
/// name match in the team folder or the vehicles root; missing files skipped).
/// The opened archive is always first. Any failure falls back to
/// [`folder_mas_paths`]: the opened archive followed by every other `*.mas` in
/// its folder. Never panics.
pub fn resolve_search_path(mas_path: &Path) -> Vec<PathBuf> {
    resolve_search_path_inner(mas_path).unwrap_or_else(|| folder_mas_paths(mas_path))
}

/// Search path for a team folder named by a `.veh` file (car mode).
///
/// Reads the `.veh` `Graphics=` file, resolves its `SearchPath=`/`MASFile=`
/// entries in the team folder and the vehicles root, and returns the real MAS
/// files in file order. Falls back to every `*.mas` in the `.veh` folder.
/// Never panics.
pub fn resolve_search_path_for_veh(veh: &Path) -> Vec<PathBuf> {
    resolve_search_path_for_veh_inner(veh)
        .unwrap_or_else(|| veh.parent().map(folder_mas_paths_in).unwrap_or_default())
}

/// Text of the `.gen` file named by a `.veh`'s `Graphics=` line, if findable.
pub fn veh_gen_text(veh: &Path) -> Option<String> {
    gen_text_for_veh(veh)
}

/// Resolve a `.SCN` scene's `MASFile=` names to real files.
///
/// The game root is the parent of the nearest ancestor folder named
/// `SeasonData` (case-insensitive). Each `SearchPath=` becomes that folder
/// under the root, resolved case-insensitively component by component. Every
/// `MASFile=` is then looked up in those folders in order; the first folder
/// that holds it (file name match, case-insensitive) wins. Missing names are
/// returned in the `missing` list, and a file found twice appears once.
pub fn resolve_scene_mas(scn_path: &Path, scene: &Scene) -> (Vec<PathBuf>, Vec<String>) {
    let dirs: Vec<PathBuf> = match game_root(scn_path) {
        Some(root) => scene
            .search_paths
            .iter()
            .map(|relative| resolve_relative_ci(&root, relative))
            .collect(),
        None => Vec::new(),
    };

    let mut found: Vec<PathBuf> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for name in &scene.mas_files {
        let mut hit = None;
        for dir in &dirs {
            if let Some(path) = find_file(dir, name) {
                hit = Some(path);
                break;
            }
        }
        match hit {
            Some(path) => {
                if !found.iter().any(|existing| same_path(existing, &path)) {
                    found.push(path);
                }
            }
            None => missing.push(name.clone()),
        }
    }
    (found, missing)
}

/// Parent of the nearest ancestor folder named `SeasonData` (case-insensitive).
fn game_root(scn_path: &Path) -> Option<PathBuf> {
    let dir = scn_path.parent()?;
    dir.ancestors()
        .find(|ancestor| {
            ancestor
                .file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("SeasonData"))
        })
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

/// Join a backslash- or slash-separated relative path to `root`, matching each
/// component against disk case-insensitively. Unresolved components are kept.
fn resolve_relative_ci(root: &Path, relative: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for part in relative
        .split(['\\', '/'])
        .filter(|part| !part.is_empty() && *part != ".")
    {
        path = find_dir_entry(&path, part).unwrap_or_else(|| path.join(part));
    }
    path
}

/// File or folder in `dir` whose name matches `name` case-insensitively.
fn find_dir_entry(dir: &Path, name: &str) -> Option<PathBuf> {
    let read = std::fs::read_dir(dir).ok()?;
    read.filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .is_some_and(|file| file.to_string_lossy().eq_ignore_ascii_case(name))
        })
}

fn resolve_search_path_inner(mas_path: &Path) -> Option<Vec<PathBuf>> {
    let mas_dir = mas_path.parent()?;
    let gen_text = gen_text_for_team(mas_dir)?;
    let rest = mas_paths_from_gen(mas_dir, &gen_text)?;

    let mut result = vec![mas_path.to_path_buf()];
    for path in rest {
        if !result.iter().any(|existing| same_path(existing, &path)) {
            result.push(path);
        }
    }
    Some(result)
}

fn resolve_search_path_for_veh_inner(veh: &Path) -> Option<Vec<PathBuf>> {
    let team_dir = veh.parent()?;
    let gen_text = gen_text_for_veh(veh)?;
    mas_paths_from_gen(team_dir, &gen_text)
}

/// Read the `.gen` named by the first `.veh` in `team_dir`.
fn gen_text_for_team(team_dir: &Path) -> Option<String> {
    let graphics = graphics_from_veh(team_dir)?;
    let vehicles_root = find_vehicles_root(team_dir)?;
    let gen_path = find_file(&vehicles_root, &graphics)?;
    read_latin1(&gen_path)
}

/// Read the `.gen` named by `veh`'s `Graphics=` line.
fn gen_text_for_veh(veh: &Path) -> Option<String> {
    let team_dir = veh.parent()?;
    let graphics = read_latin1(veh).and_then(|text| veh_graphics(&text))?;
    let vehicles_root = find_vehicles_root(team_dir)?;
    let gen_path = find_file(&vehicles_root, &graphics)?;
    read_latin1(&gen_path)
}

/// Map the `SearchPath=`/`MASFile=` entries of a `.gen` to real files.
///
/// The opened archive is not included: the caller controls ordering. Missing
/// files are skipped; duplicate files appear once. `None` when the `.gen` has
/// no usable entries.
fn mas_paths_from_gen(team_dir: &Path, gen_text: &str) -> Option<Vec<PathBuf>> {
    let vehicles_root = find_vehicles_root(team_dir)?;
    let entries = search_path(gen_text);
    if entries.is_empty() {
        return None;
    }

    let mut result: Vec<PathBuf> = Vec::new();
    for entry in &entries {
        let dir = match entry.dir {
            GenDir::Team => team_dir,
            GenDir::Vehicles => vehicles_root.as_path(),
        };
        let Some(found) = find_file(dir, &entry.mas) else {
            continue;
        };
        if result.iter().any(|existing| same_path(existing, &found)) {
            continue;
        }
        result.push(found);
    }
    Some(result)
}

/// Every `*.mas` in `dir`, sorted. Empty when `dir` cannot be read.
fn folder_mas_paths_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = read
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| has_extension(path, "mas"))
        .collect();
    paths.sort();
    paths
}

/// The opened archive first, then every other `*.mas` in the same folder,
/// sorted. The model's own archive wins on duplicate texture names.
pub fn folder_mas_paths(own: &Path) -> Vec<PathBuf> {
    let mut paths = vec![own.to_path_buf()];
    if let Some(dir) = own.parent() {
        paths.extend(
            folder_mas_paths_in(dir)
                .into_iter()
                .filter(|path| !same_path(path, own)),
        );
    }
    paths
}

/// First `Graphics=` value among the folder's `.veh` files, sorted by name.
fn graphics_from_veh(dir: &Path) -> Option<String> {
    let read = std::fs::read_dir(dir).ok()?;
    let mut veh_files: Vec<PathBuf> = read
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| has_extension(path, "veh"))
        .collect();
    veh_files.sort_by_key(|path| sort_key(path));
    veh_files
        .iter()
        .find_map(|path| read_latin1(path).and_then(|text| veh_graphics(&text)))
}

/// Nearest ancestor of `dir` whose folder name is `Vehicles`, case-insensitive.
fn find_vehicles_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|ancestor| {
            ancestor
                .file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Vehicles"))
        })
        .map(Path::to_path_buf)
}

/// Case-insensitive file-name match inside `dir`.
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let read = std::fs::read_dir(dir).ok()?;
    read.filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .is_some_and(|file| file.to_string_lossy().eq_ignore_ascii_case(name))
        })
}

/// Decode a file as Latin-1. `None` when it cannot be read.
fn read_latin1(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(bytes.into_iter().map(char::from).collect())
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
}

fn sort_key(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats_mts::{Group, Material, TextureStage, Vertex};

    fn vertex(position: [f32; 3], normal: [f32; 3], uv0: [f32; 2]) -> Vertex {
        Vertex {
            position,
            normal,
            color: 0,
            uv0,
            uv1: [0.0, 0.0],
        }
    }

    fn group(material_index: u32, vertices: Vec<Vertex>, triangles: Vec<[u16; 3]>) -> Group {
        Group {
            flags: 0,
            material_index,
            vertices,
            triangles,
        }
    }

    fn material(name: &str, texture: &str) -> Material {
        Material {
            name: name.to_string(),
            colors: [[0.0; 4]; 4],
            src_blend: 1,
            dst_blend: 0,
            specular_power: 0.0,
            stages: vec![TextureStage {
                texture: texture.to_string(),
                frames: 1,
            }],
        }
    }

    fn model(materials: Vec<Material>, groups: Vec<Group>) -> Mts {
        Mts {
            materials,
            groups,
            vertex_stride: 48,
            position: [0.0, 0.0, 0.0],
        }
    }

    fn triangle() -> Vec<Vertex> {
        vec![
            vertex([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0]),
            vertex([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0]),
            vertex([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0]),
        ]
    }

    #[test]
    fn splits_one_submesh_per_group() {
        let mts = model(
            vec![material("BODY", "HUB.BMP"), material("WHEEL", "WHEEL.BMP")],
            vec![
                group(0, triangle(), vec![[0, 1, 2]]),
                group(1, triangle(), vec![[0, 1, 2]]),
            ],
        );
        let submeshes = mts_to_submeshes(&mts);
        assert_eq!(submeshes.len(), 2);
        assert_eq!(submeshes[0].material_index, 0);
        assert_eq!(submeshes[1].material_index, 1);
        assert_eq!(submeshes[0].positions.len(), 3);
        assert_eq!(submeshes[1].indices, vec![0, 1, 2]);
    }

    #[test]
    fn keeps_uvs_and_negates_z() {
        let mut vertices = triangle();
        vertices[0].uv0 = [0.25, 0.75];
        vertices[0].position = [1.0, 2.0, 3.0];
        vertices[0].normal = [0.0, 0.0, 1.0];
        let mts = model(
            vec![material("BODY", "HUB.BMP")],
            vec![group(0, vertices, vec![])],
        );
        let submeshes = mts_to_submeshes(&mts);
        assert_eq!(submeshes[0].uvs[0], [0.25, 0.75]);
        assert_eq!(submeshes[0].positions[0], [1.0, 2.0, -3.0]);
        assert_eq!(submeshes[0].normals[0], [0.0, 0.0, -1.0]);
    }

    #[test]
    fn bounds_span_all_submeshes() {
        let mts = model(
            vec![material("A", "A.BMP"), material("B", "B.BMP")],
            vec![
                group(0, vec![vertex([0.0, 0.0, 0.0], [0.0; 3], [0.0; 2])], vec![]),
                group(
                    1,
                    vec![vertex([3.0, -1.0, 0.0], [0.0; 3], [0.0; 2])],
                    vec![],
                ),
            ],
        );
        let bounds = bounds(&mts_to_submeshes(&mts)).unwrap();
        assert_eq!(bounds.min, [0.0, -1.0, 0.0]);
        assert_eq!(bounds.max, [3.0, 0.0, 0.0]);
    }

    #[test]
    fn bounds_of_empty_is_none() {
        assert!(bounds(&[]).is_none());
    }

    #[test]
    fn sky_instances_are_ring_and_its_clouds_child() {
        let instance = |name: &str, parent: Option<usize>| formats_scn::SceneInstance {
            name: name.to_string(),
            parent,
            meshes: vec![format!("{name}.MTS")],
            mesh_refs: Vec::new(),
            render: true,
            moveable: true,
            animated: false,
        };
        let scene = Scene {
            instances: vec![
                instance("skyboxi", None),
                instance("clouds", Some(0)),
                instance("cloudsny", Some(0)),
                instance("Helicopter", None),
            ],
            ..Scene::default()
        };
        assert_eq!(sky_instance_indices(&scene), vec![0, 1]);
    }

    #[test]
    fn sky_instances_empty_without_ring() {
        let scene = Scene {
            instances: vec![formats_scn::SceneInstance {
                name: "clouds".to_string(),
                parent: None,
                meshes: Vec::new(),
                mesh_refs: Vec::new(),
                render: true,
                moveable: true,
                animated: false,
            }],
            ..Scene::default()
        };
        assert!(sky_instance_indices(&scene).is_empty());
    }

    #[test]
    fn placement_offset_is_added_before_z_mirror() {
        let mts = model(
            vec![material("WHEEL", "WHEEL.BMP")],
            vec![group(0, triangle(), vec![[0, 1, 2]])],
        );
        let submeshes = mts_to_submeshes_at(&mts, [10.0, 20.0, 30.0]);
        assert_eq!(submeshes[0].positions[0], [10.0, 20.0, -30.0]);
        assert_eq!(submeshes[0].positions[1], [11.0, 20.0, -30.0]);
        assert_eq!(submeshes[0].normals[0], [0.0, 0.0, -1.0]);
    }

    // --- place_car / car_forward ---

    fn car_submesh(positions: Vec<[f32; 3]>) -> SubMesh {
        SubMesh {
            material_index: 0,
            normals: positions.iter().map(|_| [0.0, 1.0, 0.0]).collect(),
            positions,
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }

    #[test]
    fn place_car_identity_keeps_viewer_position() {
        let mut meshes = vec![car_submesh(vec![[1.0, 2.0, -3.0]])];
        place_car(&mut meshes, [10.0, 5.0, 20.0], [0.0, 0.0, 0.0], 1.0);
        assert_eq!(meshes[0].positions[0], [11.0, 5.0, -23.0]);
    }

    #[test]
    fn place_car_rotates_yaw_around_y() {
        let mut meshes = vec![car_submesh(vec![[0.0, 0.0, -1.0]])];
        place_car(
            &mut meshes,
            [0.0, 0.0, 0.0],
            [0.0, std::f32::consts::FRAC_PI_2, 0.0],
            1.0,
        );
        let p = meshes[0].positions[0];
        assert!((p[0] - 1.0).abs() < 1e-5, "{p:?}");
        assert!(p[1].abs() < 1e-5, "{p:?}");
        assert!(p[2].abs() < 1e-5, "{p:?}");
    }

    #[test]
    fn place_car_lifts_lowest_vertex_to_pos_y() {
        let mut meshes = vec![car_submesh(vec![[0.0, -2.0, 0.0], [0.0, 1.0, 0.0]])];
        place_car(&mut meshes, [0.0, 5.0, 0.0], [0.0, 0.0, 0.0], 1.0);
        assert!((meshes[0].positions[0][1] - 5.0).abs() < 1e-5);
        assert!((meshes[0].positions[1][1] - 8.0).abs() < 1e-5);
    }

    #[test]
    fn car_forward_default_points_plus_z() {
        assert_eq!(car_forward([0.0, 0.0, 0.0], 1.0), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn car_local_lifts_lowest_vertex_to_zero_and_keeps_xz() {
        let mut meshes = vec![car_submesh(vec![[1.0, -2.0, 3.0], [-1.0, 4.0, -3.0]])];
        car_local(&mut meshes);
        assert_eq!(meshes[0].positions[0], [1.0, 0.0, 3.0]);
        assert_eq!(meshes[0].positions[1], [-1.0, 6.0, -3.0]);
    }

    #[test]
    fn car_local_empty_is_noop() {
        let mut meshes: Vec<SubMesh> = Vec::new();
        car_local(&mut meshes);
    }

    #[test]
    fn car_forward_yaw_sign_flips_the_turn() {
        let ori = [0.0, std::f32::consts::FRAC_PI_2, 0.0];
        let plus = car_forward(ori, 1.0);
        let minus = car_forward(ori, -1.0);
        assert!((plus[0] - -1.0).abs() < 1e-5, "{plus:?}");
        assert!((minus[0] - 1.0).abs() < 1e-5, "{minus:?}");
    }

    // --- follow_view_transform ---

    #[test]
    fn follow_view_reflects_the_slope_pose_once_and_keeps_the_anchor() {
        let origin = [1.0, 1.5, -0.5];
        let left = [0.894427190999916, 0.447213595499958, 0.0];
        let up = [-0.447213595499958, 0.894427190999916, 0.0];
        let forward = [0.0, 0.0, -1.0];
        let view = follow_view_transform(origin, left, up, forward);
        assert_eq!(view.translation, [1.0, 1.5, 0.5]);
        assert_eq!(view.x, [0.8944272, 0.4472136, 0.0]);
        assert_eq!(view.y, [-0.4472136, 0.8944272, 0.0]);
        assert_eq!(view.z, [0.0, 0.0, 1.0]);

        // Viewer-local rear anchor for axle (0, 1.5) is (0, 0, -1.5); the
        // transform must land on the reflected contact (1, 1.5, -1).
        let anchor = [0.0f32, 0.0, -1.5];
        let world = [
            view.translation[0]
                + view.x[0] * anchor[0]
                + view.y[0] * anchor[1]
                + view.z[0] * anchor[2],
            view.translation[1]
                + view.x[1] * anchor[0]
                + view.y[1] * anchor[1]
                + view.z[1] * anchor[2],
            view.translation[2]
                + view.x[2] * anchor[0]
                + view.y[2] * anchor[1]
                + view.z[2] * anchor[2],
        ];
        for (got, want) in world.iter().zip([1.0, 1.5, -1.0]) {
            assert!((got - want).abs() < 1e-6, "anchor world {world:?}");
        }
        // Reflecting an orthonormal game basis keeps the axes orthonormal.
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        assert!((dot(view.x, view.y)).abs() < 1e-6);
        assert!((dot(view.x, view.z)).abs() < 1e-6);
        assert!((dot(view.y, view.z)).abs() < 1e-6);
        assert!((dot(view.x, view.x) - 1.0).abs() < 1e-6);
    }

    // --- TextureIndex ---

    const MAS_MAGIC: &[u8] = b"CUBEMAS4.10\0\0\0\0\0";

    /// Build a minimal stored (uncompressed) MAS archive from name/payload pairs.
    fn build_mas(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let count = entries.len() as u32;
        let mut directory = Vec::new();
        let mut payload = Vec::new();
        for (name, data) in entries {
            let offset = payload.len() as u32;
            let size = data.len() as u32;
            directory.extend_from_slice(&0x12u32.to_le_bytes());
            directory.extend_from_slice(&offset.to_le_bytes());
            directory.extend_from_slice(&size.to_le_bytes());
            directory.extend_from_slice(&size.to_le_bytes());
            directory.extend_from_slice(&0u32.to_le_bytes());
            let mut raw = [0u8; 236];
            raw[..name.len()].copy_from_slice(name.as_bytes());
            directory.extend_from_slice(&raw);
            payload.extend_from_slice(data);
        }

        let mut out = Vec::new();
        out.extend_from_slice(MAS_MAGIC);
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&directory);
        out.extend_from_slice(&payload);
        out
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("f1c_viewer_{tag}_{}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }

        fn write_mas(&self, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, build_mas(entries)).unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lookup_tries_given_then_bmp_then_tga_case_insensitively() {
        let dir = TempDir::new("lookup");
        let path = dir.write_mas(
            "a.mas",
            &[("GREYRIM", b"x"), ("HUB.BMP", b"x"), ("WHEEL.TGA", b"x")],
        );
        let index = TextureIndex::build(&[path]);
        assert_eq!(index.find("GREYRIM", 1).unwrap().1, "GREYRIM");
        assert_eq!(index.find("greyrim", 1).unwrap().1, "GREYRIM");
        assert_eq!(index.find("HUB", 1).unwrap().1, "HUB.BMP");
        assert_eq!(index.find("hub.bmp", 1).unwrap().1, "HUB.BMP");
        assert_eq!(index.find("WHEEL", 1).unwrap().1, "WHEEL.TGA");
        assert!(index.find("missing", 1).is_none());
    }

    #[test]
    fn first_archive_wins_and_unreadable_is_skipped() {
        let dir = TempDir::new("first");
        let first = dir.write_mas("first.mas", &[("SHARED.BMP", b"first")]);
        let second = dir.write_mas("second.mas", &[("SHARED.BMP", b"second")]);
        let missing = dir.0.join("does_not_exist.mas");
        let index = TextureIndex::build(&[first.clone(), second, missing]);
        let (path, name) = index.find("shared.bmp", 1).unwrap();
        assert_eq!(path, first);
        assert_eq!(name, "SHARED.BMP");
    }

    #[test]
    fn animated_lookup_tries_first_frame_before_base_name() {
        let dir = TempDir::new("animated");
        let path = dir.write_mas("a.mas", &[("BBSGR00.BMP", b"x"), ("BBSGR.BMP", b"x")]);
        let index = TextureIndex::build(&[path]);
        assert_eq!(index.find("BBSGR", 4).unwrap().1, "BBSGR00.BMP");
        assert_eq!(index.find("BBSGR", 1).unwrap().1, "BBSGR.BMP");
    }

    // --- MeshIndex ---

    #[test]
    fn mesh_index_prefers_exact_name_then_dash_suffix() {
        let dir = TempDir::new("meshindex");
        let path = dir.write_mas(
            "a.mas",
            &[("GB28VA.MTS", b"x"), ("412T1-GB28FWA.MTS", b"x")],
        );
        let index = MeshIndex::build(&[path]);
        assert_eq!(index.find("GB28VA.MTS").unwrap().1, "GB28VA.MTS");
        assert_eq!(index.find("gb28va.mts").unwrap().1, "GB28VA.MTS");
        assert_eq!(index.find("GB28FWA.MTS").unwrap().1, "412T1-GB28FWA.MTS");
        assert!(index.find("NOPE.MTS").is_none());
    }

    #[test]
    fn mesh_index_ignores_non_mts_entries() {
        let dir = TempDir::new("meshonly");
        let path = dir.write_mas("a.mas", &[("HUB.BMP", b"x"), ("CAR.MTS", b"x")]);
        let index = MeshIndex::build(&[path]);
        assert_eq!(index.len(), 1);
        assert!(index.find("HUB.BMP").is_none());
    }

    // --- resolve_search_path ---

    /// Write a file, creating its parent directories.
    fn write_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn resolve_search_path_reads_veh_and_gen() {
        let dir = TempDir::new("genpath");
        let vehicles = dir.0.join("Vehicles");
        let team = vehicles.join("Ferrari").join("1994_412T1");
        let own = team.join("Team.mas");
        write_file(&team.join("car.veh"), b"Graphics=1994_Generic_F1.gen\n");
        write_file(
            &vehicles.join("1994_Generic_F1.gen"),
            b"SearchPath=<TEAMDIR>\n\
              MASFile=Team.mas\n\
              SearchPath=<VEHDIR>\n\
              MASFile=1994.mas\n\
              MASFile=missing.mas\n\
              MASFile=Cdb.mas\n",
        );
        write_file(&own, b"");
        write_file(&team.join("Team.mas"), b"");
        write_file(&vehicles.join("1994.mas"), b"");
        write_file(&vehicles.join("Cdb.mas"), b"");

        let resolved = resolve_search_path(&own);
        assert_eq!(
            resolved,
            vec![
                own.clone(),
                vehicles.join("1994.mas"),
                vehicles.join("Cdb.mas"),
            ]
        );
    }

    #[test]
    fn resolve_search_path_matches_file_names_case_insensitively() {
        let dir = TempDir::new("gencase");
        let vehicles = dir.0.join("Vehicles");
        let team = vehicles.join("Team");
        let own = team.join("TEAM.MAS");
        write_file(&team.join("car.veh"), b"Graphics=my.gen\n");
        write_file(
            &vehicles.join("My.GEN"),
            b"SearchPath=<VEHDIR>\nMASFile=SEASON.MAS\n",
        );
        write_file(&own, b"");
        write_file(&vehicles.join("season.mas"), b"");

        let resolved = resolve_search_path(&own);
        assert_eq!(resolved, vec![own, vehicles.join("season.mas")]);
    }

    #[test]
    fn resolve_search_path_falls_back_without_veh() {
        let dir = TempDir::new("genfallback");
        let own = dir.0.join("own.mas");
        let other = dir.0.join("other.mas");
        write_file(&own, b"");
        write_file(&other, b"");

        let resolved = resolve_search_path(&own);
        assert_eq!(resolved, vec![own, other]);
    }

    #[test]
    fn resolve_search_path_falls_back_without_vehicles_root() {
        let dir = TempDir::new("gennoroot");
        let own = dir.0.join("own.mas");
        let other = dir.0.join("other.mas");
        write_file(&dir.0.join("car.veh"), b"Graphics=some.gen\n");
        write_file(&own, b"");
        write_file(&other, b"");

        let resolved = resolve_search_path(&own);
        assert_eq!(resolved, vec![own, other]);
    }

    #[test]
    fn resolve_search_path_for_veh_reads_veh_graphics() {
        let dir = TempDir::new("vehpath");
        let vehicles = dir.0.join("Vehicles");
        let team = vehicles.join("Ferrari").join("1994_412T1");
        let veh = team.join("car.veh");
        write_file(&veh, b"Graphics=1994_Generic_F1.gen\nGenString=GB28FERBV\n");
        write_file(
            &vehicles.join("1994_Generic_F1.gen"),
            b"SearchPath=<TEAMDIR>\n\
              MASFile=Team.mas\n\
              SearchPath=<VEHDIR>\n\
              MASFile=1994.mas\n\
              MASFile=missing.mas\n",
        );
        write_file(&team.join("Team.mas"), b"");
        write_file(&vehicles.join("1994.mas"), b"");

        let resolved = resolve_search_path_for_veh(&veh);
        assert_eq!(
            resolved,
            vec![team.join("Team.mas"), vehicles.join("1994.mas")]
        );
        assert!(veh_gen_text(&veh).is_some());
    }

    #[test]
    fn resolve_search_path_for_veh_falls_back_to_folder() {
        let dir = TempDir::new("vehfallback");
        let veh = dir.0.join("car.veh");
        let other = dir.0.join("Other.mas");
        write_file(&veh, b"GenString=GB28FERBV\n");
        write_file(&other, b"");

        assert_eq!(resolve_search_path_for_veh(&veh), vec![other]);
    }

    // --- resolve_scene_mas ---

    fn scene(search_paths: &[&str], mas_files: &[&str]) -> Scene {
        Scene {
            search_paths: search_paths.iter().map(|s| s.to_string()).collect(),
            mas_files: mas_files.iter().map(|s| s.to_string()).collect(),
            ..Scene::default()
        }
    }

    #[test]
    fn scene_mas_resolves_relative_to_game_root_first_folder_wins() {
        let dir = TempDir::new("scenemas");
        let root = dir.0.join("game");
        let scn = root.join("SeasonData/Circuits/Australia/Adelaide/track.SCN");
        write_file(
            &root.join("SeasonData/Circuits/Australia/Adelaide/own.mas"),
            b"",
        );
        write_file(&root.join("SeasonData/Circuits/Common/Shared.mas"), b"");
        write_file(&root.join("SeasonData/Vehicles/Shared.mas"), b"");
        write_file(&root.join("SeasonData/Vehicles/CDB.MAS"), b"");
        write_file(&scn, b"CUBEASF\n");

        let scene = scene(
            &[
                "SeasonData\\Circuits\\Australia\\Adelaide",
                "SeasonData\\Circuits\\Common",
                "SeasonData\\Vehicles",
            ],
            &["own.mas", "Shared.mas", "cdb.mas", "Missing.mas"],
        );
        let (found, missing) = resolve_scene_mas(&scn, &scene);
        assert_eq!(
            found,
            vec![
                root.join("SeasonData/Circuits/Australia/Adelaide/own.mas"),
                root.join("SeasonData/Circuits/Common/Shared.mas"),
                root.join("SeasonData/Vehicles/CDB.MAS"),
            ]
        );
        assert_eq!(missing, vec!["Missing.mas".to_string()]);
    }

    #[test]
    fn scene_mas_matches_path_and_file_case_insensitively() {
        let dir = TempDir::new("scenecase");
        let root = dir.0.join("game");
        let scn = root.join("SeasonData/Circuits/Adelaide/track.SCN");
        write_file(&root.join("SeasonData/Circuits/Common/Map.BMP"), b"");
        write_file(&root.join("SeasonData/Circuits/Common/Cmaps.mas"), b"");
        write_file(&scn, b"CUBEASF\n");

        let scene = scene(
            &["seasondata\\circuits\\common"],
            &["CMAPS.MAS", "none.mas"],
        );
        let (found, missing) = resolve_scene_mas(&scn, &scene);
        assert_eq!(
            found,
            vec![root.join("SeasonData/Circuits/Common/Cmaps.mas")]
        );
        assert_eq!(missing, vec!["none.mas".to_string()]);
    }

    #[test]
    fn scene_mas_without_season_data_ancestor_reports_all_missing() {
        let dir = TempDir::new("scenonroot");
        let scn = dir.0.join("nowhere/track.SCN");
        write_file(&scn, b"CUBEASF\n");
        let scene = scene(&["SeasonData"], &["a.mas"]);
        let (found, missing) = resolve_scene_mas(&scn, &scene);
        assert!(found.is_empty());
        assert_eq!(missing, vec!["a.mas".to_string()]);
    }

    // --- decode_rgba ---

    fn encode(format: image::ImageFormat) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([200, 100, 50, 255]));
        let mut buffer = std::io::Cursor::new(Vec::new());
        image.write_to(&mut buffer, format).unwrap();
        buffer.into_inner()
    }

    #[test]
    fn decodes_bmp() {
        let (width, height, rgba) =
            decode_rgba(&encode(image::ImageFormat::Bmp), "test.bmp").unwrap();
        assert_eq!((width, height), (2, 1));
        assert_eq!(rgba, vec![200, 100, 50, 255, 200, 100, 50, 255]);
    }

    #[test]
    fn magenta_pixels_become_transparent() {
        let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 255, 255]));
        let mut buffer = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut buffer, image::ImageFormat::Bmp)
            .unwrap();
        let (_, _, rgba) = decode_rgba(&buffer.into_inner(), "KEY.BMP").unwrap();
        assert_eq!(rgba, vec![0, 0, 0, 0]);
    }

    #[test]
    fn decodes_jpeg_named_bmp() {
        // Some track archives store JPEG data under a `.BMP` name.
        let image = image::RgbImage::from_pixel(8, 8, image::Rgb([200, 100, 50]));
        let mut buffer = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut buffer, image::ImageFormat::Jpeg)
            .unwrap();
        let (width, height, _) = decode_rgba(&buffer.into_inner(), "BRICKB.BMP").unwrap();
        assert_eq!((width, height), (8, 8));
    }

    #[test]
    fn decodes_tga() {
        let (width, height, rgba) =
            decode_rgba(&encode(image::ImageFormat::Tga), "test.tga").unwrap();
        assert_eq!((width, height), (2, 1));
        assert_eq!(rgba.len(), 8);
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode_rgba(b"not an image", "bad.bmp").is_err());
    }

    // --- ground mesh selection ---

    #[test]
    fn explicit_hat_target_top_level_is_selected_even_when_hidden() {
        // Render is not part of the rule; a hidden mesh is still selected.
        assert_eq!(
            classify_mesh(true, false, false, false, false),
            MeshSelection::Selected
        );
    }

    #[test]
    fn non_selected_meshes_are_excluded() {
        // Absent/false/invalid HATTarget all arrive as hat_true = false.
        assert_eq!(
            classify_mesh(false, false, false, false, false),
            MeshSelection::Excluded
        );
    }

    #[test]
    fn selected_but_unsupported_context_is_reported() {
        for (nested, moveable, animated, sky) in [
            (true, false, false, false),
            (false, true, false, false),
            (false, false, true, false),
            (false, false, false, true),
        ] {
            assert_eq!(
                classify_mesh(true, nested, moveable, animated, sky),
                MeshSelection::Unsupported
            );
        }
    }

    // --- barrier mesh selection ---

    #[test]
    fn visible_explicit_coll_target_is_selected() {
        assert_eq!(
            classify_barrier(true, true, false, false, false, false),
            BarrierSelection::Selected
        );
    }

    #[test]
    fn hat_only_or_hidden_barriers_are_excluded() {
        // CollTarget=True is required: a HAT-only mesh passes coll_true=false.
        assert_eq!(
            classify_barrier(false, true, false, false, false, false),
            BarrierSelection::Excluded
        );
        // Render=False is excluded even with CollTarget=True (hidden timing).
        assert_eq!(
            classify_barrier(true, false, false, false, false, false),
            BarrierSelection::Excluded
        );
    }

    #[test]
    fn selected_barriers_in_unsupported_contexts_are_reported() {
        for (nested, moveable, animated, sky) in [
            (true, false, false, false),
            (false, true, false, false),
            (false, false, true, false),
            (false, false, false, true),
        ] {
            assert_eq!(
                classify_barrier(true, true, nested, moveable, animated, sky),
                BarrierSelection::Unsupported
            );
        }
    }

    #[test]
    fn diagnostic_sweep_crosses_the_face_interior() {
        let wall = BarrierSet::build(vec![MeshGeometry {
            occurrence: 0,
            name: "WALL".to_string(),
            groups: vec![GroupGeometry {
                group: 0,
                positions: vec![[5.0, -5.0, -5.0], [5.0, 5.0, -5.0], [5.0, 0.0, 5.0]],
                indices: vec![0, 1, 2],
            }],
        }])
        .unwrap();
        let diagnostic = diagnostic_barrier_sweep(&wall).expect("diagnostic sweep");
        assert_eq!(diagnostic.name, "WALL");
        assert_eq!(diagnostic.source.occurrence, 0);
        assert!((diagnostic.t - 0.625).abs() < 1e-8, "t {}", diagnostic.t);
        assert!(diagnostic.start[0] > 5.0 && diagnostic.end[0] == 5.0);
    }
}
