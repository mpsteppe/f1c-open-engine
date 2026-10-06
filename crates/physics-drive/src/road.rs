//! DS13 constrained road following.
//!
//! Kinematic surface following on top of the DS11 fixed-step motion and the
//! DS12 nearby-height query. This is **not** a physics model: there is no
//! suspension, gravity, tire contact, barrier or original-handling parity. The
//! rules here are the coordinator prototype choices from `specs/ROAD_FOLLOW.md`.
//!
//! Each fixed step proposes the ordinary DS11 step, queries the nearby surface
//! under the proposed rear axle using the **last accepted height** as the
//! reference, and either accepts the planar pose and surface atomically or
//! latches a distinct surface-lost state. Everything is headless and in f64.

use std::fmt;

use ground_query::{GroundError, GroundQuerySet, QueryResult, SourceId};

use crate::barrier::BarrierStop;
use crate::sim::{Controls, Gear, Sim, FIXED_STEP};

/// Maximum vertical lookup distance from the reference, in metres.
pub const SURFACE_BAND: f64 = 2.0;
/// Maximum accepted height change per fixed step, in metres.
pub const MAX_STEP_JUMP: f64 = 0.25;

/// Whether the follower currently has a valid surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowStatus {
    /// A surface was accepted on the last step (or at spawn/reset).
    Following,
    /// Support was lost; the car is frozen until `R` resets it.
    Lost,
}

/// The accepted local model basis and placement, in game axes.
///
/// `+X` is `left`, `+Y` is `up` and `+Z` is `rear` (so `-Z` is the forward
/// tangent). `origin` is the mesh origin; transforming the rear local anchor by
/// this basis reproduces `contact` exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FollowPose {
    /// Mesh origin O in game axes.
    pub origin: [f64; 3],
    /// Left axis L.
    pub left: [f64; 3],
    /// Up axis U (the surface normal).
    pub up: [f64; 3],
    /// Rear axis B = -F.
    pub rear: [f64; 3],
    /// Forward tangent F.
    pub forward: [f64; 3],
    /// Rear contact anchor C = (rear X, hit height, rear Z).
    pub contact: [f64; 3],
}

impl FollowPose {
    /// Build the pose for one accepted surface sample.
    ///
    /// `axle` is the PM rear-axle local centre `(x, z)`. Returns `None` for a
    /// non-finite or degenerate surface (no usable tangent basis).
    pub fn from_surface(
        rear_x: f64,
        height: f64,
        rear_z: f64,
        yaw: f64,
        normal: [f64; 3],
        axle: [f64; 2],
    ) -> Option<Self> {
        if !rear_x.is_finite()
            || !height.is_finite()
            || !rear_z.is_finite()
            || !yaw.is_finite()
            || normal.iter().any(|value| !value.is_finite())
        {
            return None;
        }
        let up = normalize(normal)?;
        // Horizontal requested forward for the game's forward (-Z at yaw 0).
        let horizontal = [-yaw.sin(), 0.0, -yaw.cos()];
        let along = dot(horizontal, up);
        let projected = [
            horizontal[0] - along * up[0],
            horizontal[1] - along * up[1],
            horizontal[2] - along * up[2],
        ];
        let forward = normalize(projected)?;
        let left = normalize(cross(forward, up))?;
        let rear = [-forward[0], -forward[1], -forward[2]];
        let contact = [rear_x, height, rear_z];
        let origin = [
            contact[0] - left[0] * axle[0] - rear[0] * axle[1],
            contact[1] - left[1] * axle[0] - rear[1] * axle[1],
            contact[2] - left[2] * axle[0] - rear[2] * axle[1],
        ];
        if origin.iter().any(|value| !value.is_finite())
            || !is_orthonormal(left, up, forward)
            || (dot(left, cross(up, rear)) - 1.0).abs() > 1e-9
        {
            return None;
        }
        Some(FollowPose {
            origin,
            left,
            up,
            rear,
            forward,
            contact,
        })
    }

    /// Game-local rear anchor transformed by this pose; equals [`Self::contact`].
    pub fn anchor(&self, axle: [f64; 2]) -> [f64; 3] {
        [
            self.origin[0] + self.left[0] * axle[0] + self.rear[0] * axle[1],
            self.origin[1] + self.left[1] * axle[0] + self.rear[1] * axle[1],
            self.origin[2] + self.left[2] * axle[0] + self.rear[2] * axle[1],
        ]
    }
}

/// One accepted surface sample, for the HUD and the last-height reference.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceSample {
    /// Interpolated surface height in metres.
    pub height: f64,
    /// Upward unit geometric normal.
    pub normal: [f64; 3],
    /// Stable identity of the source triangle.
    pub source: SourceId,
    /// Display name of the source mesh.
    pub name: String,
    /// Candidate triangles tested for this sample.
    pub candidates: usize,
}

impl SurfaceSample {
    fn from_hit(hit: &ground_query::Hit, candidates: usize) -> Self {
        SurfaceSample {
            height: hit.height,
            normal: hit.normal,
            source: hit.source,
            name: hit.name.clone(),
            candidates,
        }
    }
}

/// A reason the follower could not start or continue.
#[derive(Debug, Clone, PartialEq)]
pub enum FollowError {
    /// The underlying nearby-height query failed.
    Query(GroundError),
    /// No surface was within the band of the spawn rear axle.
    NoSurfaceAtSpawn {
        /// Spawn rear-axle X in game axes.
        rear_x: f64,
        /// Spawn rear-axle Z in game axes.
        rear_z: f64,
        /// Grid reference height in metres.
        reference: f64,
    },
    /// A hit existed but produced no usable tangent basis.
    DegeneratePose,
}

impl fmt::Display for FollowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FollowError::Query(error) => write!(formatter, "ground query failed: {error}"),
            FollowError::NoSurfaceAtSpawn {
                rear_x,
                rear_z,
                reference,
            } => write!(
                formatter,
                "no surface within {SURFACE_BAND:.1} m of the spawn rear axle \
                 ({rear_x:.3}, {rear_z:.3}) at grid Y {reference:.3}; \
                 choose another grid or use flat drive without --road-follow"
            ),
            FollowError::DegeneratePose => {
                write!(formatter, "surface at spawn gives a degenerate pose basis")
            }
        }
    }
}

impl std::error::Error for FollowError {}

/// Persistent road-follow state: surface sample, pose, spawn copy and status.
#[derive(Debug, Clone)]
pub struct RoadFollower {
    axle: [f64; 2],
    spawn_surface: SurfaceSample,
    spawn_pose: FollowPose,
    surface: SurfaceSample,
    pose: FollowPose,
    status: FollowStatus,
}

impl RoadFollower {
    /// Select the spawn surface and build the initial follower state.
    ///
    /// Queries the DS11 spawn rear-axle X/Z with the grid Y reference and the
    /// [`SURFACE_BAND`] distance, requires a hit, and initializes the reference
    /// to that hit height rather than the grid Y.
    pub fn new(set: &GroundQuerySet, sim: &Sim) -> Result<Self, FollowError> {
        let axle = sim.spec.rear_axle;
        let [rear_x, rear_z] = sim.rear_axle();
        let reference = sim.spawn.height;
        let result = set
            .query(rear_x, rear_z, reference, SURFACE_BAND)
            .map_err(FollowError::Query)?;
        let hit = result.hit.ok_or(FollowError::NoSurfaceAtSpawn {
            rear_x,
            rear_z,
            reference,
        })?;
        let pose =
            FollowPose::from_surface(rear_x, hit.height, rear_z, sim.state.yaw, hit.normal, axle)
                .ok_or(FollowError::DegeneratePose)?;
        let sample = SurfaceSample::from_hit(&hit, result.candidates);
        Ok(RoadFollower {
            axle,
            spawn_surface: sample.clone(),
            spawn_pose: pose,
            surface: sample,
            pose,
            status: FollowStatus::Following,
        })
    }

    /// Current surface status.
    pub fn status(&self) -> FollowStatus {
        self.status
    }

    /// True while a surface is lost and the car is frozen.
    pub fn is_lost(&self) -> bool {
        self.status == FollowStatus::Lost
    }

    /// Last accepted surface sample (HUD and reference).
    pub fn surface(&self) -> &SurfaceSample {
        &self.surface
    }

    /// Last accepted pose.
    pub fn pose(&self) -> &FollowPose {
        &self.pose
    }

    /// Restore the spawn surface and pose, clearing any lost state.
    pub fn reset(&mut self) {
        self.surface = self.spawn_surface.clone();
        self.pose = self.spawn_pose;
        self.status = FollowStatus::Following;
    }
}

/// The result of proposing one fixed road-follow step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// The proposal was accepted; the planar pose and surface advanced together.
    Accepted,
    /// Support was lost; the car is frozen until `R` resets it.
    SurfaceLost,
    /// A barrier contact rejected the proposal; the car is frozen until reset.
    BarrierStopped,
}

impl StepOutcome {
    /// True when the step latched a stop (surface loss or barrier) and the
    /// caller should clear queued input and backlog.
    pub fn is_stopped(self) -> bool {
        !matches!(self, StepOutcome::Accepted)
    }
}

/// Propose and accept (or reject) one fixed road-follow step.
///
/// `gear_before` is the gear to restore if the step is rejected (the queued
/// shift has already been applied to `sim` by the caller). When `barrier` is
/// supplied the accepted proposal's proxy sweep is checked before the move is
/// committed; a contact rejects the whole proposal. Returns the outcome so the
/// caller can clear queued input and backlog.
pub fn step_follow(
    sim: &mut Sim,
    follower: &mut RoadFollower,
    set: &GroundQuerySet,
    barrier: Option<&mut BarrierStop>,
    controls: &Controls,
    gear_before: Gear,
) -> StepOutcome {
    let mut proposed = sim.clone();
    proposed.step(controls, FIXED_STEP);

    let [rear_x, rear_z] = proposed.rear_axle();
    let reference = follower.surface.height;
    let accepted = match set.query(rear_x, rear_z, reference, SURFACE_BAND) {
        Ok(QueryResult { hit, candidates }) => match hit {
            Some(hit) if (hit.height - reference).abs() <= MAX_STEP_JUMP => {
                FollowPose::from_surface(
                    rear_x,
                    hit.height,
                    rear_z,
                    proposed.state.yaw,
                    hit.normal,
                    follower.axle,
                )
                .map(|pose| (pose, hit, candidates))
            }
            _ => None,
        },
        Err(_) => None,
    };

    let Some((pose, hit, candidates)) = accepted else {
        // Reject the whole proposed step: keep the last valid planar pose and
        // gear, stop the car and latch the distinct lost state.
        sim.state.gear = gear_before;
        sim.state.speed = 0.0;
        sim.state.rpm = sim.spec.idle_rpm;
        follower.status = FollowStatus::Lost;
        return StepOutcome::SurfaceLost;
    };

    if let Some(barrier) = barrier {
        match barrier.check(&pose) {
            Ok(None) => {}
            Ok(Some(_)) | Err(_) => {
                // A contact or an unresolvable sweep rejects the proposal whole:
                // keep the last pose, stop the car and latch the barrier state.
                sim.state.gear = gear_before;
                sim.state.speed = 0.0;
                sim.state.rpm = sim.spec.idle_rpm;
                barrier.stop();
                return StepOutcome::BarrierStopped;
            }
        }
        *sim = proposed;
        follower.surface = SurfaceSample::from_hit(&hit, candidates);
        follower.pose = pose;
        follower.status = FollowStatus::Following;
        barrier.accept(&pose);
        return StepOutcome::Accepted;
    }

    *sim = proposed;
    follower.surface = SurfaceSample::from_hit(&hit, candidates);
    follower.pose = pose;
    follower.status = FollowStatus::Following;
    StepOutcome::Accepted
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

/// Normalize a vector, or `None` when it is non-finite or near zero.
fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(v, v).sqrt();
    if !length.is_finite() || length < 1e-12 {
        return None;
    }
    Some([v[0] / length, v[1] / length, v[2] / length])
}

/// True when the three unit axes are mutually orthogonal (within tolerance).
fn is_orthonormal(x: [f64; 3], y: [f64; 3], z: [f64; 3]) -> bool {
    const TOL: f64 = 1e-9;
    (dot(x, x) - 1.0).abs() < TOL
        && (dot(y, y) - 1.0).abs() < TOL
        && (dot(z, z) - 1.0).abs() < TOL
        && dot(x, y).abs() < TOL
        && dot(x, z).abs() < TOL
        && dot(y, z).abs() < TOL
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CarSpec, Spawn};
    use crate::sim::{DriveParams, HeldInput, Session};
    use ground_query::{GroupGeometry, MeshGeometry};

    fn spec(rear_axle: [f64; 2]) -> CarSpec {
        CarSpec {
            mass: 800.0,
            forward_gears: vec![2.0, 1.5],
            final_drive: 4.0,
            torque: vec![(0.0, -20.0, 200.0), (8000.0, -20.0, 200.0)],
            rev_limit: 8000.0,
            idle_rpm: 2000.0,
            front_radius: 0.4,
            rear_radius: 0.4,
            steer_lock_rad: 10f64.to_radians(),
            brake_torque: [0.0; 4],
            wheelbase: 3.0,
            rear_axle,
        }
    }

    fn spawn(origin_x: f64, origin_z: f64, yaw: f64, height: f64) -> Spawn {
        Spawn {
            origin_x,
            origin_z,
            yaw,
            height,
        }
    }

    fn ground(positions: Vec<[f64; 3]>, indices: Vec<u32>) -> GroundQuerySet {
        GroundQuerySet::build(vec![MeshGeometry {
            occurrence: 0,
            name: "TEST".to_string(),
            groups: vec![GroupGeometry {
                group: 0,
                positions,
                indices,
            }],
        }])
        .expect("build ground")
    }

    /// Two-triangle quad covering `-extent..extent` in X/Z with `y(x, z)`.
    fn plane(extent: f64, y: impl Fn(f64, f64) -> f64) -> GroundQuerySet {
        let positions = vec![
            [-extent, y(-extent, -extent), -extent],
            [extent, y(extent, -extent), -extent],
            [extent, y(extent, extent), extent],
            [-extent, y(-extent, extent), extent],
        ];
        ground(positions, vec![0, 1, 2, 0, 2, 3])
    }

    /// Flat surface at `height`, sparing the Z < `z_min` region (a hole).
    fn plane_from(z_min: f64) -> GroundQuerySet {
        let positions = vec![
            [-50.0, 1.0, z_min],
            [50.0, 1.0, z_min],
            [50.0, 1.0, 50.0],
            [-50.0, 1.0, 50.0],
        ];
        ground(positions, vec![0, 1, 2, 0, 2, 3])
    }

    fn full_throttle() -> HeldInput {
        HeldInput {
            throttle: true,
            ..HeldInput::default()
        }
    }

    // --- pose oracle ---

    #[test]
    fn slope_pose_matches_the_spec_oracle() {
        let normal = [-0.447213595499958, 0.894427190999916, 0.0];
        let pose =
            FollowPose::from_surface(1.0, 1.5, 1.0, 0.0, normal, [0.0, 1.5]).expect("slope pose");
        for (got, want) in pose.origin.iter().zip([1.0, 1.5, -0.5]) {
            assert!((got - want).abs() < 1e-9, "origin {:?}", pose.origin);
        }
        for (got, want) in pose.forward.iter().zip([0.0, 0.0, -1.0]) {
            assert!((got - want).abs() < 1e-9, "forward {:?}", pose.forward);
        }
        // The spec's left vector follows L = F x U and is orthogonal to F.
        for (got, want) in pose
            .left
            .iter()
            .zip([0.894427190999916, 0.447213595499958, 0.0])
        {
            assert!((got - want).abs() < 1e-9, "left {:?}", pose.left);
        }
        for (got, want) in pose.rear.iter().zip([0.0, 0.0, 1.0]) {
            assert!((got - want).abs() < 1e-9, "rear {:?}", pose.rear);
        }
        // Transforming the rear local anchor (0, 0, 1.5) gives the contact.
        let anchored = pose.anchor([0.0, 1.5]);
        for (got, want) in anchored.iter().zip([1.0, 1.5, 1.0]) {
            assert!((got - want).abs() < 1e-9, "anchor {anchored:?}");
        }
    }

    #[test]
    fn nonzero_yaw_and_offset_axle_preserve_anchor_and_orthonormality() {
        let normal = [-0.447213595499958, 0.894427190999916, 0.0];
        let axle = [0.37, 1.5];
        for yaw in [-1.1, -0.3, 0.0, 0.7, 2.4] {
            let pose = FollowPose::from_surface(1.0, 1.5, 1.0, yaw, normal, axle).expect("pose");
            assert!(is_orthonormal(pose.left, pose.up, pose.forward));
            let anchored = pose.anchor(axle);
            for (got, want) in anchored.iter().zip(pose.contact) {
                assert!((got - want).abs() < 1e-9, "anchor {anchored:?}");
            }
            // forward is the normalized projection of the horizontal heading.
            let horizontal = [-yaw.sin(), 0.0, -yaw.cos()];
            let along = dot(horizontal, pose.up);
            let projection = [
                horizontal[0] - along * pose.up[0],
                horizontal[1] - along * pose.up[1],
                horizontal[2] - along * pose.up[2],
            ];
            let unit = normalize(projection).unwrap();
            for (got, want) in pose.forward.iter().zip(unit) {
                assert!((got - want).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn degenerate_surface_is_rejected() {
        assert!(
            FollowPose::from_surface(0.0, 0.0, 0.0, 0.0, [0.0, 0.0, 0.0], [0.0, 0.0]).is_none()
        );
        assert!(
            FollowPose::from_surface(0.0, 0.0, 0.0, 0.0, [f64::NAN, 1.0, 0.0], [0.0, 0.0])
                .is_none()
        );
    }

    // --- flat compatibility ---

    #[test]
    fn flat_follow_matches_plain_drive_and_only_changes_height() {
        let surface = plane(500.0, |_, _| 1.0);
        let sim = Sim::new(
            spec([0.0, 1.5]),
            DriveParams::default(),
            spawn(1.0, -0.5, 0.0, 1.0),
        )
        .unwrap();
        let mut follow = Session::new(sim.clone());
        follow.set_road_follow(RoadFollower::new(&surface, &sim).unwrap());
        let mut plain = Session::new(sim);

        for _ in 0..600 {
            follow.advance_with_ground(FIXED_STEP, full_throttle(), &surface);
            plain.advance(FIXED_STEP, full_throttle());
        }
        let a = follow.sim.state;
        let b = plain.sim.state;
        assert!(
            (a.speed - b.speed).abs() < 1e-9,
            "{} vs {}",
            a.speed,
            b.speed
        );
        assert!((a.pos_x - b.pos_x).abs() < 1e-9);
        assert!((a.pos_z - b.pos_z).abs() < 1e-9);
        assert!((a.yaw - b.yaw).abs() < 1e-9);
        let follower = follow.follower().unwrap();
        assert!((follower.pose().contact[1] - 1.0).abs() < 1e-12);
        assert!((follower.pose().origin[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn spawn_and_reset_anchor_returns_to_contact() {
        let surface = plane(500.0, |_, _| 1.0);
        let sim = Sim::new(
            spec([0.2, 1.5]),
            DriveParams::default(),
            spawn(1.0, -0.5, 0.4, 1.0),
        )
        .unwrap();
        let mut session = Session::new(sim.clone());
        session.set_road_follow(RoadFollower::new(&surface, &sim).unwrap());
        for _ in 0..200 {
            session.advance_with_ground(FIXED_STEP, full_throttle(), &surface);
        }
        session.reset();
        let follower = session.follower().unwrap();
        assert_eq!(follower.status(), FollowStatus::Following);
        let anchored = follower.pose().anchor([0.2, 1.5]);
        for (got, want) in anchored.iter().zip(follower.pose().contact) {
            assert!((got - want).abs() < 1e-12);
        }
    }

    // --- continuity ---

    #[test]
    fn gradual_slope_tracks_height_with_bounded_steps() {
        let surface = plane(2000.0, |_, z| 1.0 - 0.5 * z);
        let sim = Sim::new(
            spec([0.0, 1.5]),
            DriveParams::default(),
            spawn(1.0, -0.5, 0.0, 1.5),
        )
        .unwrap();
        let mut session = Session::new(sim.clone());
        session.set_road_follow(RoadFollower::new(&surface, &sim).unwrap());

        let mut min_height = f64::INFINITY;
        let mut max_height = f64::NEG_INFINITY;
        let mut max_jump: f64 = 0.0;
        let mut previous = session.follower().unwrap().surface().height;
        for _ in 0..1200 {
            let steps = session.advance_with_ground(FIXED_STEP, full_throttle(), &surface);
            assert_eq!(steps, 1);
            assert_eq!(
                session.follower().unwrap().status(),
                FollowStatus::Following
            );
            let height = session.follower().unwrap().surface().height;
            min_height = min_height.min(height);
            max_height = max_height.max(height);
            max_jump = max_jump.max((height - previous).abs());
            assert!(
                (height - previous).abs() <= MAX_STEP_JUMP + 1e-12,
                "jump {}",
                height - previous
            );
            previous = height;
        }
        assert!(
            max_height - min_height > 2.0,
            "climbed {}",
            max_height - min_height
        );
        assert!(max_jump <= MAX_STEP_JUMP + 1e-12);
    }

    #[test]
    fn stale_reference_never_snaps_to_a_distant_layer() {
        // Lower layer Y=1 and upper Y=5 covering the same XZ area.
        let meshes = vec![
            MeshGeometry {
                occurrence: 0,
                name: "LOW".to_string(),
                groups: vec![GroupGeometry {
                    group: 0,
                    positions: vec![
                        [-500.0, 1.0, -500.0],
                        [500.0, 1.0, -500.0],
                        [500.0, 1.0, 500.0],
                        [-500.0, 1.0, 500.0],
                    ],
                    indices: vec![0, 1, 2, 0, 2, 3],
                }],
            },
            MeshGeometry {
                occurrence: 1,
                name: "HIGH".to_string(),
                groups: vec![GroupGeometry {
                    group: 0,
                    positions: vec![
                        [-500.0, 5.0, -500.0],
                        [500.0, 5.0, -500.0],
                        [500.0, 5.0, 500.0],
                        [-500.0, 5.0, 500.0],
                    ],
                    indices: vec![0, 1, 2, 0, 2, 3],
                }],
            },
        ];
        let surface = GroundQuerySet::build(meshes).unwrap();
        let sim = Sim::new(
            spec([0.0, 1.5]),
            DriveParams::default(),
            spawn(1.0, -0.5, 0.0, 1.0),
        )
        .unwrap();
        let mut session = Session::new(sim.clone());
        session.set_road_follow(RoadFollower::new(&surface, &sim).unwrap());
        for _ in 0..600 {
            session.advance_with_ground(FIXED_STEP, full_throttle(), &surface);
            assert_eq!(
                session.follower().unwrap().status(),
                FollowStatus::Following
            );
            let sample = session.follower().unwrap().surface();
            assert_eq!(sample.name, "LOW");
            assert!((sample.height - 1.0).abs() < 1e-12);
        }
    }

    // --- loss ---

    #[test]
    fn hole_latches_loss_and_blocks_further_motion() {
        let surface = plane_from(-5.0);
        let sim = Sim::new(
            spec([0.0, 1.5]),
            DriveParams::default(),
            spawn(1.0, -0.5, 0.0, 1.0),
        )
        .unwrap();
        let mut session = Session::new(sim.clone());
        session.set_road_follow(RoadFollower::new(&surface, &sim).unwrap());

        let mut lost_at = None;
        for step in 0..2000 {
            session.advance_with_ground(FIXED_STEP, full_throttle(), &surface);
            if session.follower().unwrap().is_lost() {
                lost_at = Some(step);
                break;
            }
        }
        assert!(lost_at.is_some(), "never lost support");
        let follower = session.follower().unwrap();
        let frozen = follower.pose().contact;
        assert_eq!(session.sim.state.speed, 0.0);
        assert_eq!(session.sim.state.rpm, session.sim.spec.idle_rpm);
        // Lost state blocks every further step until reset.
        session.request_shift_up();
        for _ in 0..100 {
            assert_eq!(
                session.advance_with_ground(FIXED_STEP, full_throttle(), &surface),
                0
            );
        }
        assert!(session.follower().unwrap().is_lost());
        assert_eq!(session.follower().unwrap().pose().contact, frozen);
        // Reset restores the spawn surface and clears loss.
        session.reset();
        assert_eq!(
            session.follower().unwrap().status(),
            FollowStatus::Following
        );
        assert!((session.follower().unwrap().surface().height - 1.0).abs() < 1e-12);
        assert!(session.advance_with_ground(1.0, full_throttle(), &surface) > 0);
    }

    #[test]
    fn height_discontinuity_beyond_bound_latches_loss() {
        // Two flat halves: Y=1 for Z >= 0, Y=1.5 for Z < 0, adjacent at Z=0.
        let surface = GroundQuerySet::build(vec![
            MeshGeometry {
                occurrence: 0,
                name: "LOW".to_string(),
                groups: vec![GroupGeometry {
                    group: 0,
                    positions: vec![
                        [-50.0, 1.0, 0.0],
                        [50.0, 1.0, 0.0],
                        [50.0, 1.0, 50.0],
                        [-50.0, 1.0, 50.0],
                    ],
                    indices: vec![0, 1, 2, 0, 2, 3],
                }],
            },
            MeshGeometry {
                occurrence: 1,
                name: "STEP".to_string(),
                groups: vec![GroupGeometry {
                    group: 0,
                    positions: vec![
                        [-50.0, 1.5, -50.0],
                        [50.0, 1.5, -50.0],
                        [50.0, 1.5, 0.0],
                        [-50.0, 1.5, 0.0],
                    ],
                    indices: vec![0, 1, 2, 0, 2, 3],
                }],
            },
        ])
        .unwrap();
        let sim = Sim::new(
            spec([0.0, 1.5]),
            DriveParams::default(),
            spawn(1.0, -0.5, 0.0, 1.0),
        )
        .unwrap();
        let mut session = Session::new(sim.clone());
        session.set_road_follow(RoadFollower::new(&surface, &sim).unwrap());
        let mut lost = false;
        for _ in 0..2000 {
            session.advance_with_ground(FIXED_STEP, full_throttle(), &surface);
            if session.follower().unwrap().is_lost() {
                lost = true;
                break;
            }
        }
        assert!(lost, "0.5 m step should exceed the 0.25 m bound");
        assert_eq!(session.sim.state.speed, 0.0);
    }

    #[test]
    fn reset_while_paused_stays_paused_and_clears_loss() {
        let surface = plane_from(-3.0);
        let sim = Sim::new(
            spec([0.0, 1.5]),
            DriveParams::default(),
            spawn(1.0, -0.5, 0.0, 1.0),
        )
        .unwrap();
        let mut session = Session::new(sim.clone());
        session.set_road_follow(RoadFollower::new(&surface, &sim).unwrap());
        for _ in 0..2000 {
            session.advance_with_ground(FIXED_STEP, full_throttle(), &surface);
            if session.follower().unwrap().is_lost() {
                break;
            }
        }
        session.set_paused(true);
        session.reset();
        assert!(session.paused);
        assert_eq!(
            session.follower().unwrap().status(),
            FollowStatus::Following
        );
    }

    // --- timing ---

    #[test]
    fn road_follow_is_fps_independent() {
        let surface = plane(2000.0, |_, z| 1.0 - 0.5 * z);
        let mut results = Vec::new();
        for fps in [30u32, 60, 144] {
            let sim = Sim::new(
                spec([0.0, 1.5]),
                DriveParams::default(),
                spawn(1.0, -0.5, 0.0, 1.5),
            )
            .unwrap();
            let mut session = Session::new(sim.clone());
            session.set_road_follow(RoadFollower::new(&surface, &sim).unwrap());
            let mut total = 0;
            for _ in 0..fps {
                total +=
                    session.advance_with_ground(1.0 / f64::from(fps), full_throttle(), &surface);
            }
            assert_eq!(total, 120);
            results.push((
                session.sim.state,
                session.follower().unwrap().surface().height,
                *session.follower().unwrap().pose(),
            ));
        }
        for pair in results.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!((a.0.speed - b.0.speed).abs() < 1e-8);
            assert!((a.0.pos_x - b.0.pos_x).abs() < 1e-8);
            assert!((a.0.pos_z - b.0.pos_z).abs() < 1e-8);
            assert!((a.1 - b.1).abs() < 1e-8);
            for (x, y) in a.2.origin.iter().zip(b.2.origin) {
                assert!((x - y).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn zero_step_and_multi_step_frames_preserve_follow_state() {
        let surface = plane(2000.0, |_, z| 1.0 - 0.5 * z);
        let sim = Sim::new(
            spec([0.0, 1.5]),
            DriveParams::default(),
            spawn(1.0, -0.5, 0.0, 1.5),
        )
        .unwrap();
        let mut session = Session::new(sim.clone());
        session.set_road_follow(RoadFollower::new(&surface, &sim).unwrap());
        // Zero-step frame keeps the surface sample.
        let before = session.follower().unwrap().surface().clone();
        assert_eq!(
            session.advance_with_ground(0.0, full_throttle(), &surface),
            0
        );
        assert_eq!(session.follower().unwrap().surface(), &before);
        // A multi-step catch-up frame runs every step through the follower.
        let steps = session.advance_with_ground(4.0 * FIXED_STEP, full_throttle(), &surface);
        assert_eq!(steps, 4);
        assert_eq!(
            session.follower().unwrap().status(),
            FollowStatus::Following
        );
    }
}
