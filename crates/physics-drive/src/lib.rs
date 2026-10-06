//! Headless DS11 driving prototype.
//!
//! [`CarSpec::from_physics`] validates a loaded `formats-hdv` car, [`Sim`] runs
//! the fixed-step motion and [`Session`] owns the input queue and pause state.
//! No Bevy types live here, so the whole prototype can be tested headlessly.

pub mod barrier;
pub mod config;
pub mod road;
pub mod sim;

pub use barrier::{
    proxy_center, BarrierContact, BarrierError, BarrierStop, StopStatus, PROXY_OFFSET_Y,
    PROXY_RADIUS,
};
pub use config::{CarSpec, ConfigError, Spawn, MAX_FORWARD_GEARS, MAX_STEER_LOCK_DEG};
pub use road::{
    step_follow, FollowError, FollowPose, FollowStatus, RoadFollower, StepOutcome, SurfaceSample,
    MAX_STEP_JUMP, SURFACE_BAND,
};
pub use sim::{
    coupled_rpm, rev_limit_speed, wrap_angle, yaw_rate, Controls, DriveParams, FixedStepper, Gear,
    HeldInput, Session, Sim, State, FIXED_STEP, MAX_CATCH_UP, STEER_RATE_DEG,
};

#[cfg(test)]
mod tests {
    use super::*;
    use formats_hdv::{CarPhysics, EngineFile, GearFile, Hdv, PmFile, TbcFile};
    use std::path::PathBuf;

    const HDV: &str = "\
[GENERAL]
Mass=800
CGHeight=0.250
TireBrand=brand
TireCompoundSetting=0

[CONTROLS]
SteerLockRange=(5.0, 0.5, 37)
SteerLockSetting=30

[ENGINE]
Normal=eng

[DRIVELINE]
WheelDrive=REAR
GearFile=g
FinalDriveSetting=0
ReverseSetting=0
ForwardGears=2
Gear1Setting=0
Gear2Setting=1

[SUSPENSION]
PhysicalModelFile=s.pm
LeftWheelBase=0.0
RightWheelBase=0.0

[FRONTLEFT]
BrakeTorque=3000
[FRONTRIGHT]
BrakeTorque=3000
[REARLEFT]
BrakeTorque=2000
[REARRIGHT]
BrakeTorque=2000
";

    const ENGINE: &str = "\
RPMTorque=(0, -20, 0)
RPMTorque=(2000, -20, 200)
RPMTorque=(8000, -30, 180)
IdleRPMLogic=(2000, 2000)
RevLimitRange=(8000, 100, 0)
RevLimitSetting=0
";

    const GEARS: &str = "\
[GEAR_RATIOS]
ratio=(12, 34)
ratio=(13, 36)
[FINAL_DRIVE]
bevel=(30, 42)
ratio=(13, 60)
";

    const TBC: &str = "\
[COMPOUND]
Name=\"Test\"
WetWeather=0
Front:
Radius=0.32
Rear:
Radius=0.33
";

    const PM: &str = "\
[BODY]
name=fl_wheel mass=(10) inertia=(1,1,1) pos=(0.8,0,-1.5) ori=(0,0,0)
[BODY]
name=fr_wheel mass=(10) inertia=(1,1,1) pos=(-0.8,0,-1.5) ori=(0,0,0)
[BODY]
name=rl_wheel mass=(10) inertia=(1,1,1) pos=(0.8,0,1.5) ori=(0,0,0)
[BODY]
name=rr_wheel mass=(10) inertia=(1,1,1) pos=(-0.8,0,1.5) ori=(0,0,0)
";

    fn physics_with(hdv: &str, engine: &str, gears: &str, tbc: &str, pm: &str) -> CarPhysics {
        CarPhysics {
            veh_path: PathBuf::from("car.veh"),
            hdv: Some(Hdv::parse(hdv)),
            engine: Some(EngineFile::parse(engine)),
            gears: Some(GearFile::parse(gears)),
            tires: Some(TbcFile::parse(tbc)),
            suspension: Some(PmFile::parse(pm)),
            missing: Vec::new(),
        }
    }

    fn valid_physics() -> CarPhysics {
        physics_with(HDV, ENGINE, GEARS, TBC, PM)
    }

    fn spec_error(hdv: &str, engine: &str, gears: &str, tbc: &str, pm: &str) -> String {
        CarSpec::from_physics(&physics_with(hdv, engine, gears, tbc, pm))
            .expect_err("expected a config error")
            .message
    }

    // --- configuration validation ---

    #[test]
    fn validates_a_car_and_reads_front_and_rear_radii() {
        let spec = CarSpec::from_physics(&valid_physics()).unwrap();
        assert_eq!(spec.mass, 800.0);
        assert_eq!(spec.front_radius, 0.32);
        assert_eq!(spec.rear_radius, 0.33);
        assert_eq!(spec.gear_count(), 2);
        assert_eq!(spec.forward_gears[0], 34.0 / 12.0);
        assert_eq!(spec.final_drive, (42.0 / 30.0) * (60.0 / 13.0));
        assert_eq!(spec.idle_rpm, 2000.0);
        assert_eq!(spec.rev_limit, 8000.0);
        assert_eq!(spec.brake_torque, [3000.0, 3000.0, 2000.0, 2000.0]);
        assert_eq!(spec.wheelbase, 3.0);
        assert_eq!(spec.rear_axle, [0.0, 1.5]);
        assert!((spec.steer_lock_rad - 20f64.to_radians()).abs() < 1e-12);
    }

    #[test]
    fn rejects_front_and_four_wheel_drive() {
        for drive in ["FRONT", "FOUR"] {
            let hdv = HDV.replace("WheelDrive=REAR", &format!("WheelDrive={drive}"));
            let message = spec_error(&hdv, ENGINE, GEARS, TBC, PM);
            assert!(message.contains(drive), "{message}");
        }
    }

    #[test]
    fn rejects_missing_middle_gear_index() {
        let hdv = HDV.replace("ForwardGears=2", "ForwardGears=3");
        let message = spec_error(&hdv, ENGINE, GEARS, TBC, PM);
        assert!(message.contains("Gear3Setting"), "{message}");
    }

    #[test]
    fn rejects_out_of_range_gear_index() {
        let hdv = HDV.replace("Gear2Setting=1", "Gear2Setting=99");
        let message = spec_error(&hdv, ENGINE, GEARS, TBC, PM);
        assert!(message.contains("out of range"), "{message}");
    }

    #[test]
    fn rejects_duplicate_and_descending_rpm() {
        let engine = "RPMTorque=(0, -20, 0)\nRPMTorque=(2000, -20, 200)\nRPMTorque=(2000, -20, 200)\nIdleRPMLogic=(1000, 1500)\nRevLimitRange=(8000, 100, 0)\nRevLimitSetting=0\n";
        let message = spec_error(HDV, engine, GEARS, TBC, PM);
        assert!(message.contains("strictly increase"), "{message}");
    }

    #[test]
    fn rejects_malformed_torque_row() {
        let engine = format!("{ENGINE}RPMTorque=garbage\n");
        let message = spec_error(HDV, &engine, GEARS, TBC, PM);
        assert!(message.contains("malformed RPMTorque"), "{message}");
    }

    #[test]
    fn accepts_positive_torque_interpolated_between_samples_outside_idle_band() {
        let engine = "RPMTorque=(0, -20, 200)\nRPMTorque=(10000, -20, 200)\nIdleRPMLogic=(2000, 2000)\nRevLimitRange=(8000, 100, 0)\nRevLimitSetting=0\n";
        let spec = CarSpec::from_physics(&physics_with(HDV, engine, GEARS, TBC, PM))
            .expect("positive interpolated torque within the operating band");
        assert_eq!(spec.torque_at(spec.idle_rpm).1, 200.0);
    }

    #[test]
    fn rejects_torque_positive_only_outside_operating_band() {
        let engine = "RPMTorque=(0, -20, -100)\nRPMTorque=(8000, -20, -100)\nRPMTorque=(10000, -20, 200)\nIdleRPMLogic=(2000, 2000)\nRevLimitRange=(8000, 100, 0)\nRevLimitSetting=0\n";
        assert!(spec_error(HDV, engine, GEARS, TBC, PM).contains("no positive max torque"));
    }

    #[test]
    fn rejects_non_finite_values() {
        let hdv = HDV.replace("Mass=800", "Mass=nan");
        let message = spec_error(&hdv, ENGINE, GEARS, TBC, PM);
        assert!(message.contains("Mass"), "{message}");
    }

    #[test]
    fn rejects_missing_rear_tire_radius() {
        let tbc = "[COMPOUND]\nName=\"Test\"\nFront:\nRadius=0.32\n";
        let message = spec_error(HDV, ENGINE, GEARS, tbc, PM);
        assert!(message.contains("Rear: Radius"), "{message}");
    }

    #[test]
    fn rejects_missing_wheel_body() {
        let pm = PM.replace(
            "name=rr_wheel mass=(10) inertia=(1,1,1) pos=(-0.8,0,1.5) ori=(0,0,0)\n",
            "",
        );
        let message = spec_error(HDV, ENGINE, GEARS, TBC, &pm);
        assert!(message.contains("rr_wheel"), "{message}");
    }

    #[test]
    fn rejects_non_finite_wheel_coordinates() {
        let pm = PM.replace("pos=(0.8,0,1.5)", "pos=(NaN,0,1.5)");
        assert!(spec_error(HDV, ENGINE, GEARS, TBC, &pm).contains("finite coordinates"));
    }

    #[test]
    fn rejects_non_zero_wheelbase_override() {
        let hdv = HDV.replace("LeftWheelBase=0.0", "LeftWheelBase=2.5");
        let message = spec_error(&hdv, ENGINE, GEARS, TBC, PM);
        assert!(message.contains("LeftWheelBase"), "{message}");
    }

    #[test]
    fn rejects_zero_brakes() {
        let hdv = HDV
            .replace("BrakeTorque=3000", "BrakeTorque=0")
            .replace("BrakeTorque=2000", "BrakeTorque=0");
        let message = spec_error(&hdv, ENGINE, GEARS, TBC, PM);
        assert!(message.contains("zero"), "{message}");
    }

    // --- numerical oracle ---

    fn oracle_spec() -> CarSpec {
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

    fn oracle_spawn() -> Spawn {
        Spawn {
            origin_x: 0.0,
            origin_z: 0.0,
            yaw: 0.0,
            height: 0.0,
        }
    }

    fn oracle_sim() -> Sim {
        Sim::new(oracle_spec(), DriveParams::default(), oracle_spawn()).unwrap()
    }

    #[test]
    fn oracle_rev_limit_speed() {
        let speed = rev_limit_speed(8000.0, 2.0, 4.0, 0.4);
        assert!((speed - 41.887902048).abs() < 1e-8, "{speed}");
    }

    #[test]
    fn oracle_uncapped_yaw_rate() {
        let rate = yaw_rate(10f64.to_radians(), 10.0, 3.0, &DriveParams::default());
        assert!((rate - 0.587756602362).abs() < 1e-9, "{rate}");
    }

    #[test]
    fn oracle_first_step() {
        let mut sim = oracle_sim();
        sim.step(
            &Controls {
                throttle: 1.0,
                brake: 0.0,
                steer: 0.0,
            },
            FIXED_STEP,
        );
        assert!(
            (sim.state.speed - 0.040440416667).abs() < 1e-9,
            "{}",
            sim.state.speed
        );
        let [x, z] = sim.rear_axle();
        let distance = (x * x + (z - 1.5) * (z - 1.5)).sqrt();
        assert!((distance - 0.000337003472).abs() < 1e-9, "{distance}");
        assert_eq!(sim.state.rpm, 2000.0);
    }

    #[test]
    fn limiter_cuts_positive_torque_above_the_coupled_rev_limit() {
        let mut sim = oracle_sim();
        sim.state.speed = rev_limit_speed(8000.0, 2.0, 4.0, 0.4);
        let before = sim.state.speed;
        sim.step(
            &Controls {
                throttle: 1.0,
                brake: 0.0,
                steer: 0.0,
            },
            FIXED_STEP,
        );
        assert!(
            sim.state.speed < before,
            "{} -> {}",
            before,
            sim.state.speed
        );
    }

    #[test]
    fn neutral_cannot_accelerate_from_throttle() {
        let mut sim = oracle_sim();
        sim.state.gear = Gear::Neutral;
        sim.step(
            &Controls {
                throttle: 1.0,
                brake: 0.0,
                steer: 0.0,
            },
            FIXED_STEP,
        );
        assert_eq!(sim.state.speed, 0.0);
        assert_eq!(sim.state.rpm, 2000.0);
    }

    #[test]
    fn coasting_slows_and_brakes_stop_without_reverse() {
        let mut sim = oracle_sim();
        sim.state.speed = 20.0;
        let coast = Controls::default();
        sim.step(&coast, FIXED_STEP);
        assert!(sim.state.speed < 20.0);

        sim.spec.brake_torque = [800.0; 4];
        let mut coasting = sim.clone();
        coasting.state.speed = 5.0;
        coasting.step(&coast, FIXED_STEP);
        sim.state.speed = 5.0;
        let brake = Controls {
            throttle: 0.0,
            brake: 1.0,
            steer: 0.0,
        };
        sim.step(&brake, FIXED_STEP);
        assert!(sim.state.speed < coasting.state.speed);
        for _ in 0..4000 {
            sim.step(&brake, FIXED_STEP);
            assert!(sim.state.speed >= 0.0);
        }
        assert_eq!(sim.state.speed, 0.0);
    }

    #[test]
    fn brake_priority_wins_over_throttle() {
        let held = HeldInput {
            throttle: true,
            brake: true,
            left: false,
            right: false,
        };
        let controls = held.controls();
        assert_eq!(controls.throttle, 0.0);
        assert_eq!(controls.brake, 1.0);
    }

    #[test]
    fn steering_at_rest_cannot_rotate() {
        let mut sim = oracle_sim();
        sim.step(
            &Controls {
                throttle: 0.0,
                brake: 0.0,
                steer: 1.0,
            },
            FIXED_STEP,
        );
        assert_eq!(sim.state.yaw, 0.0);
    }

    #[test]
    fn steering_signs_from_zero_yaw() {
        let step_steer = |steer: f64| {
            let mut sim = oracle_sim();
            sim.state.speed = 10.0;
            sim.step(
                &Controls {
                    throttle: 0.0,
                    brake: 0.0,
                    steer,
                },
                FIXED_STEP,
            );
            sim.state.yaw
        };
        assert!(step_steer(1.0) > 0.0);
        assert!(step_steer(-1.0) < 0.0);
    }

    #[test]
    fn steering_rate_is_capped() {
        let mut sim = oracle_sim();
        sim.step(
            &Controls {
                throttle: 0.0,
                brake: 0.0,
                steer: 1.0,
            },
            FIXED_STEP,
        );
        let expected = STEER_RATE_DEG.to_radians() * FIXED_STEP;
        assert!(
            (sim.state.steer - expected).abs() < 1e-12,
            "{}",
            sim.state.steer
        );
    }

    #[test]
    fn lateral_cap_holds_at_high_speed() {
        let params = DriveParams::default();
        let rate = yaw_rate(60f64.to_radians(), 100.0, 3.0, &params);
        let cap = params.mu * params.g / 100.0;
        assert!((rate - cap).abs() < 1e-12, "{rate} vs {cap}");
    }

    #[test]
    fn downshift_can_report_over_limit_rpm_without_nan_or_jump() {
        let mut sim = oracle_sim();
        sim.state.gear = Gear::Forward(2);
        sim.state.speed = 50.0;
        sim.shift_down();
        let before = sim.state.speed;
        sim.step(
            &Controls {
                throttle: 1.0,
                brake: 0.0,
                steer: 0.0,
            },
            FIXED_STEP,
        );
        assert!(sim.state.rpm.is_finite());
        assert!(sim.state.rpm > sim.spec.rev_limit);
        assert!((sim.state.speed - before).abs() < 0.5, "speed jumped");
    }

    // --- session: input queue, pause, continuity ---

    fn full_throttle() -> HeldInput {
        HeldInput {
            throttle: true,
            ..HeldInput::default()
        }
    }

    #[test]
    fn queued_shift_survives_a_zero_step_frame() {
        let mut session = Session::new(oracle_sim());
        session.request_shift_up();
        assert_eq!(session.advance(0.0, full_throttle()), 0);
        assert!(session.has_pending_shift());
        assert_eq!(session.sim.state.gear, Gear::Forward(1));
        assert_eq!(session.advance(FIXED_STEP, full_throttle()), 1);
        assert_eq!(session.sim.state.gear, Gear::Forward(2));
        assert!(!session.has_pending_shift());
    }

    #[test]
    fn one_shift_is_not_repeated_across_catch_up_steps() {
        let mut session = Session::new(oracle_sim());
        session.request_shift_up();
        let steps = session.advance(4.0 * FIXED_STEP, full_throttle());
        assert_eq!(steps, 4);
        assert_eq!(session.sim.state.gear, Gear::Forward(2));
    }

    #[test]
    fn reset_and_pause_clear_the_backlog() {
        let mut session = Session::new(oracle_sim());
        session.request_shift_up();
        session.request_shift_up();
        session.reset();
        assert!(!session.has_pending_shift());
        assert_eq!(session.sim.state.gear, Gear::Forward(1));

        session.request_shift_up();
        session.toggle_pause();
        assert!(session.paused);
        assert!(!session.has_pending_shift());
        // Shift requests are ignored while paused.
        session.request_shift_down();
        assert!(!session.has_pending_shift());
    }

    #[test]
    fn resume_waits_for_key_release() {
        let mut session = Session::new(oracle_sim());
        session.toggle_pause();
        session.toggle_pause();
        assert!(!session.paused);
        // Keys still held: input is ignored after resume.
        assert_eq!(session.advance(FIXED_STEP, full_throttle()), 1);
        assert_eq!(session.sim.state.speed, 0.0);
        // Release then press again.
        session.advance(0.0, HeldInput::default());
        session.advance(10.0 * FIXED_STEP, full_throttle());
        assert!(session.sim.state.speed > 0.0);
    }

    #[test]
    fn excess_wall_time_is_dropped() {
        let mut stepper = FixedStepper::new();
        assert_eq!(stepper.advance(1.0), 12);
        assert_eq!(stepper.advance(1.0), 12);
    }

    #[test]
    fn frame_grouping_does_not_change_the_result() {
        let total_steps = 120u32;
        let mut final_states = Vec::new();
        for group in [1u32, 2, 3, 4, 5, 12] {
            let mut session = Session::new(oracle_sim());
            let mut done = 0;
            while done < total_steps {
                let this = group.min(total_steps - done);
                let steps = session.advance(f64::from(this) * FIXED_STEP, full_throttle());
                assert_eq!(steps, this);
                done += steps;
            }
            final_states.push(session.sim.state);
        }
        for pair in final_states.windows(2) {
            assert!((pair[0].speed - pair[1].speed).abs() < 1e-8);
            assert!((pair[0].pos_x - pair[1].pos_x).abs() < 1e-8);
            assert!((pair[0].pos_z - pair[1].pos_z).abs() < 1e-8);
        }
    }

    #[test]
    fn same_duration_at_30_60_144_fps_matches() {
        let mut results = Vec::new();
        for fps in [30u32, 60, 144] {
            let mut session = Session::new(oracle_sim());
            let mut total = 0;
            for _ in 0..fps {
                total += session.advance(1.0 / f64::from(fps), full_throttle());
            }
            assert_eq!(total, 120);
            results.push(session.sim.state);
        }
        for pair in results.windows(2) {
            assert!((pair[0].speed - pair[1].speed).abs() < 1e-8);
            assert!((pair[0].pos_x - pair[1].pos_x).abs() < 1e-8);
            assert!((pair[0].pos_z - pair[1].pos_z).abs() < 1e-8);
        }
    }
}
