//! Fixed-step, double-precision rear-wheel-drive motion model.
//!
//! This is the DS11 prototype from `specs/DRIVE.md`: a flat-ground, no-collision
//! point model of one car. It is deliberately not a reconstruction of the
//! original solver; the constants are project choices. Everything is headless
//! and independent of Bevy so the motion can be tested on its own.

use std::collections::VecDeque;

use ground_query::GroundQuerySet;

use crate::barrier::BarrierStop;
use crate::config::{CarSpec, ConfigError, Spawn};
use crate::road::{step_follow, RoadFollower, StepOutcome};

/// Fixed simulation step in seconds (120 Hz).
pub const FIXED_STEP: f64 = 1.0 / 120.0;
/// Most wall time one rendered frame may convert into steps (0.1 s = 12 steps).
pub const MAX_CATCH_UP: f64 = 0.1;
/// Steering may move this fast, in degrees per second.
pub const STEER_RATE_DEG: f64 = 90.0;

/// Project constants; not tire `.tbc` grip or HDV aero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriveParams {
    /// Gravity, m/s^2.
    pub g: f64,
    /// Prototype friction coefficient used for the traction and yaw caps.
    pub mu: f64,
    /// Rolling-resistance coefficient.
    pub rolling: f64,
    /// Quadratic drag coefficient, kg/m.
    pub drag: f64,
}

impl Default for DriveParams {
    fn default() -> Self {
        DriveParams {
            g: 9.81,
            mu: 1.3,
            rolling: 0.015,
            drag: 0.5,
        }
    }
}

/// The selected gear; first gear is `Forward(1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gear {
    Neutral,
    Forward(usize),
}

impl Gear {
    /// Short label for the HUD: `N`, `1`, `2`, ...
    pub fn label(self) -> String {
        match self {
            Gear::Neutral => "N".to_string(),
            Gear::Forward(n) => n.to_string(),
        }
    }
}

/// Pedal and steering request for one step, already resolved from the keys.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Controls {
    /// Throttle, 0 or 1.
    pub throttle: f64,
    /// Brake, 0 or 1.
    pub brake: f64,
    /// Steering request, -1 (left) .. +1 (right).
    pub steer: f64,
}

/// Raw key state sampled once per rendered frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeldInput {
    pub throttle: bool,
    pub brake: bool,
    pub left: bool,
    pub right: bool,
}

impl HeldInput {
    /// True when any control key is held.
    pub fn any(&self) -> bool {
        self.throttle || self.brake || self.left || self.right
    }

    /// Resolve the keys into one step's controls (brake priority, steer cancel).
    pub fn controls(&self) -> Controls {
        let (throttle, brake) = if self.throttle && self.brake {
            (0.0, 1.0)
        } else {
            (
                if self.throttle { 1.0 } else { 0.0 },
                if self.brake { 1.0 } else { 0.0 },
            )
        };
        let steer = match (self.left, self.right) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        };
        Controls {
            throttle,
            brake,
            steer,
        }
    }
}

/// The mutable driving state, all in game axes except `speed`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State {
    /// Rear-axle centre X (game).
    pub pos_x: f64,
    /// Rear-axle centre Z (game).
    pub pos_z: f64,
    /// Heading yaw in radians, wrapped to `[-pi, pi)`; positive turns right.
    pub yaw: f64,
    /// Forward speed in m/s, never negative.
    pub speed: f64,
    /// Current steering angle in radians.
    pub steer: f64,
    /// Current engine speed in RPM (not clamped to the rev limit).
    pub rpm: f64,
    /// Selected gear.
    pub gear: Gear,
}

/// One validated car, its constants and its spawn, ready to step.
#[derive(Debug, Clone)]
pub struct Sim {
    pub spec: CarSpec,
    pub params: DriveParams,
    pub spawn: Spawn,
    pub state: State,
}

impl Sim {
    /// Validate the spawn and build the starting state.
    pub fn new(spec: CarSpec, params: DriveParams, spawn: Spawn) -> Result<Self, ConfigError> {
        if !spawn.origin_x.is_finite()
            || !spawn.origin_z.is_finite()
            || !spawn.yaw.is_finite()
            || !spawn.height.is_finite()
        {
            return Err(ConfigError::new(
                "grid spawn position/yaw is not finite; pick a valid grid slot",
            ));
        }
        let state = starting_state(&spec, &spawn);
        Ok(Sim {
            spec,
            params,
            spawn,
            state,
        })
    }

    /// Restore the selected spawn: zero speed/steer, first gear, idle RPM.
    pub fn reset(&mut self) {
        self.state = starting_state(&self.spec, &self.spawn);
    }

    /// Shift up one gear, clamped at the top; neutral is below first.
    pub fn shift_up(&mut self) {
        self.state.gear = match self.state.gear {
            Gear::Neutral => Gear::Forward(1),
            Gear::Forward(n) if n < self.spec.gear_count() => Gear::Forward(n + 1),
            gear => gear,
        };
    }

    /// Shift down one gear, clamped at neutral.
    pub fn shift_down(&mut self) {
        self.state.gear = match self.state.gear {
            Gear::Forward(1) => Gear::Neutral,
            Gear::Forward(n) => Gear::Forward(n - 1),
            Gear::Neutral => Gear::Neutral,
        };
    }

    /// Rear-axle centre `(x, z)` in game axes.
    pub fn rear_axle(&self) -> [f64; 2] {
        [self.state.pos_x, self.state.pos_z]
    }

    /// Mesh origin `(x, z)` in game axes for the current pose.
    pub fn mesh_origin(&self) -> [f64; 2] {
        let (a_x, a_z) = (self.spec.rear_axle[0], self.spec.rear_axle[1]);
        let (s, c) = self.state.yaw.sin_cos();
        [
            self.state.pos_x - a_x * c - a_z * s,
            self.state.pos_z + a_x * s - a_z * c,
        ]
    }

    /// Advance one fixed step. `controls` is already resolved for this step.
    pub fn step(&mut self, controls: &Controls, h: f64) {
        let speed = self.state.speed.max(0.0);
        let (ratio, final_drive) = self.gear_ratio_final();
        let coupled = coupled_rpm(speed, ratio, final_drive, self.spec.rear_radius);

        let (min_torque, max_torque) = match self.state.gear {
            Gear::Neutral => (0.0, 0.0),
            Gear::Forward(_) => self.spec.torque_at(coupled.max(self.spec.idle_rpm)),
        };
        let throttle = controls.throttle.clamp(0.0, 1.0);
        let mut torque = (1.0 - throttle) * min_torque + throttle * max_torque;
        // Neutral and zero throttle never make positive engine force, even if a
        // curve's idle drag sample is positive.
        if self.state.gear == Gear::Neutral || throttle <= 0.0 {
            torque = torque.min(0.0);
        }
        // Hard limiter: above the coupled rev limit, positive torque is cut but
        // negative drag torque is kept.
        if self.state.gear != Gear::Neutral && coupled >= self.spec.rev_limit {
            torque = torque.min(0.0);
        }

        let mut engine_force = torque * ratio * final_drive / self.spec.rear_radius;
        let traction_cap = self.params.mu * self.spec.mass * self.params.g / 2.0;
        engine_force = engine_force.clamp(-traction_cap, traction_cap);

        // Steering moves toward the request at a fixed rate.
        let requested = controls.steer.clamp(-1.0, 1.0) * self.spec.steer_lock_rad;
        let max_steer_step = STEER_RATE_DEG.to_radians() * h;
        self.state.steer += (requested - self.state.steer).clamp(-max_steer_step, max_steer_step);

        let brake = controls.brake.clamp(0.0, 1.0);
        let brake_torque_per_radius = (self.spec.brake_torque[0] + self.spec.brake_torque[1])
            / self.spec.front_radius
            + (self.spec.brake_torque[2] + self.spec.brake_torque[3]) / self.spec.rear_radius;
        let brake_force =
            (brake * brake_torque_per_radius).min(self.params.mu * self.spec.mass * self.params.g);
        let resistance =
            self.params.rolling * self.spec.mass * self.params.g + self.params.drag * speed * speed;

        let acceleration = (engine_force - brake_force - resistance) / self.spec.mass;
        let speed_next = (speed + acceleration * h).max(0.0);

        let mut yaw_rate = yaw_rate(
            self.state.steer,
            speed_next,
            self.spec.wheelbase,
            &self.params,
        );
        if speed_next == 0.0 {
            yaw_rate = 0.0;
        }
        let yaw_next = wrap_angle(self.state.yaw + yaw_rate * h);

        let pos_x_next = self.state.pos_x - speed_next * yaw_next.sin() * h;
        let pos_z_next = self.state.pos_z - speed_next * yaw_next.cos() * h;

        self.state.speed = speed_next;
        self.state.yaw = yaw_next;
        self.state.pos_x = pos_x_next;
        self.state.pos_z = pos_z_next;
        self.state.rpm = match self.state.gear {
            Gear::Neutral => self.spec.idle_rpm,
            Gear::Forward(_) => coupled_rpm(speed_next, ratio, final_drive, self.spec.rear_radius)
                .max(self.spec.idle_rpm),
        };
    }

    /// `(rear gear ratio, final drive)` for the current gear; `(0, 0)` neutral.
    fn gear_ratio_final(&self) -> (f64, f64) {
        match self.state.gear {
            Gear::Forward(n) => (self.spec.forward_gears[n - 1], self.spec.final_drive),
            Gear::Neutral => (0.0, 0.0),
        }
    }
}

/// Starting state for a spawn: rear-axle centre, zero speed/steer, first gear.
fn starting_state(spec: &CarSpec, spawn: &Spawn) -> State {
    let [a_x, a_z] = spec.rear_axle;
    let (s, c) = spawn.yaw.sin_cos();
    State {
        pos_x: spawn.origin_x + a_x * c + a_z * s,
        pos_z: spawn.origin_z - a_x * s + a_z * c,
        yaw: wrap_angle(spawn.yaw),
        speed: 0.0,
        steer: 0.0,
        rpm: spec.idle_rpm,
        gear: Gear::Forward(1),
    }
}

/// Engine speed coupled to a wheel speed, in RPM.
pub fn coupled_rpm(speed: f64, ratio: f64, final_drive: f64, radius: f64) -> f64 {
    60.0 * speed * ratio * final_drive / (std::f64::consts::TAU * radius)
}

/// Speed at which the coupled RPM reaches `rev_limit`, in m/s.
pub fn rev_limit_speed(rev_limit: f64, ratio: f64, final_drive: f64, radius: f64) -> f64 {
    rev_limit * std::f64::consts::TAU * radius / (60.0 * ratio * final_drive)
}

/// Bicycle yaw rate `v tan(delta) / L`, capped at `mu g / max(v, 0.5)`.
pub fn yaw_rate(steer: f64, speed: f64, wheelbase: f64, params: &DriveParams) -> f64 {
    let raw = speed * steer.tan() / wheelbase;
    let cap = params.mu * params.g / speed.max(0.5);
    raw.clamp(-cap, cap)
}

/// Wrap an angle to `[-pi, pi)`.
pub fn wrap_angle(angle: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    let mut wrapped = (angle + PI) % TAU;
    if wrapped < 0.0 {
        wrapped += TAU;
    }
    wrapped - PI
}

/// One queued gear request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shift {
    Up,
    Down,
}

/// Converts rendered-frame wall time into fixed steps, dropping excess time.
#[derive(Debug, Clone, Default)]
pub struct FixedStepper {
    accumulator: f64,
}

impl FixedStepper {
    pub fn new() -> Self {
        FixedStepper { accumulator: 0.0 }
    }

    /// Drop any pending time (used on pause and reset).
    pub fn reset(&mut self) {
        self.accumulator = 0.0;
    }

    /// Add `wall` seconds (clamped to [`MAX_CATCH_UP`]) and return the number of
    /// whole fixed steps to run. Excess time is dropped, never backlogged.
    pub fn advance(&mut self, wall: f64) -> u32 {
        if wall.is_finite() && wall > 0.0 {
            self.accumulator = (self.accumulator + wall).min(MAX_CATCH_UP);
        }
        let mut steps = 0u32;
        while self.accumulator + 1e-12 >= FIXED_STEP {
            self.accumulator -= FIXED_STEP;
            steps += 1;
        }
        steps
    }
}

/// A driving session: the simulation, its input queue and pause state.
///
/// Shift requests are queued and consumed once per fixed step, so they survive
/// a frame with zero steps and are never repeated across catch-up steps.
#[derive(Debug, Clone)]
pub struct Session {
    pub sim: Sim,
    pub stepper: FixedStepper,
    pub paused: bool,
    pending: VecDeque<Shift>,
    await_release: bool,
    follower: Option<RoadFollower>,
    barrier: Option<BarrierStop>,
}

impl Session {
    /// Start a session with an idle input queue, not paused.
    pub fn new(sim: Sim) -> Self {
        Session {
            sim,
            stepper: FixedStepper::new(),
            paused: false,
            pending: VecDeque::new(),
            await_release: false,
            follower: None,
            barrier: None,
        }
    }

    /// Attach (or replace) the DS13 road follower.
    pub fn set_road_follow(&mut self, follower: RoadFollower) {
        self.follower = Some(follower);
    }

    /// The road follower, when road-follow mode is active.
    pub fn follower(&self) -> Option<&RoadFollower> {
        self.follower.as_ref()
    }

    /// Attach the DS14 barrier stop; it is only queried in road-follow mode.
    pub fn set_barrier_stop(&mut self, barrier: BarrierStop) {
        self.barrier = Some(barrier);
    }

    /// The barrier stop, when `--barrier-stop` is active.
    pub fn barrier(&self) -> Option<&BarrierStop> {
        self.barrier.as_ref()
    }

    /// Queue one upshift (ignored while paused).
    pub fn request_shift_up(&mut self) {
        if !self.paused {
            self.pending.push_back(Shift::Up);
        }
    }

    /// Queue one downshift (ignored while paused).
    pub fn request_shift_down(&mut self) {
        if !self.paused {
            self.pending.push_back(Shift::Down);
        }
    }

    /// Restore the spawn and clear queued shifts and pending time.
    ///
    /// In road-follow mode this also restores the spawn surface/pose, clears any
    /// loss and waits for the control keys to be released before moving again.
    pub fn reset(&mut self) {
        self.sim.reset();
        if let Some(follower) = self.follower.as_mut() {
            follower.reset();
            self.await_release = true;
        }
        if let Some(barrier) = self.barrier.as_mut() {
            barrier.reset();
            self.await_release = true;
        }
        self.pending.clear();
        self.stepper.reset();
    }

    /// Toggle pause; both directions clear queued shifts and pending time, and
    /// resume waits for the control keys to be released and pressed again.
    pub fn toggle_pause(&mut self) {
        self.set_paused(!self.paused);
    }

    /// Set the pause state and clear queued shifts and pending time on change.
    pub fn set_paused(&mut self, paused: bool) {
        if self.paused != paused {
            self.paused = paused;
            self.pending.clear();
            self.stepper.reset();
            self.await_release = true;
        }
    }

    /// True when there is a queued shift not yet consumed.
    pub fn has_pending_shift(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Advance by `wall` seconds of held input; returns the steps run.
    pub fn advance(&mut self, wall: f64, held: HeldInput) -> u32 {
        self.advance_with(wall, held, None)
    }

    /// Advance by `wall` seconds with the DS13 road follower, querying `set`
    /// once per fixed step. The follower must already be set with
    /// [`Session::set_road_follow`].
    pub fn advance_with_ground(&mut self, wall: f64, held: HeldInput, set: &GroundQuerySet) -> u32 {
        self.advance_with(wall, held, Some(set))
    }

    fn advance_with(&mut self, wall: f64, held: HeldInput, set: Option<&GroundQuerySet>) -> u32 {
        if self.paused {
            self.stepper.reset();
            return 0;
        }
        // A latched surface loss or barrier stop freezes the car until R resets.
        if self
            .follower
            .as_ref()
            .is_some_and(|follower| follower.is_lost())
            || self
                .barrier
                .as_ref()
                .is_some_and(|barrier| barrier.is_stopped())
        {
            self.stepper.reset();
            return 0;
        }
        let steps = self.stepper.advance(wall);
        // "Released" is a frame-level event: clear the resume gate as soon as no
        // control key is held, even on a frame that runs no steps.
        if self.await_release && !held.any() {
            self.await_release = false;
        }
        let mut ran = 0;
        for _ in 0..steps {
            let gear_before = self.sim.state.gear;
            if let Some(shift) = self.pending.pop_front() {
                match shift {
                    Shift::Up => self.sim.shift_up(),
                    Shift::Down => self.sim.shift_down(),
                }
            }
            let controls = if self.await_release {
                Controls::default()
            } else {
                held.controls()
            };
            ran += 1;
            let outcome = {
                let (sim, follower, barrier) =
                    (&mut self.sim, &mut self.follower, &mut self.barrier);
                match (follower.as_mut(), set) {
                    (Some(follower), Some(set)) => {
                        step_follow(sim, follower, set, barrier.as_mut(), &controls, gear_before)
                    }
                    _ => {
                        sim.step(&controls, FIXED_STEP);
                        StepOutcome::Accepted
                    }
                }
            };
            if outcome.is_stopped() {
                // Drop the remaining backlog/queued input and stop this frame.
                self.pending.clear();
                self.stepper.reset();
                break;
            }
        }
        ran
    }
}
