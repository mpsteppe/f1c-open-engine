//! Headless barrier geometry and a swept-sphere query.
//!
//! This is the prototype prototype from `specs/BARRIER_STOP.md`. It is **not** a
//! reconstruction of the original collision model: the selection rule (visible
//! explicit `CollTarget=True`, steep faces), the proxy sphere and the swept
//! query are project choices.
//!
//! Geometry is immutable `f64` in **game world coordinates**, in metres, with
//! stable [`SourceId`] identities, mirroring the prototype `ground_query` set. Only
//! non-degenerate 3D triangles with an absolute unit geometric normal Y below
//! `0.5` are retained, so near-vertical walls and barriers survive while flat
//! and shallow surfaces (track, kerbs, run-off) are excluded and counted.
//!
//! The query is a conservative sweep of a sphere along a straight centre
//! segment. Conservative advancement guarantees no tunnelling: at each step the
//! distance from the moving centre to the closest point on the triangle bounds
//! how far the centre can move before touching. If the iteration budget is
//! exhausted the query returns a named failure rather than claiming a clear
//! path.

use std::cmp::Ordering;

use crate::{GroundError, GroupGeometry, MeshGeometry, SourceId};

/// Retain only faces whose `|normal.y|` is below this (steeper than 60 degrees).
const MAX_NORMAL_Y: f64 = 0.5;
/// A 3D triangle whose cross-product length is at or below this is degenerate.
const DEGENERATE_AREA: f64 = 1e-10;
/// Upper bound on spatial-index cells, so allocation stays bounded.
const MAX_GRID_CELLS: usize = 1 << 20;
/// A triangle covering more cells than this is kept in the fallback list.
const MAX_CELLS_PER_TRIANGLE: usize = 128;
/// Above this many cells a sweep falls back to a full scan instead.
const MAX_CELLS_PER_QUERY: usize = 4096;
/// Distance tolerance for declaring contact, in metres.
const DIST_TOL: f64 = 1e-8;
/// Times within this fraction count as an exact tie, in sweep fractions.
const TIE_TOL: f64 = 1e-9;
/// Conservative-advancement iteration budget.
const MAX_ITER: u32 = 128;

/// Counts of retained and rejected barrier triangles after the build.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BarrierStats {
    /// Triangles kept in the barrier set.
    pub retained: usize,
    /// Triangles dropped for a near-zero 3D area.
    pub degenerate: usize,
    /// Triangles dropped for being no steeper than 60 degrees.
    pub horizontal: usize,
}

impl BarrierStats {
    /// Total dropped triangles.
    pub fn rejected(&self) -> usize {
        self.degenerate + self.horizontal
    }
}

/// One retained barrier triangle with its geometric normal and identity.
#[derive(Debug, Clone)]
struct BarrierTriangle {
    v: [[f64; 3]; 3],
    normal: [f64; 3],
    source: SourceId,
    name_index: u32,
}

/// One retained barrier triangle, for diagnostics and constructed sweeps.
#[derive(Debug, Clone, PartialEq)]
pub struct BarrierFace {
    /// Triangle vertices in game world coordinates.
    pub vertices: [[f64; 3]; 3],
    /// Unit geometric normal (not forced upward; either winding).
    pub normal: [f64; 3],
    /// Stable identity of the source triangle.
    pub source: SourceId,
    /// Display name of the source mesh.
    pub name: String,
}

/// One swept-sphere contact.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepHit {
    /// Earliest contact fraction along the sweep, in `[0, 1]`.
    pub t: f64,
    /// Contact point on the triangle surface.
    pub point: [f64; 3],
    /// Unit contact normal, pointing from the surface to the sphere centre.
    pub normal: [f64; 3],
    /// Stable identity of the triangle that was hit.
    pub source: SourceId,
    /// Display name of the source mesh.
    pub name: String,
    /// Stable retained-triangle index.
    pub triangle: u32,
}

/// A sweep result: an optional contact plus how many triangles were tested.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepResult {
    /// The earliest contact, or `None` when the sweep is clear.
    pub hit: Option<SweepHit>,
    /// Number of candidate triangles tested for this sweep.
    pub candidates: usize,
}

/// A built barrier set: retained steep triangles plus a local spatial index.
#[derive(Debug, Clone)]
pub struct BarrierSet {
    triangles: Vec<BarrierTriangle>,
    names: Vec<String>,
    grid: Grid,
    oversized: Vec<u32>,
    stats: BarrierStats,
}

/// The outcome of trying to retain one face.
enum Face {
    Keep(BarrierTriangle),
    Degenerate,
    Horizontal,
}

/// The outcome of sweeping one triangle.
enum Sweep {
    Hit(f64),
    Miss,
    Uncertain,
}

impl BarrierSet {
    /// Build the barrier set from selected mesh geometry.
    ///
    /// Rejects non-finite vertices, out-of-range indices and index lists that
    /// are not a whole number of triangles with a named error; drops degenerate
    /// and non-steep faces (counted); fails on zero retained triangles.
    pub fn build(meshes: Vec<MeshGeometry>) -> Result<Self, GroundError> {
        let mut names: Vec<String> = Vec::new();
        let mut triangles: Vec<BarrierTriangle> = Vec::new();
        let mut stats = BarrierStats::default();

        for mesh in meshes {
            let name_index = names.len() as u32;
            names.push(mesh.name.clone());
            for group in mesh.groups {
                check_group(&mesh.name, &group)?;
                for (triangle_index, chunk) in group.indices.as_chunks::<3>().0.iter().enumerate() {
                    let indices = *chunk;
                    let a = group.positions[indices[0] as usize];
                    let b = group.positions[indices[1] as usize];
                    let c = group.positions[indices[2] as usize];
                    let identity = source(mesh.occurrence, group.group, triangle_index);
                    match make_triangle(a, b, c, name_index, identity) {
                        Face::Keep(triangle) => {
                            stats.retained += 1;
                            triangles.push(triangle);
                        }
                        Face::Degenerate => stats.degenerate += 1,
                        Face::Horizontal => stats.horizontal += 1,
                    }
                }
            }
        }

        if triangles.is_empty() {
            return Err(GroundError::NoTriangles);
        }

        let (grid, oversized) = build_index(&triangles);
        Ok(BarrierSet {
            triangles,
            names,
            grid,
            oversized,
            stats,
        })
    }

    /// Total retained barrier triangles in the set.
    pub fn total_triangles(&self) -> usize {
        self.triangles.len()
    }

    /// Build statistics (retained, degenerate, horizontal).
    pub fn stats(&self) -> BarrierStats {
        self.stats
    }

    /// Find the earliest swept-sphere contact along the straight centre segment.
    ///
    /// The sphere travels from `start` to `end` with the given `radius`. Returns
    /// the earliest contact fraction `t` in `[0, 1]`, the contact point/normal,
    /// the stable source identity and the tested candidate count; `None` means
    /// the whole sweep is clear. Invalid inputs return a named error, and a
    /// sweep that cannot be resolved within its budget returns
    /// [`GroundError::SweepNotConverged`] instead of claiming a clear path.
    pub fn sweep_sphere(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        radius: f64,
    ) -> Result<SweepResult, GroundError> {
        if !start.iter().all(|value| value.is_finite()) {
            return Err(GroundError::InvalidInput {
                what: "sweep start",
            });
        }
        if !end.iter().all(|value| value.is_finite()) {
            return Err(GroundError::InvalidInput { what: "sweep end" });
        }
        if !radius.is_finite() || radius <= 0.0 {
            return Err(GroundError::InvalidInput {
                what: "sweep radius",
            });
        }

        let delta = sub(end, start);
        let speed = length(delta);
        // The index is in the XZ plane; the box covers the whole swept sphere.
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for point in [start, end] {
            min[0] = min[0].min(point[0]);
            max[0] = max[0].max(point[0]);
            min[1] = min[1].min(point[2]);
            max[1] = max[1].max(point[2]);
        }
        min[0] -= radius;
        min[1] -= radius;
        max[0] += radius;
        max[1] += radius;

        let mut candidates: Vec<u32> = Vec::new();
        if let Some(range) = self.grid.cell_range(min, max) {
            for cx in range.cx0..=range.cx1 {
                for cz in range.cz0..=range.cz1 {
                    candidates.extend_from_slice(&self.grid.cells[cx * self.grid.nz + cz]);
                }
            }
        }
        candidates.extend_from_slice(&self.oversized);
        candidates.sort_unstable();
        candidates.dedup();

        let mut best: Option<(f64, u32)> = None;
        let mut uncertain: Option<usize> = None;
        for &index in &candidates {
            let triangle = &self.triangles[index as usize];
            match sweep_triangle(triangle, start, delta, speed, radius) {
                Sweep::Hit(t) => {
                    let replace = match best {
                        None => true,
                        Some((current, current_index)) => {
                            if t < current - TIE_TOL {
                                true
                            } else if t > current + TIE_TOL {
                                false
                            } else {
                                source_less(
                                    triangle.source,
                                    self.triangles[current_index as usize].source,
                                )
                            }
                        }
                    };
                    if replace {
                        best = Some((t, index));
                    }
                }
                Sweep::Miss => {}
                Sweep::Uncertain => uncertain = Some(index as usize),
            }
        }

        // A converged hit is not proven earliest if another candidate could
        // not be resolved. Only a contact at the start is globally earliest.
        if best.is_none_or(|(t, _)| t > 0.0) {
            if let Some(index) = uncertain {
                let name = self.names[self.triangles[index].name_index as usize].clone();
                return Err(GroundError::SweepNotConverged { name });
            }
        }
        if let Some((t, index)) = best {
            let triangle = &self.triangles[index as usize];
            if let Some((point, normal)) = contact(triangle, start, delta, t) {
                return Ok(SweepResult {
                    hit: Some(SweepHit {
                        t,
                        point,
                        normal,
                        source: triangle.source,
                        name: self.names[triangle.name_index as usize].clone(),
                        triangle: index,
                    }),
                    candidates: candidates.len(),
                });
            }
            uncertain = Some(index as usize);
        }

        if let Some(index) = uncertain {
            let name = self.names[self.triangles[index].name_index as usize].clone();
            return Err(GroundError::SweepNotConverged { name });
        }

        Ok(SweepResult {
            hit: None,
            candidates: candidates.len(),
        })
    }

    /// One retained barrier triangle, for diagnostics and constructed sweeps.
    pub fn face(&self, index: usize) -> Option<BarrierFace> {
        let triangle = self.triangles.get(index)?;
        Some(BarrierFace {
            vertices: triangle.v,
            normal: triangle.normal,
            source: triangle.source,
            name: self.names[triangle.name_index as usize].clone(),
        })
    }
}

/// Validate one group and its triangle-list length.
fn check_group(name: &str, group: &GroupGeometry) -> Result<(), GroundError> {
    if !group
        .positions
        .iter()
        .flatten()
        .all(|value| value.is_finite())
    {
        return Err(GroundError::NonFiniteVertex {
            name: name.to_string(),
            group: group.group,
        });
    }
    if !group.indices.len().is_multiple_of(3) {
        return Err(GroundError::MalformedTriangleList {
            name: name.to_string(),
            group: group.group,
            indices: group.indices.len(),
        });
    }
    if let Some((triangle, _)) =
        group
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .enumerate()
            .find(|(_, chunk)| {
                chunk
                    .iter()
                    .any(|&index| index as usize >= group.positions.len())
            })
    {
        return Err(GroundError::IndexOutOfRange {
            name: name.to_string(),
            group: group.group,
            triangle,
        });
    }
    Ok(())
}

fn source(occurrence: u32, group: u32, triangle: usize) -> SourceId {
    SourceId {
        occurrence,
        group,
        triangle: triangle as u32,
    }
}

/// Classify a face: keep it when non-degenerate and steep enough.
fn make_triangle(a: [f64; 3], b: [f64; 3], c: [f64; 3], name_index: u32, source: SourceId) -> Face {
    let cross = cross(sub(b, a), sub(c, a));
    let area = length(cross);
    if !area.is_finite() || area <= DEGENERATE_AREA {
        return Face::Degenerate;
    }
    let normal = [cross[0] / area, cross[1] / area, cross[2] / area];
    if normal[1].abs() >= MAX_NORMAL_Y {
        return Face::Horizontal;
    }
    Face::Keep(BarrierTriangle {
        v: [a, b, c],
        normal,
        source,
        name_index,
    })
}

/// Conservative advancement of one sphere against one triangle.
fn sweep_triangle(
    triangle: &BarrierTriangle,
    start: [f64; 3],
    delta: [f64; 3],
    speed: f64,
    radius: f64,
) -> Sweep {
    let mut t = 0.0f64;
    for _ in 0..MAX_ITER {
        let centre = add(start, scale(delta, t));
        let closest = closest_point_on_triangle(centre, triangle.v);
        let distance = length(sub(centre, closest));
        if !distance.is_finite() {
            return Sweep::Uncertain;
        }
        if distance <= radius + DIST_TOL {
            return Sweep::Hit(t.clamp(0.0, 1.0));
        }
        if speed == 0.0 {
            return Sweep::Miss;
        }
        let advance = (distance - radius) / speed;
        if !advance.is_finite() || advance <= 0.0 {
            return Sweep::Uncertain;
        }
        t += advance;
        if t > 1.0 {
            return Sweep::Miss;
        }
    }
    Sweep::Uncertain
}

/// Contact point and normal for a triangle at sweep fraction `t`.
fn contact(
    triangle: &BarrierTriangle,
    start: [f64; 3],
    delta: [f64; 3],
    t: f64,
) -> Option<([f64; 3], [f64; 3])> {
    let centre = add(start, scale(delta, t.clamp(0.0, 1.0)));
    let closest = closest_point_on_triangle(centre, triangle.v);
    if !centre.iter().all(|value| value.is_finite())
        || !closest.iter().all(|value| value.is_finite())
    {
        return None;
    }
    let offset = sub(centre, closest);
    let normal = normalize(offset).unwrap_or(triangle.normal);
    Some((closest, normal))
}

/// Closest point on a triangle (Ericson, *Real-Time Collision Detection*).
fn closest_point_on_triangle(point: [f64; 3], triangle: [[f64; 3]; 3]) -> [f64; 3] {
    let [a, b, c] = triangle;
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(point, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(point, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return add(a, scale(ab, v));
    }
    let cp = sub(point, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return add(a, scale(ac, w));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return add(b, scale(sub(c, b), w));
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    add(a, add(scale(ab, v), scale(ac, w)))
}

/// True when `a` sorts before `b` by stable source identity.
fn source_less(a: SourceId, b: SourceId) -> bool {
    (a.occurrence, a.group, a.triangle).cmp(&(b.occurrence, b.group, b.triangle)) == Ordering::Less
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(v: [f64; 3], s: f64) -> [f64; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn length(v: [f64; 3]) -> f64 {
    dot(v, v).sqrt()
}

fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let len = length(v);
    if !len.is_finite() || len < 1e-12 {
        return None;
    }
    Some([v[0] / len, v[1] / len, v[2] / len])
}

/// A bounded uniform XZ grid; oversized triangles fall back to a global list.
#[derive(Debug, Clone)]
struct Grid {
    min_x: f64,
    min_z: f64,
    cell: f64,
    nx: usize,
    nz: usize,
    cells: Vec<Vec<u32>>,
}

/// Inclusive cell index range covering an XZ box.
#[derive(Debug, Clone, Copy)]
struct CellRange {
    cx0: usize,
    cx1: usize,
    cz0: usize,
    cz1: usize,
}

impl CellRange {
    /// Number of cells in the range.
    fn span(&self) -> usize {
        (self.cx1 - self.cx0 + 1).saturating_mul(self.cz1 - self.cz0 + 1)
    }
}

impl Grid {
    /// Cell index range covering an XZ box, or `None` for an empty/oversized box.
    fn cell_range(&self, min: [f64; 2], max: [f64; 2]) -> Option<CellRange> {
        if self.nx == 0 || self.nz == 0 || self.cells.is_empty() {
            return None;
        }
        let range = CellRange {
            cx0: cell_index((min[0] - self.min_x) / self.cell, self.nx),
            cx1: cell_index((max[0] - self.min_x) / self.cell, self.nx),
            cz0: cell_index((min[1] - self.min_z) / self.cell, self.nz),
            cz1: cell_index((max[1] - self.min_z) / self.cell, self.nz),
        };
        if range.span() > MAX_CELLS_PER_QUERY {
            // Too many cells to walk cheaply: test every triangle instead.
            return None;
        }
        Some(range)
    }
}

fn build_index(triangles: &[BarrierTriangle]) -> (Grid, Vec<u32>) {
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for triangle in triangles {
        for vertex in &triangle.v {
            min[0] = min[0].min(vertex[0]);
            min[1] = min[1].min(vertex[2]);
            max[0] = max[0].max(vertex[0]);
            max[1] = max[1].max(vertex[2]);
        }
    }
    let extent_x = (max[0] - min[0]).max(0.0);
    let extent_z = (max[1] - min[1]).max(0.0);
    let target = triangles.len().max(1) as f64;
    let area = extent_x * extent_z;
    let mut cell = if area > 0.0 {
        (area / target).sqrt()
    } else {
        1.0
    };
    if !cell.is_finite() || cell <= 0.0 {
        cell = 1.0;
    }
    while cell_count(extent_x, cell).saturating_mul(cell_count(extent_z, cell)) > MAX_GRID_CELLS {
        cell *= 2.0;
    }
    let nx = cell_count(extent_x, cell).max(1);
    let nz = cell_count(extent_z, cell).max(1);

    let mut cells = vec![Vec::new(); nx * nz];
    let mut oversized = Vec::new();
    for (index, triangle) in triangles.iter().enumerate() {
        let range = triangle_cells(triangle, min, cell, nx, nz);
        if range.span() > MAX_CELLS_PER_TRIANGLE {
            oversized.push(index as u32);
            continue;
        }
        for cx in range.cx0..=range.cx1 {
            for cz in range.cz0..=range.cz1 {
                cells[cx * nz + cz].push(index as u32);
            }
        }
    }
    (
        Grid {
            min_x: min[0],
            min_z: min[1],
            cell,
            nx,
            nz,
            cells,
        },
        oversized,
    )
}

fn triangle_cells(
    triangle: &BarrierTriangle,
    min: [f64; 2],
    cell: f64,
    nx: usize,
    nz: usize,
) -> CellRange {
    let mut x0 = f64::INFINITY;
    let mut x1 = f64::NEG_INFINITY;
    let mut z0 = f64::INFINITY;
    let mut z1 = f64::NEG_INFINITY;
    for vertex in &triangle.v {
        x0 = x0.min(vertex[0]);
        x1 = x1.max(vertex[0]);
        z0 = z0.min(vertex[2]);
        z1 = z1.max(vertex[2]);
    }
    CellRange {
        cx0: cell_index((x0 - min[0]) / cell, nx),
        cx1: cell_index((x1 - min[0]) / cell, nx),
        cz0: cell_index((z0 - min[1]) / cell, nz),
        cz1: cell_index((z1 - min[1]) / cell, nz),
    }
}

fn cell_count(extent: f64, cell: f64) -> usize {
    if !extent.is_finite() || extent <= 0.0 {
        return 1;
    }
    let count = (extent / cell).ceil();
    if !count.is_finite() {
        return usize::MAX;
    }
    (count as usize).saturating_add(1).max(1)
}

fn cell_index(value: f64, count: usize) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    let index = value as usize;
    index.min(count.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(
        occurrence: u32,
        name: &str,
        positions: Vec<[f64; 3]>,
        indices: Vec<u32>,
    ) -> MeshGeometry {
        MeshGeometry {
            occurrence,
            name: name.to_string(),
            groups: vec![GroupGeometry {
                group: 0,
                positions,
                indices,
            }],
        }
    }

    fn set(meshes: Vec<MeshGeometry>) -> BarrierSet {
        BarrierSet::build(meshes).expect("build")
    }

    /// The spec's vertical wall at X=5.
    fn wall() -> Vec<[f64; 3]> {
        vec![[5.0, -5.0, -5.0], [5.0, 5.0, -5.0], [5.0, 0.0, 5.0]]
    }

    fn wall_set() -> BarrierSet {
        set(vec![mesh(0, "WALL", wall(), vec![0, 1, 2])])
    }

    #[test]
    fn wall_oracle_face_hit_and_centre() {
        let result = wall_set()
            .sweep_sphere([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 0.75)
            .unwrap();
        let hit = result.hit.expect("hit");
        assert!((hit.t - 0.425).abs() < 1e-8, "t {}", hit.t);
        assert!((hit.point[0] - 5.0).abs() < 1e-8);
        let centre_x = 10.0 * hit.t;
        assert!((centre_x - 4.25).abs() < 1e-8, "centre {}", centre_x);
        assert!(
            (hit.normal[0] + 1.0).abs() < 1e-8,
            "normal {:?}",
            hit.normal
        );
        assert_eq!(hit.name, "WALL");
        assert_eq!(result.candidates, 1);
    }

    #[test]
    fn reverse_direction_matches() {
        let forward = wall_set()
            .sweep_sphere([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 0.75)
            .unwrap()
            .hit
            .unwrap();
        let backward = wall_set()
            .sweep_sphere([10.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.75)
            .unwrap()
            .hit
            .unwrap();
        assert!((forward.t - backward.t).abs() < 1e-12);
    }

    #[test]
    fn reversed_winding_is_identical() {
        let positions = vec![[5.0, -5.0, -5.0], [5.0, 0.0, 5.0], [5.0, 5.0, -5.0]];
        let flipped = set(vec![mesh(0, "WALL", positions, vec![0, 1, 2])]);
        let hit = flipped
            .sweep_sphere([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 0.75)
            .unwrap()
            .hit
            .unwrap();
        assert!((hit.t - 0.425).abs() < 1e-8);
    }

    #[test]
    fn initial_overlap_and_touch_return_zero() {
        let overlapping = wall_set()
            .sweep_sphere([4.5, 0.0, 0.0], [10.0, 0.0, 0.0], 0.75)
            .unwrap()
            .hit
            .unwrap();
        assert_eq!(overlapping.t, 0.0);
        let touching = wall_set()
            .sweep_sphere([4.25, 0.0, 0.0], [10.0, 0.0, 0.0], 0.75)
            .unwrap()
            .hit
            .unwrap();
        assert_eq!(touching.t, 0.0);
    }

    #[test]
    fn zero_length_sweep_checks_overlap() {
        let clear = wall_set()
            .sweep_sphere([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.75)
            .unwrap();
        assert!(clear.hit.is_none());
        let overlap = wall_set()
            .sweep_sphere([4.5, 0.0, 0.0], [4.5, 0.0, 0.0], 0.75)
            .unwrap();
        assert_eq!(overlap.hit.unwrap().t, 0.0);
    }

    #[test]
    fn no_hit_outside_the_triangle() {
        let result = wall_set()
            .sweep_sphere([0.0, 20.0, 0.0], [10.0, 20.0, 0.0], 0.75)
            .unwrap();
        assert!(result.hit.is_none());
    }

    #[test]
    fn finite_edges_miss_where_the_infinite_plane_would_hit() {
        // A small steep triangle whose plane X=5 is far from the path's Y.
        let positions = vec![[5.0, 4.0, 4.0], [5.0, 5.0, 4.0], [5.0, 4.0, 5.0]];
        let small = set(vec![mesh(0, "PATCH", positions, vec![0, 1, 2])]);
        let result = small
            .sweep_sphere([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 0.75)
            .unwrap();
        assert!(result.hit.is_none(), "plane test would have hit");
    }

    #[test]
    fn vertex_only_hit_and_miss() {
        // Small right-angle triangle at vertex (5,0,0); approach outside both edges.
        let positions = vec![[5.0, 0.0, 0.0], [5.0, 1.0, 0.0], [5.0, 0.0, 1.0]];
        let corner = set(vec![mesh(0, "CORNER", positions, vec![0, 1, 2])]);
        let far = corner
            .sweep_sphere([0.0, -0.3, -0.3], [10.0, -0.3, -0.3], 0.4)
            .unwrap();
        assert!(far.hit.is_none(), "sqrt(0.18) > 0.4");
        let near = corner
            .sweep_sphere([0.0, -0.3, -0.3], [10.0, -0.3, -0.3], 0.75)
            .unwrap()
            .hit
            .expect("vertex contact");
        for (got, want) in near.point.iter().zip([5.0, 0.0, 0.0]) {
            assert!((got - want).abs() < 1e-8, "point {:?}", near.point);
        }
    }

    #[test]
    fn edge_only_hit_and_miss() {
        let positions = vec![[5.0, 0.0, 0.0], [5.0, 1.0, 0.0], [5.0, 0.0, 1.0]];
        let edge = set(vec![mesh(0, "EDGE", positions, vec![0, 1, 2])]);
        // (y,z)=(0.6,0.6) is outside the hypotenuse y+z=1; closest point interior.
        let hit = edge
            .sweep_sphere([0.0, 0.6, 0.6], [10.0, 0.6, 0.6], 0.75)
            .unwrap()
            .hit
            .expect("edge contact");
        assert!(
            (hit.point[1] - 0.5).abs() < 1e-6 && (hit.point[2] - 0.5).abs() < 1e-6,
            "point {:?}",
            hit.point
        );
        let miss = edge
            .sweep_sphere([0.0, 4.0, 4.0], [10.0, 4.0, 4.0], 0.75)
            .unwrap();
        assert!(miss.hit.is_none());
    }

    #[test]
    fn offset_face_hit_within_radius() {
        // Path 0.75 m off the wall centreline; the finite face still contacts.
        let result = wall_set()
            .sweep_sphere([0.0, 0.75, 0.0], [10.0, 0.75, 0.0], 0.75)
            .unwrap();
        assert!(result.hit.is_some());
    }

    #[test]
    fn parallel_sweep_to_the_wall_can_hit_when_close() {
        // Moving in Z alongside the wall, 0.5 m away in X.
        let result = wall_set()
            .sweep_sphere([4.5, 0.0, -10.0], [4.5, 0.0, 10.0], 0.75)
            .unwrap();
        assert!(result.hit.is_some());
    }

    #[test]
    fn high_speed_crosses_a_thin_wall() {
        let result = wall_set()
            .sweep_sphere([0.0, 0.0, 0.0], [100.0, 0.0, 0.0], 0.75)
            .unwrap();
        let hit = result.hit.expect("high-speed hit");
        assert!((hit.t - 0.0425).abs() < 1e-9, "t {}", hit.t);
    }

    #[test]
    fn unresolved_candidate_cannot_be_hidden_by_another_hit() {
        let parallel = mesh(
            1,
            "NEAR_PARALLEL",
            vec![
                [0.0, -100.0, 0.750001],
                [100.0, -100.0, 0.750001],
                [0.0, 100.0, 0.750001],
            ],
            vec![0, 1, 2],
        );
        let barriers = set(vec![mesh(0, "WALL", wall(), vec![0, 1, 2]), parallel]);
        assert!(matches!(
            barriers.sweep_sphere([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 0.75),
            Err(GroundError::SweepNotConverged { .. })
        ));
    }

    #[test]
    fn layered_walls_return_the_earliest() {
        let near = mesh(
            0,
            "NEAR",
            vec![[3.0, -5.0, -5.0], [3.0, 5.0, -5.0], [3.0, 0.0, 5.0]],
            vec![0, 1, 2],
        );
        let far = mesh(1, "FAR", wall(), vec![0, 1, 2]);
        let hit = set(vec![far, near])
            .sweep_sphere([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 0.75)
            .unwrap()
            .hit
            .unwrap();
        assert_eq!(hit.name, "NEAR");
        assert!((hit.t - 0.225).abs() < 1e-8, "t {}", hit.t);
    }

    #[test]
    fn duplicate_ties_break_on_stable_id() {
        let first = mesh(0, "FIRST", wall(), vec![0, 1, 2]);
        let second = mesh(1, "SECOND", wall(), vec![0, 1, 2]);
        let hit = set(vec![second, first])
            .sweep_sphere([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 0.75)
            .unwrap()
            .hit
            .unwrap();
        assert_eq!(hit.source.occurrence, 0);
    }

    #[test]
    fn invalid_inputs_error() {
        let set = wall_set();
        assert!(set
            .sweep_sphere([f64::NAN, 0.0, 0.0], [1.0; 3], 0.75)
            .is_err());
        assert!(set
            .sweep_sphere([0.0; 3], [f64::INFINITY, 0.0, 0.0], 0.75)
            .is_err());
        assert!(set.sweep_sphere([0.0; 3], [1.0; 3], 0.0).is_err());
        assert!(set.sweep_sphere([0.0; 3], [1.0; 3], f64::NAN).is_err());
    }

    // --- selection and rejection ---

    #[test]
    fn steep_faces_are_retained_horizontal_are_counted() {
        let steep = mesh(0, "STEEP", wall(), vec![0, 1, 2]);
        let flat = mesh(
            1,
            "FLAT",
            vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 0.0, 10.0]],
            vec![0, 1, 2],
        );
        let set = set(vec![steep, flat]);
        assert_eq!(set.total_triangles(), 1);
        assert_eq!(set.stats().retained, 1);
        assert_eq!(set.stats().horizontal, 1);
        assert_eq!(set.stats().degenerate, 0);
    }

    #[test]
    fn degenerate_faces_are_counted() {
        let degenerate = mesh(
            0,
            "DEG",
            vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
            vec![0, 1, 2],
        );
        assert!(matches!(
            BarrierSet::build(vec![degenerate]),
            Err(GroundError::NoTriangles)
        ));
    }

    #[test]
    fn malformed_triangle_list_is_a_named_error() {
        let bad = mesh(0, "BAD", wall(), vec![0, 1]);
        let error = BarrierSet::build(vec![bad]).expect_err("malformed");
        assert!(matches!(error, GroundError::MalformedTriangleList { .. }));
        assert!(error.to_string().contains("BAD"));
    }

    #[test]
    fn out_of_range_index_is_a_named_error() {
        let bad = mesh(0, "BAD", wall(), vec![0, 1, 9]);
        assert!(matches!(
            BarrierSet::build(vec![bad]),
            Err(GroundError::IndexOutOfRange { .. })
        ));
    }

    #[test]
    fn non_finite_vertex_is_a_named_error() {
        let mut positions = wall();
        positions[0][1] = f64::NAN;
        let bad = mesh(0, "BAD", positions, vec![0, 1, 2]);
        assert!(matches!(
            BarrierSet::build(vec![bad]),
            Err(GroundError::NonFiniteVertex { .. })
        ));
    }

    // --- index ---

    #[test]
    fn indexed_sweep_equals_brute_force() {
        let mut meshes = Vec::new();
        for i in 0..40u32 {
            let x = 5.0 + f64::from(i) * 8.0;
            meshes.push(mesh(
                i,
                "WALL",
                vec![[x, -5.0, -5.0], [x, 5.0, -5.0], [x, 0.0, 5.0]],
                vec![0, 1, 2],
            ));
        }
        let set = set(meshes);
        for step in 0..20 {
            let start = [0.0, 0.4 * f64::from(step), 0.0];
            let end = [300.0, 0.4 * f64::from(step), 0.0];
            let indexed = set.sweep_sphere(start, end, 0.75).unwrap();
            let brute = brute_force(&set, start, end, 0.75);
            assert_eq!(
                indexed.hit.as_ref().map(|hit| (hit.t, hit.source)),
                brute,
                "step {step}"
            );
        }
    }

    fn brute_force(
        set: &BarrierSet,
        start: [f64; 3],
        end: [f64; 3],
        radius: f64,
    ) -> Option<(f64, SourceId)> {
        let delta = sub(end, start);
        let speed = length(delta);
        let mut best: Option<(f64, SourceId)> = None;
        for triangle in &set.triangles {
            if let Sweep::Hit(t) = sweep_triangle(triangle, start, delta, speed, radius) {
                let replace = match best {
                    None => true,
                    Some((current, source)) => {
                        if t < current - TIE_TOL {
                            true
                        } else if t > current + TIE_TOL {
                            false
                        } else {
                            source_less(triangle.source, source)
                        }
                    }
                };
                if replace {
                    best = Some((t, triangle.source));
                }
            }
        }
        best
    }

    #[test]
    fn local_candidate_count_is_below_total() {
        let mut meshes = Vec::new();
        for i in 0..200u32 {
            let x = f64::from(i) * 50.0;
            meshes.push(mesh(
                i,
                "WALL",
                vec![[x, -5.0, -5.0], [x, 5.0, -5.0], [x, 0.0, 5.0]],
                vec![0, 1, 2],
            ));
        }
        let set = set(meshes);
        let result = set
            .sweep_sphere([50.0, 0.0, 0.0], [50.0, 0.0, 0.1], 0.75)
            .unwrap();
        assert!(result.hit.is_some());
        assert!(
            result.candidates < set.total_triangles(),
            "{} of {}",
            result.candidates,
            set.total_triangles()
        );
    }

    #[test]
    fn swept_aabb_uses_the_xz_plane_not_y() {
        // A single tiny triangle far from the origin; the broad phase must find
        // it even though the sweep's Y barely changes (regression: Y was used
        // where Z belongs).
        let positions = vec![
            [431.6554260253906, 5.971600532531738, -487.0397644042969],
            [431.66961669921875, 5.874515056610107, -486.9311218261719],
            [431.6554260253906, 5.874515056610107, -487.0397644042969],
        ];
        let set = set(vec![mesh(0, "PTLIT", positions, vec![0, 1, 2])]);
        let face = set.face(0).unwrap();
        let centroid = [
            (face.vertices[0][0] + face.vertices[1][0] + face.vertices[2][0]) / 3.0,
            (face.vertices[0][1] + face.vertices[1][1] + face.vertices[2][1]) / 3.0,
            (face.vertices[0][2] + face.vertices[1][2] + face.vertices[2][2]) / 3.0,
        ];
        let start = add(centroid, scale(face.normal, 2.0));
        let end = add(centroid, scale(face.normal, -2.0));
        let hit = set
            .sweep_sphere(start, end, 0.75)
            .unwrap()
            .hit
            .expect("broad phase must find the swept triangle");
        assert!((hit.t - 0.3125).abs() < 1e-8, "t {}", hit.t);
    }

    #[test]
    fn large_and_extreme_geometry_stays_finite() {
        let huge = mesh(
            0,
            "HUGE",
            vec![
                [1.0e12, -1.0e12, -1.0e12],
                [1.0e12, 1.0e12, -1.0e12],
                [1.0e12, 0.0, 1.0e12],
            ],
            vec![0, 1, 2],
        );
        let set = set(vec![huge]);
        let result = set
            .sweep_sphere([0.0, 0.0, 0.0], [1.1e12, 0.0, 0.0], 0.75)
            .unwrap();
        assert!(result.hit.is_some());
        let hit = result.hit.unwrap();
        assert!(hit.t.is_finite() && hit.point.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn face_accessor_reports_source() {
        let set = wall_set();
        let face = set.face(0).unwrap();
        assert_eq!(face.name, "WALL");
        assert_eq!(face.source.occurrence, 0);
        assert!((face.normal[1]).abs() < 1e-12);
        assert!(set.face(1).is_none());
    }
}
