//! prototype barrier stop: one sphere proxy swept at the road-follow mesh.
//!
//! This is the prototype prototype from `specs/BARRIER_STOP.md`, on top of the prototype
//! road follower and the headless barrier geometry. The proxy is a single
//! `0.75 m` sphere centred `0.75 m` above the road-follow mesh origin; each
//! fixed step sweeps it from the last accepted centre to the proposed centre.
//! On contact the whole proposal is discarded and the car latches a distinct
//! **Barrier stopped** state until `R` resets it. There is no sliding, bounce,
//! damage or whole-car collision shape: this is a conservative stop, not a
//! reconstruction of the original collision behaviour.

use std::fmt;

use ground_query::{BarrierSet, GroundError, SourceId};

use crate::road::FollowPose;

/// Radius of the car proxy sphere, in metres.
pub const PROXY_RADIUS: f64 = 0.75;
/// Height of the proxy centre above the road-follow mesh origin, in metres.
pub const PROXY_OFFSET_Y: f64 = 0.75;

/// Whether the barrier latch has stopped the car.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopStatus {
    /// No barrier contact; the car may move.
    Clear,
    /// A contact latched the stop; the car is frozen until reset.
    Stopped,
}

/// The last barrier contact, for the HUD and the report.
#[derive(Debug, Clone, PartialEq)]
pub struct BarrierContact {
    /// Display name of the source mesh.
    pub name: String,
    /// Stable identity of the contacted triangle.
    pub source: SourceId,
    /// Contact fraction along the rejected sweep, in `[0, 1]`.
    pub t: f64,
    /// Candidate triangles tested for the rejecting sweep.
    pub candidates: usize,
}

/// A barrier preflight problem.
#[derive(Debug, Clone, PartialEq)]
pub enum BarrierError {
    /// The stationary spawn proxy already overlaps a retained barrier triangle.
    Overlap {
        /// Display name of the overlapped mesh.
        name: String,
        /// Stable identity of the overlapped triangle.
        source: SourceId,
    },
    /// The spawn overlap check itself failed.
    Query(GroundError),
}

impl fmt::Display for BarrierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BarrierError::Overlap { name, source } => write!(
                formatter,
                "spawn proxy overlaps barrier mesh {name} (occurrence {} group {} triangle {}); \
                 choose another grid or use road follow without --barrier-stop",
                source.occurrence, source.group, source.triangle
            ),
            BarrierError::Query(error) => {
                write!(formatter, "barrier preflight query failed: {error}")
            }
        }
    }
}

impl std::error::Error for BarrierError {}

/// The car's proxy centre for one accepted road-follow pose, in game axes.
pub fn proxy_center(pose: &FollowPose) -> [f64; 3] {
    [
        pose.origin[0],
        pose.origin[1] + PROXY_OFFSET_Y,
        pose.origin[2],
    ]
}

/// The barrier set plus the latched stop state and last accepted centre.
#[derive(Debug, Clone)]
pub struct BarrierStop {
    set: BarrierSet,
    status: StopStatus,
    last_center: [f64; 3],
    spawn_center: [f64; 3],
    last_contact: Option<BarrierContact>,
}

impl BarrierStop {
    /// Build the barrier state and reject a spawn that already overlaps.
    ///
    /// The stationary spawn proxy is checked once; an overlap is a named error
    /// rather than a silently disabled barrier.
    pub fn new(
        set: BarrierSet,
        follower: &crate::road::RoadFollower,
    ) -> Result<Self, BarrierError> {
        let spawn_center = proxy_center(follower.pose());
        let result = set
            .sweep_sphere(spawn_center, spawn_center, PROXY_RADIUS)
            .map_err(BarrierError::Query)?;
        if let Some(hit) = result.hit {
            return Err(BarrierError::Overlap {
                name: hit.name,
                source: hit.source,
            });
        }
        Ok(BarrierStop {
            set,
            status: StopStatus::Clear,
            last_center: spawn_center,
            spawn_center,
            last_contact: None,
        })
    }

    /// Current latch state.
    pub fn status(&self) -> StopStatus {
        self.status
    }

    /// True while a barrier contact has latched the car stopped.
    pub fn is_stopped(&self) -> bool {
        self.status == StopStatus::Stopped
    }

    /// The last contact that stopped the car, if any.
    pub fn last_contact(&self) -> Option<&BarrierContact> {
        self.last_contact.as_ref()
    }

    /// The retained barrier triangle count.
    pub fn triangle_count(&self) -> usize {
        self.set.total_triangles()
    }

    /// Restore the spawn centre and clear the latch and last contact.
    pub fn reset(&mut self) {
        self.status = StopStatus::Clear;
        self.last_center = self.spawn_center;
        self.last_contact = None;
    }

    /// Query the sweep from the last accepted centre to `proposed`'s centre.
    ///
    /// Returns `Ok(Some(contact))` when the sweep touches a barrier, `Ok(None)`
    /// when it is clear, and a named error when the query cannot be resolved.
    /// The caller decides whether to accept or latch; this does not move state.
    pub fn check(&mut self, proposed: &FollowPose) -> Result<Option<BarrierContact>, GroundError> {
        let center = proxy_center(proposed);
        let result = self
            .set
            .sweep_sphere(self.last_center, center, PROXY_RADIUS)?;
        match result.hit {
            Some(hit) => {
                let contact = BarrierContact {
                    name: hit.name,
                    source: hit.source,
                    t: hit.t,
                    candidates: result.candidates,
                };
                self.last_contact = Some(contact.clone());
                Ok(Some(contact))
            }
            None => Ok(None),
        }
    }

    /// Accept a clear proposal, advancing the last accepted centre.
    pub fn accept(&mut self, proposed: &FollowPose) {
        self.last_center = proxy_center(proposed);
    }

    /// Latch the stopped state.
    pub fn stop(&mut self) {
        self.status = StopStatus::Stopped;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ground_query::{GroundQuerySet, GroupGeometry, MeshGeometry};

    use crate::config::{CarSpec, Spawn};
    use crate::road::{FollowStatus, RoadFollower};
    use crate::sim::{DriveParams, FixedStepper, HeldInput, Session, Sim, FIXED_STEP};

    fn spec() -> CarSpec {
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
            rear_axle: [0.0, 1.5],
        }
    }

    fn sim() -> Sim {
        Sim::new(
            spec(),
            DriveParams::default(),
            Spawn {
                origin_x: 1.0,
                origin_z: -0.5,
                yaw: 0.0,
                height: 1.0,
            },
        )
        .unwrap()
    }

    fn ground(positions: Vec<[f64; 3]>, indices: Vec<u32>) -> GroundQuerySet {
        GroundQuerySet::build(vec![MeshGeometry {
            occurrence: 0,
            name: "GROUND".to_string(),
            groups: vec![GroupGeometry {
                group: 0,
                positions,
                indices,
            }],
        }])
        .unwrap()
    }

    fn flat_ground() -> GroundQuerySet {
        ground(
            vec![
                [-500.0, 1.0, -500.0],
                [500.0, 1.0, -500.0],
                [500.0, 1.0, 500.0],
                [-500.0, 1.0, 500.0],
            ],
            vec![0, 1, 2, 0, 2, 3],
        )
    }

    /// A steep wall in the plane Z=`z`, crossing the car's forward path.
    fn wall_set_at(z: f64) -> BarrierSet {
        BarrierSet::build(vec![MeshGeometry {
            occurrence: 0,
            name: "WALL".to_string(),
            groups: vec![GroupGeometry {
                group: 0,
                positions: vec![[-50.0, -1.0, z], [50.0, -1.0, z], [0.0, 3.0, z]],
                indices: vec![0, 1, 2],
            }],
        }])
        .unwrap()
    }

    fn wall_set() -> BarrierSet {
        wall_set_at(-3.0)
    }

    fn full_throttle() -> HeldInput {
        HeldInput {
            throttle: true,
            ..HeldInput::default()
        }
    }

    fn session_with_barrier(ground: &GroundQuerySet) -> Session {
        let sim = sim();
        let follower = RoadFollower::new(ground, &sim).unwrap();
        let barrier = BarrierStop::new(wall_set(), &follower).unwrap();
        let mut session = Session::new(sim);
        session.set_road_follow(follower);
        session.set_barrier_stop(barrier);
        session
    }

    #[test]
    fn spawn_overlap_is_a_named_error() {
        let sim = sim();
        let follower = RoadFollower::new(&flat_ground(), &sim).unwrap();
        // Wall right at the spawn proxy centre (game Z=-0.5), r=0.75 overlaps.
        let set = BarrierSet::build(vec![MeshGeometry {
            occurrence: 4,
            name: "SPAWN WALL".to_string(),
            groups: vec![GroupGeometry {
                group: 2,
                positions: vec![[-50.0, -1.0, -0.5], [50.0, -1.0, -0.5], [0.0, 3.0, -0.5]],
                indices: vec![0, 1, 2],
            }],
        }])
        .unwrap();
        let error = BarrierStop::new(set, &follower).expect_err("overlap");
        match &error {
            BarrierError::Overlap { name, source } => {
                assert_eq!(name, "SPAWN WALL");
                assert_eq!(source.occurrence, 4);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(error.to_string().contains("without --barrier-stop"));
    }

    #[test]
    fn contact_stops_latches_and_preserves_state() {
        let sim = sim();
        let follower = RoadFollower::new(&flat_ground(), &sim).unwrap();
        let barrier = BarrierStop::new(wall_set(), &follower).unwrap();
        let mut session = Session::new(sim);
        session.set_road_follow(follower);
        session.set_barrier_stop(barrier);

        let mut stopped_at = None;
        for step in 0..2000 {
            // A shift is queued every step; the stopping step must discard it.
            session.request_shift_up();
            session.advance_with_ground(FIXED_STEP, full_throttle(), &flat_ground());
            if session.barrier().unwrap().is_stopped() {
                stopped_at = Some(step);
                break;
            }
        }
        assert!(stopped_at.is_some(), "never contacted the wall");
        assert_eq!(session.sim.state.speed, 0.0);
        assert_eq!(session.sim.state.rpm, session.sim.spec.idle_rpm);
        assert!(!session.has_pending_shift(), "stopping step left a shift");
        let contact = session.barrier().unwrap().last_contact().unwrap().clone();
        assert_eq!(contact.name, "WALL");
        let frozen = *session.follower().unwrap().pose();
        let frozen_gear = session.sim.state.gear;

        // A latched barrier blocks every further step.
        for _ in 0..50 {
            assert_eq!(
                session.advance_with_ground(FIXED_STEP, full_throttle(), &flat_ground()),
                0
            );
        }
        assert!(session.barrier().unwrap().is_stopped());
        assert_eq!(session.sim.state.gear, frozen_gear);
        assert_eq!(*session.follower().unwrap().pose(), frozen);

        // Reset clears the latch and restores the spawn centre.
        session.reset();
        assert_eq!(session.barrier().unwrap().status(), StopStatus::Clear);
        assert_eq!(
            session.follower().unwrap().status(),
            FollowStatus::Following
        );
        assert!(session.advance_with_ground(1.0, full_throttle(), &flat_ground()) > 0);
    }

    #[test]
    fn surface_loss_wins_over_the_barrier() {
        // Ground with a hole just ahead; the barrier is far away and never hit.
        let sim = sim();
        let holey = ground(
            vec![
                [-500.0, 1.0, -2.0],
                [500.0, 1.0, -2.0],
                [500.0, 1.0, 500.0],
                [-500.0, 1.0, 500.0],
            ],
            vec![0, 1, 2, 0, 2, 3],
        );
        let follower = RoadFollower::new(&holey, &sim).unwrap();
        // The wall is far past the hole so the surface loss happens first.
        let barrier = BarrierStop::new(wall_set_at(-30.0), &follower).unwrap();
        let mut session = Session::new(sim);
        session.set_road_follow(follower);
        session.set_barrier_stop(barrier);

        let mut lost = false;
        for _ in 0..2000 {
            session.advance_with_ground(FIXED_STEP, full_throttle(), &holey);
            if session.follower().unwrap().is_lost() {
                lost = true;
                break;
            }
        }
        assert!(lost, "surface loss should win before the barrier");
        assert_eq!(session.barrier().unwrap().status(), StopStatus::Clear);
    }

    #[test]
    fn reset_while_paused_stays_paused_and_clears_barrier() {
        let mut session = session_with_barrier(&flat_ground());
        for _ in 0..2000 {
            session.advance_with_ground(FIXED_STEP, full_throttle(), &flat_ground());
            if session.barrier().unwrap().is_stopped() {
                break;
            }
        }
        session.set_paused(true);
        session.reset();
        assert!(session.paused);
        assert_eq!(session.barrier().unwrap().status(), StopStatus::Clear);
        assert_eq!(
            session.follower().unwrap().status(),
            FollowStatus::Following
        );
    }

    #[test]
    fn barrier_result_is_fps_independent() {
        let mut results = Vec::new();
        for fps in [30u32, 60, 144] {
            let mut session = session_with_barrier(&flat_ground());
            for _ in 0..fps {
                session.advance_with_ground(1.0 / f64::from(fps), full_throttle(), &flat_ground());
            }
            results.push((
                session.sim.state,
                *session.follower().unwrap().pose(),
                session.barrier().unwrap().status(),
                session.barrier().unwrap().last_center,
            ));
        }
        for pair in results.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!((a.0.speed - b.0.speed).abs() < 1e-8);
            assert!((a.0.pos_x - b.0.pos_x).abs() < 1e-8);
            assert!((a.0.pos_z - b.0.pos_z).abs() < 1e-8);
            for (x, y) in a.1.origin.iter().zip(b.1.origin) {
                assert!((x - y).abs() < 1e-8);
            }
            assert_eq!(a.2, b.2);
            for (x, y) in a.3.iter().zip(b.3) {
                assert!((x - y).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn zero_step_frame_preserves_the_barrier_centre() {
        let mut session = session_with_barrier(&flat_ground());
        let before = session.barrier().unwrap().last_center;
        assert_eq!(
            session.advance_with_ground(0.0, full_throttle(), &flat_ground()),
            0
        );
        assert_eq!(session.barrier().unwrap().last_center, before);
        assert_eq!(FixedStepper::new().advance(0.0), 0);
    }
}
