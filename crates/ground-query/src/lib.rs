//! Headless nearby-height ground query geometry and spatial index.
//!
//! No Bevy or physics dependency. The input is immutable indexed triangles in
//! **game world coordinates**, in metres, with stable source IDs and display
//! names; the output is a deterministic nearby-height query. The rules are the
//! prototype prototype choices from `specs/GROUND_QUERY.md`, not a reconstruction of
//! the original collision or handling model.
//!
//! Geometry is stored and queried in `f64`; mirroring is only a display concern.
//! Faces are normalized so their geometric normals point upward; the material
//! normal is never used. Degenerate and steeper-than-60-degree faces are dropped
//! and counted, so a query only ever returns a walkable nearby surface.

pub mod barrier;

pub use barrier::{BarrierFace, BarrierSet, BarrierStats, SweepHit, SweepResult};

use std::fmt;

/// A triangle steeper than 60 degrees is dropped (`|normal.y| < 0.5`).
const MIN_NORMAL_Y: f64 = 0.5;
/// Absolute XZ-projected cross product at or below this is degenerate (m^2).
const DEGENERATE_XZ: f64 = 1e-10;
/// Barycentric weights at or above this count as inside (shared edges).
const EDGE_EPSILON: f64 = -1e-9;
/// Upper bound on spatial-index cells, so allocation stays bounded.
const MAX_GRID_CELLS: usize = 1 << 20;
/// A triangle covering more cells than this is kept in the fallback list.
const MAX_CELLS_PER_TRIANGLE: usize = 128;

/// Stable identity of one source triangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceId {
    /// Stable occurrence ID of the source `MeshFile=`.
    pub occurrence: u32,
    /// MTS geometry group index.
    pub group: u32,
    /// Triangle index within its group.
    pub triangle: u32,
}

/// One mesh occurrence's geometry, already in game world coordinates.
///
/// The caller adds the model's placement position once while converting from
/// viewer-mirrored geometry; see [`unmirror_positions`].
#[derive(Debug, Clone, PartialEq)]
pub struct MeshGeometry {
    /// Stable occurrence ID used in every [`SourceId`] of this mesh.
    pub occurrence: u32,
    /// Display name of the source mesh.
    pub name: String,
    /// Geometry groups in MTS order.
    pub groups: Vec<GroupGeometry>,
}

/// One indexed geometry group of a mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupGeometry {
    /// MTS group index.
    pub group: u32,
    /// Game-world vertex positions, in metres.
    pub positions: Vec<[f64; 3]>,
    /// Triangle-list indices into `positions`.
    pub indices: Vec<u32>,
}

/// Counts of retained and rejected triangles after the build.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildStats {
    /// Triangles kept in the query set.
    pub retained: usize,
    /// Triangles dropped for a near-zero XZ-projected area.
    pub degenerate: usize,
    /// Triangles dropped for being steeper than 60 degrees.
    pub steep: usize,
}

impl BuildStats {
    /// Total dropped triangles.
    pub fn rejected(&self) -> usize {
        self.degenerate + self.steep
    }
}

/// A geometry or query problem, always naming the affected mesh or input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroundError {
    /// A source vertex is not finite.
    NonFiniteVertex { name: String, group: u32 },
    /// A triangle index does not reference a vertex of its group.
    IndexOutOfRange {
        name: String,
        group: u32,
        triangle: usize,
    },
    /// A query input is not finite, or the maximum distance is negative.
    InvalidInput { what: &'static str },
    /// A selected mesh group's index list is not a whole number of triangles.
    MalformedTriangleList {
        name: String,
        group: u32,
        indices: usize,
    },
    /// A swept-sphere query could not be resolved within its iteration budget.
    SweepNotConverged { name: String },
    /// No triangle survived the build, so there is nothing to query.
    NoTriangles,
}

impl fmt::Display for GroundError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroundError::NonFiniteVertex { name, group } => write!(
                formatter,
                "selected mesh {name} group {group} has a non-finite vertex"
            ),
            GroundError::IndexOutOfRange {
                name,
                group,
                triangle,
            } => write!(
                formatter,
                "selected mesh {name} group {group} triangle {triangle} has an out-of-range vertex index"
            ),
            GroundError::InvalidInput { what } => write!(formatter, "invalid query input: {what}"),
            GroundError::MalformedTriangleList {
                name,
                group,
                indices,
            } => write!(
                formatter,
                "selected mesh {name} group {group} has {indices} indices, not a whole number of triangles"
            ),
            GroundError::SweepNotConverged { name } => write!(
                formatter,
                "swept-sphere query against {name} did not converge; refusing to claim a clear path"
            ),
            GroundError::NoTriangles => write!(
                formatter,
                "no retained ground triangles; check the selected HATTarget meshes"
            ),
        }
    }
}

impl std::error::Error for GroundError {}

/// One retained triangle with its upward normal and stable identity.
#[derive(Debug, Clone)]
struct Triangle {
    v: [[f64; 3]; 3],
    normal: [f64; 3],
    source: SourceId,
    name_index: u32,
}

/// The best candidate found so far; ties break by stable index.
#[derive(Debug, Clone, Copy)]
struct Best {
    distance: f64,
    height: f64,
    index: usize,
}

/// The outcome of trying to retain one face.
enum Face {
    Keep(Triangle),
    Degenerate,
    Steep,
}

/// One query hit.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// Interpolated surface height in metres.
    pub height: f64,
    /// Upward unit geometric normal.
    pub normal: [f64; 3],
    /// Stable identity of the source triangle.
    pub source: SourceId,
    /// Display name of the source mesh.
    pub name: String,
    /// Stable retained-triangle index.
    pub triangle: u32,
}

/// A query result: an optional hit plus how many triangles were tested.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    /// The best nearby surface, or `None` when nothing is in range.
    pub hit: Option<Hit>,
    /// Number of candidate triangles tested for this query.
    pub candidates: usize,
}

/// A built ground query set: retained triangles plus a local spatial index.
#[derive(Debug, Clone)]
pub struct GroundQuerySet {
    triangles: Vec<Triangle>,
    names: Vec<String>,
    grid: Grid,
    oversized: Vec<u32>,
    stats: BuildStats,
}

impl GroundQuerySet {
    /// Build the query set from selected mesh geometry.
    ///
    /// Rejects non-finite vertices and out-of-range indices with a named error,
    /// drops degenerate and steep faces (counted), and fails on zero retained
    /// triangles.
    pub fn build(meshes: Vec<MeshGeometry>) -> Result<Self, GroundError> {
        let mut names: Vec<String> = Vec::new();
        let mut triangles: Vec<Triangle> = Vec::new();
        let mut stats = BuildStats::default();

        for mesh in meshes {
            let name_index = names.len() as u32;
            names.push(mesh.name.clone());
            for group in mesh.groups {
                for position in &group.positions {
                    if !position.iter().all(|value| value.is_finite()) {
                        return Err(GroundError::NonFiniteVertex {
                            name: mesh.name.clone(),
                            group: group.group,
                        });
                    }
                }
                for (triangle_index, chunk) in group.indices.as_chunks::<3>().0.iter().enumerate() {
                    let indices = *chunk;
                    let out_of_range = indices
                        .iter()
                        .any(|&index| index as usize >= group.positions.len());
                    if out_of_range {
                        return Err(GroundError::IndexOutOfRange {
                            name: mesh.name.clone(),
                            group: group.group,
                            triangle: triangle_index,
                        });
                    }
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
                        Face::Steep => stats.steep += 1,
                    }
                }
            }
        }

        if triangles.is_empty() {
            return Err(GroundError::NoTriangles);
        }

        let (grid, oversized) = build_index(&triangles);
        Ok(GroundQuerySet {
            triangles,
            names,
            grid,
            oversized,
            stats,
        })
    }

    /// Total retained triangles in the query set.
    pub fn total_triangles(&self) -> usize {
        self.triangles.len()
    }

    /// Build statistics (retained, degenerate, steep).
    pub fn stats(&self) -> BuildStats {
        self.stats
    }

    /// Find the nearby surface height beneath `(x, z)`, relative to `reference_y`.
    ///
    /// Both sides of a triangle are considered: this is a nearby-height query,
    /// not a downward ray. Absence of a hit returns `hit: None`, never a
    /// fallback plane. Invalid inputs return a named error.
    pub fn query(
        &self,
        x: f64,
        z: f64,
        reference_y: f64,
        max_distance: f64,
    ) -> Result<QueryResult, GroundError> {
        if !x.is_finite() {
            return Err(GroundError::InvalidInput { what: "X" });
        }
        if !z.is_finite() {
            return Err(GroundError::InvalidInput { what: "Z" });
        }
        if !reference_y.is_finite() {
            return Err(GroundError::InvalidInput {
                what: "reference Y",
            });
        }
        if !max_distance.is_finite() || max_distance < 0.0 {
            return Err(GroundError::InvalidInput {
                what: "maximum distance",
            });
        }

        let mut best: Option<Best> = None;
        let mut candidates = 0usize;
        if let Some(cell) = self.grid.cell_of(x, z) {
            for &index in &self.grid.cells[cell] {
                consider(
                    &self.triangles,
                    index,
                    x,
                    z,
                    reference_y,
                    max_distance,
                    &mut best,
                    &mut candidates,
                );
            }
        }
        for &index in &self.oversized {
            consider(
                &self.triangles,
                index,
                x,
                z,
                reference_y,
                max_distance,
                &mut best,
                &mut candidates,
            );
        }

        let hit = best.map(|best| {
            let triangle = &self.triangles[best.index];
            Hit {
                height: best.height,
                normal: triangle.normal,
                source: triangle.source,
                name: self.names[triangle.name_index as usize].clone(),
                triangle: best.index as u32,
            }
        });
        Ok(QueryResult { hit, candidates })
    }
}

/// Convert viewer-mirrored positions (Z negated) back to game world coordinates.
///
/// The viewer mirrors F1C geometry by negating Z once for display; a ground
/// query needs the original game axes, so this negation must happen exactly
/// once. Positions already carry any MTS placement offset.
pub fn unmirror_positions(positions: &[[f32; 3]]) -> Vec<[f64; 3]> {
    positions
        .iter()
        .map(|position| {
            [
                f64::from(position[0]),
                f64::from(position[1]),
                -f64::from(position[2]),
            ]
        })
        .collect()
}

fn source(occurrence: u32, group: u32, triangle: usize) -> SourceId {
    SourceId {
        occurrence,
        group,
        triangle: triangle as u32,
    }
}

/// Classify a face: keep it, or drop it as degenerate or too steep.
fn make_triangle(a: [f64; 3], b: [f64; 3], c: [f64; 3], name_index: u32, source: SourceId) -> Face {
    let edge1 = sub(b, a);
    let edge2 = sub(c, a);
    let cross = cross(edge1, edge2);
    // The XZ-projected cross product is the Y component of the 3D cross.
    if cross[1].abs() <= DEGENERATE_XZ {
        return Face::Degenerate;
    }
    let length = length(cross);
    if !length.is_finite() || length == 0.0 {
        return Face::Degenerate;
    }
    let mut normal = [cross[0] / length, cross[1] / length, cross[2] / length];
    let (a, b, c) = if normal[1] < 0.0 {
        normal = [-normal[0], -normal[1], -normal[2]];
        (a, c, b)
    } else {
        (a, b, c)
    };
    if normal[1] < MIN_NORMAL_Y {
        return Face::Steep;
    }
    Face::Keep(Triangle {
        v: [a, b, c],
        normal,
        source,
        name_index,
    })
}

/// Barycentric height at `(x, z)`, or `None` when the point is outside.
fn triangle_height(triangle: &Triangle, x: f64, z: f64) -> Option<f64> {
    let [a, b, c] = triangle.v;
    let denom = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
    if denom == 0.0 {
        return None;
    }
    let w0 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / denom;
    let w1 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / denom;
    let w2 = 1.0 - w0 - w1;
    if w0 < EDGE_EPSILON || w1 < EDGE_EPSILON || w2 < EDGE_EPSILON {
        return None;
    }
    Some(w0 * a[1] + w1 * b[1] + w2 * c[1])
}

#[allow(clippy::too_many_arguments)]
fn consider(
    triangles: &[Triangle],
    index: u32,
    x: f64,
    z: f64,
    reference_y: f64,
    max_distance: f64,
    best: &mut Option<Best>,
    candidates: &mut usize,
) {
    *candidates += 1;
    let triangle = &triangles[index as usize];
    let Some(height) = triangle_height(triangle, x, z) else {
        return;
    };
    let distance = (height - reference_y).abs();
    if distance > max_distance {
        return;
    }
    let replace = match best {
        None => true,
        Some(current) => {
            if distance < current.distance {
                true
            } else if distance > current.distance {
                false
            } else if height < current.height {
                true
            } else if height > current.height {
                false
            } else {
                (index as usize) < current.index
            }
        }
    };
    if replace {
        *best = Some(Best {
            distance,
            height,
            index: index as usize,
        });
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
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

impl Grid {
    /// Cell holding `(x, z)`, clamped into range; `None` for an empty grid.
    fn cell_of(&self, x: f64, z: f64) -> Option<usize> {
        if self.nx == 0 || self.nz == 0 || self.cells.is_empty() {
            return None;
        }
        let cx = cell_index((x - self.min_x) / self.cell, self.nx);
        let cz = cell_index((z - self.min_z) / self.cell, self.nz);
        Some(cx * self.nz + cz)
    }
}

fn build_index(triangles: &[Triangle]) -> (Grid, Vec<u32>) {
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
        let cx0 = cell_index((x0 - min[0]) / cell, nx);
        let cx1 = cell_index((x1 - min[0]) / cell, nx);
        let cz0 = cell_index((z0 - min[1]) / cell, nz);
        let cz1 = cell_index((z1 - min[1]) / cell, nz);
        let span = (cx1 - cx0 + 1) * (cz1 - cz0 + 1);
        if span > MAX_CELLS_PER_TRIANGLE {
            oversized.push(index as u32);
            continue;
        }
        for cx in cx0..=cx1 {
            for cz in cz0..=cz1 {
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

    fn set(meshes: Vec<MeshGeometry>) -> GroundQuerySet {
        GroundQuerySet::build(meshes).expect("build")
    }

    // --- invented numerical oracle ---

    fn oracle_triangle() -> Vec<[f64; 3]> {
        vec![[0.0, 1.0, 0.0], [4.0, 3.0, 0.0], [0.0, 1.0, 4.0]]
    }

    #[test]
    fn oracle_height_and_normal() {
        let set = set(vec![mesh(0, "ORACLE", oracle_triangle(), vec![0, 1, 2])]);
        let result = set.query(1.0, 1.0, 2.0, 2.0).unwrap();
        let hit = result.hit.expect("hit");
        assert!((hit.height - 1.5).abs() < 1e-9, "{}", hit.height);
        let expected = [-0.447213595499958, 0.894427190999916, 0.0];
        for (got, want) in hit.normal.iter().zip(expected) {
            assert!((got - want).abs() < 1e-9, "{:?}", hit.normal);
        }
    }

    #[test]
    fn reversed_winding_yields_the_same_result() {
        let positions = vec![[0.0, 1.0, 0.0], [0.0, 1.0, 4.0], [4.0, 3.0, 0.0]];
        let set = set(vec![mesh(0, "ORACLE", positions, vec![0, 1, 2])]);
        let hit = set.query(1.0, 1.0, 2.0, 2.0).unwrap().hit.expect("hit");
        assert!((hit.height - 1.5).abs() < 1e-9, "{}", hit.height);
        assert!(hit.normal[1] > 0.0);
    }

    #[test]
    fn outside_point_has_no_hit() {
        let set = set(vec![mesh(0, "ORACLE", oracle_triangle(), vec![0, 1, 2])]);
        assert!(set.query(3.0, 3.0, 2.0, 2.0).unwrap().hit.is_none());
    }

    #[test]
    fn shared_diagonal_edge_is_included() {
        let set = set(vec![mesh(0, "ORACLE", oracle_triangle(), vec![0, 1, 2])]);
        assert!(set.query(2.0, 2.0, 2.0, 2.0).unwrap().hit.is_some());
    }

    #[test]
    fn distance_band_rejects_far_surfaces() {
        let set = set(vec![mesh(0, "ORACLE", oracle_triangle(), vec![0, 1, 2])]);
        // height 1.5, reference 5, max 1 -> 3.5 away.
        assert!(set.query(1.0, 1.0, 5.0, 1.0).unwrap().hit.is_none());
    }

    // --- adapter ---

    #[test]
    fn unmirror_negates_z_exactly_once() {
        let game = [1.0_f64, 2.0, 3.0];
        let mirrored = [1.0f32, 2.0, -3.0];
        let back = unmirror_positions(&[mirrored]);
        assert_eq!(back, vec![game]);
    }

    #[test]
    fn mirrored_and_unmirrored_builds_agree() {
        let game = oracle_triangle();
        let mirrored: Vec<[f32; 3]> = game
            .iter()
            .map(|p| [p[0] as f32, p[1] as f32, -p[2] as f32])
            .collect();
        let via_adapter = set(vec![mesh(
            0,
            "M",
            unmirror_positions(&mirrored),
            vec![0, 1, 2],
        )]);
        let direct = set(vec![mesh(0, "M", game, vec![0, 1, 2])]);
        let a = via_adapter.query(1.0, 1.0, 2.0, 2.0).unwrap();
        let b = direct.query(1.0, 1.0, 2.0, 2.0).unwrap();
        assert_eq!(a, b);
    }

    // --- rejection ---

    #[test]
    fn degenerate_zero_area_xz_is_rejected_and_counted() {
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0], [2.0, 0.0, 2.0]];
        assert!(matches!(
            GroundQuerySet::build(vec![mesh(0, "D", positions, vec![0, 1, 2])]),
            Err(GroundError::NoTriangles)
        ));
    }

    #[test]
    fn vertical_face_is_rejected_as_degenerate() {
        // Triangle standing in the XY plane: XZ projection is a line.
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
        assert!(matches!(
            GroundQuerySet::build(vec![mesh(0, "V", positions, vec![0, 1, 2])]),
            Err(GroundError::NoTriangles)
        ));
        assert!(matches!(
            make_triangle(
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                0,
                source(0, 0, 0)
            ),
            Face::Degenerate
        ));
    }

    #[test]
    fn steep_face_is_rejected_but_counted() {
        // Normal Y ~0.196, steeper than 60 degrees from vertical.
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.2]];
        let copy = positions.clone();
        assert!(matches!(
            GroundQuerySet::build(vec![mesh(0, "S", positions, vec![0, 1, 2])]),
            Err(GroundError::NoTriangles)
        ));
        assert!(matches!(
            make_triangle(copy[0], copy[1], copy[2], 0, source(0, 0, 0)),
            Face::Steep
        ));
    }

    #[test]
    fn non_finite_vertex_is_a_named_error() {
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, f64::NAN, 1.0]];
        let error = GroundQuerySet::build(vec![mesh(0, "BAD", positions, vec![0, 1, 2])])
            .expect_err("non-finite");
        assert!(matches!(error, GroundError::NonFiniteVertex { .. }));
        assert!(error.to_string().contains("BAD"));
    }

    #[test]
    fn out_of_range_index_is_a_named_error() {
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 1.0]];
        let error = GroundQuerySet::build(vec![mesh(0, "BAD", positions, vec![0, 1, 9])])
            .expect_err("index");
        assert!(matches!(error, GroundError::IndexOutOfRange { .. }));
        assert!(error.to_string().contains("BAD"));
    }

    #[test]
    fn empty_build_fails() {
        assert!(matches!(
            GroundQuerySet::build(Vec::new()),
            Err(GroundError::NoTriangles)
        ));
    }

    // --- layers and ties ---

    fn layer(occurrence: u32, name: &str, y: f64) -> MeshGeometry {
        let positions = vec![[0.0, y, 0.0], [10.0, y, 0.0], [0.0, y, 10.0]];
        mesh(occurrence, name, positions, vec![0, 1, 2])
    }

    #[test]
    fn layer_selection_follows_the_reference() {
        let set = set(vec![layer(0, "LOW", 1.0), layer(1, "HIGH", 5.0)]);

        let low = set.query(2.0, 2.0, 1.2, 2.0).unwrap().hit.unwrap();
        assert_eq!(low.name, "LOW");
        assert!((low.height - 1.0).abs() < 1e-12);

        let high = set.query(2.0, 2.0, 4.8, 2.0).unwrap().hit.unwrap();
        assert_eq!(high.name, "HIGH");

        // Both 2 m away: 1 is |3-1| = 2, 5 is |3-5| = 2; lower height wins.
        let middle = set.query(2.0, 2.0, 3.0, 2.0).unwrap().hit.unwrap();
        assert_eq!(middle.name, "LOW");

        assert!(set.query(2.0, 2.0, 3.0, 1.0).unwrap().hit.is_none());
    }

    #[test]
    fn duplicate_geometry_ties_break_on_stable_id() {
        let set = set(vec![layer(0, "FIRST", 1.0), layer(1, "SECOND", 1.0)]);
        let hit = set.query(2.0, 2.0, 1.0, 2.0).unwrap().hit.unwrap();
        assert_eq!(hit.name, "FIRST");
        assert_eq!(hit.triangle, 0);
    }

    // --- index ---

    #[test]
    fn indexed_results_equal_brute_force() {
        let meshes = vec![
            layer(0, "A", 1.0),
            MeshGeometry {
                occurrence: 1,
                name: "B".to_string(),
                groups: vec![GroupGeometry {
                    group: 0,
                    positions: vec![[3.0, 2.0, 3.0], [7.0, 4.0, 3.0], [3.0, 2.0, 8.0]],
                    indices: vec![0, 1, 2],
                }],
            },
            mesh(
                2,
                "C",
                vec![[-5.0, 0.5, -5.0], [-1.0, 0.5, -5.0], [-5.0, 0.5, -1.0]],
                vec![0, 1, 2],
            ),
        ];
        let set = set(meshes);
        for xi in -8..12 {
            for zi in -8..12 {
                let (x, z) = (f64::from(xi), f64::from(zi));
                let indexed = set.query(x, z, 1.0, 5.0).unwrap();
                let brute = brute_force(&set, x, z, 1.0, 5.0);
                assert_eq!(
                    indexed.hit.map(|h| (h.triangle, h.height)),
                    brute,
                    "{x},{z}"
                );
            }
        }
    }

    fn brute_force(
        set: &GroundQuerySet,
        x: f64,
        z: f64,
        reference_y: f64,
        max_distance: f64,
    ) -> Option<(u32, f64)> {
        let mut best: Option<Best> = None;
        let mut candidates = 0;
        for (index, _) in set.triangles.iter().enumerate() {
            consider(
                &set.triangles,
                index as u32,
                x,
                z,
                reference_y,
                max_distance,
                &mut best,
                &mut candidates,
            );
        }
        best.map(|b| (b.index as u32, b.height))
    }

    #[test]
    fn localized_query_tests_fewer_candidates_than_total() {
        let mut meshes = Vec::new();
        for i in 0..100u32 {
            let x = f64::from(i) * 50.0;
            meshes.push(mesh(
                i,
                "TILE",
                vec![[x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x, 0.0, 1.0]],
                vec![0, 1, 2],
            ));
        }
        let set = set(meshes);
        let result = set.query(0.2, 0.2, 0.0, 2.0).unwrap();
        assert!(result.hit.is_some());
        assert!(
            result.candidates < set.total_triangles(),
            "{} candidates of {}",
            result.candidates,
            set.total_triangles()
        );
    }

    #[test]
    fn large_triangle_and_extreme_coordinates_stay_bounded() {
        let huge = mesh(
            0,
            "HUGE",
            vec![[0.0, 0.0, 0.0], [1.0e12, 0.0, 0.0], [0.0, 0.0, 1.0e12]],
            vec![0, 1, 2],
        );
        let neighbor = mesh(
            1,
            "NEAR",
            vec![[1.0, 1.0, 1.0], [2.0, 1.0, 1.0], [1.0, 1.0, 2.0]],
            vec![0, 1, 2],
        );
        let set = set(vec![huge, neighbor]);
        assert_eq!(set.total_triangles(), 2);
        let result = set.query(1.2, 1.2, 1.0, 1.0).unwrap();
        assert!(result.hit.is_some());
    }

    #[test]
    fn invalid_query_inputs_return_errors() {
        let set = set(vec![mesh(0, "ORACLE", oracle_triangle(), vec![0, 1, 2])]);
        assert!(set.query(f64::NAN, 0.0, 0.0, 1.0).is_err());
        assert!(set.query(0.0, f64::INFINITY, 0.0, 1.0).is_err());
        assert!(set.query(0.0, 0.0, f64::NAN, 1.0).is_err());
        assert!(set.query(0.0, 0.0, 0.0, -1.0).is_err());
    }

    #[test]
    fn source_identity_is_preserved() {
        let set = set(vec![mesh(7, "SOURCE", oracle_triangle(), vec![0, 1, 2])]);
        let hit = set.query(1.0, 1.0, 2.0, 2.0).unwrap().hit.unwrap();
        assert_eq!(
            hit.source,
            SourceId {
                occurrence: 7,
                group: 0,
                triangle: 0
            }
        );
        assert_eq!(hit.name, "SOURCE");
    }

    #[test]
    fn stats_count_retained_and_rejected() {
        let good = layer(0, "GOOD", 1.0);
        let degenerate = mesh(
            1,
            "DEG",
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0], [2.0, 0.0, 2.0]],
            vec![0, 1, 2],
        );
        let set = set(vec![good, degenerate]);
        let stats = set.stats();
        assert_eq!(stats.retained, 1);
        assert_eq!(stats.rejected(), 1);
    }
}
