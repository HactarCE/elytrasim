use crate::sim::*;

/// split the `vz0.abs()` too.
/// parent is [`crate::cases::tick`].
pub fn tick(vel: Vec3, rot: Rot) -> Vec3 {
    assert_eq!(vel.x, 0.0);
    assert_eq!(rot.y, 0.0);
    // let mut vy: f64 = vel.y;
    // let mut vz: f64 = vel.z;
    let vy0: f64 = vel.y;
    let vz0: f64 = vel.z;
    let pitch: f64 = rot.x as f64;
    let vel = ();
    let rot = ();

    // renamed from `lean_angle`
    let pitch_rad: f64 = pitch * (std::f64::consts::PI / 180.0);

    let pitch_sin: f64 = pitch_rad.sin();

    // renamed from `look_angle.z`
    // also renamed from `look_hor_length`
    let pitch_cos: f64 = pitch_rad.cos();
    assert!(pitch_cos >= 0.0);

    // renamed from `lift_force`
    let pitch_cos2: f64 = pitch_cos * pitch_cos;
    assert!(0.25 >= (-1.0 + pitch_cos2 * 0.75));
    let vy_grav: f64 = vy0 + GRAVITY * (-1.0 + pitch_cos2 * 0.75);
    let vy0 = ();

    let (vy_fin, vz_fin) = if vz0 >= 0.0 {
        // forwards

        // converts v_down to v_forward if falling
        if vy_grav < 0.0 {
            let convert: f64 = vy_grav * -0.1 * pitch_cos2;
            assert!(convert >= 0.0);
            let vy_fall: f64 = vy_grav + convert;
            let vz_fall: f64 = vz0 + convert;
            // let vy_fall: f64 = vy_grav * (1.0 - 0.1 * pitch_cos2);
            let vy_grav = ();
            let vz0_ = vz0;

            // converts v_forward to v_up if looking up
            if pitch_rad < 0.0 {
                // forwards, falling, looking up

                assert!(pitch_sin <= 0.0);
                let convert: f64 = vz0 * -pitch_sin * 0.04;
                assert!(convert >= 0.0);
                let vy_up = vy_fall + convert * 3.2;
                let vz_up = vz_fall - convert;
                let vy_fall = ();
                let vz_fall = ();

                // turning
                let vz_turn = vz_up * 0.9 + vz0 * 0.1;
                let vz_up = ();
                (vy_up, vz_turn)
            } else {
                // forwards, falling, looking down

                // turning
                // idk whether this is tight
                assert!(vz_fall >= vz0);
                let vz_turn = vz_fall * 0.9 + vz0 * 0.1;
                let vz_fall = ();
                (vy_fall, vz_turn)
            }
        } else {
            // rising

            // converts v_forward to v_up if looking up
            if pitch_rad < 0.0 {
                // forwards, rising, looking up

                assert!(pitch_sin <= 0.0);
                let convert: f64 = vz0 * -pitch_sin * 0.04;
                assert!(convert >= 0.0);
                let vy_up = vy_grav + convert * 3.2;
                let vz_up = if vz0 > 0.0 {
                    vz0 * (1.0 + pitch_sin * 0.04)
                } else if vz0 == 0.0 {
                    0.0
                } else {
                    vz0 * (1.0 - pitch_sin * 0.04)
                };
                let vy_grav = ();

                // turning
                assert!(vz_up <= vz0);
                let vz_turn = vz_up * 0.9 + vz0 * 0.1;
                let vz_up = ();
                (vy_up, vz_turn)
            } else {
                // forwards, rising, looking down

                (vy_grav, vz0)
            }
        }
    } else {
        // backwards

        // converts v_down to v_forward if falling
        if vy_grav < 0.0 {
            let convert: f64 = vy_grav * -0.1 * pitch_cos2;
            assert!(convert >= 0.0);
            let vy_fall: f64 = vy_grav + convert;
            let vz_fall: f64 = vz0 + convert;
            // let vy_fall: f64 = vy_grav * (1.0 - 0.1 * pitch_cos2);
            let vy_grav = ();

            // converts v_forward to v_up if looking up
            if pitch_rad < 0.0 {
                // backwards, falling, looking up

                assert!(pitch_sin <= 0.0);
                let convert: f64 = vz0 * pitch_sin * 0.04;
                assert!(convert >= 0.0);
                let vy_up = vy_fall + convert * 3.2;
                let vz_up = vz_fall - convert;
                let vy_fall = ();
                let vz_fall = ();

                // turning
                let vz_turn = vz_up * 0.9 - vz0 * 0.1;
                let vz_up = ();
                (vy_up, vz_turn)
            } else {
                // backwards, falling, looking down

                // turning
                let vz_turn = vz_fall * 0.9 - vz0 * 0.1;
                let vz_fall = ();
                (vy_fall, vz_turn)
            }
        } else {
            // rising

            // converts v_forward to v_up if looking up
            if pitch_rad < 0.0 {
                // backwards, rising, looking up

                assert!(pitch_sin <= 0.0);
                let convert: f64 = -vz0 * -pitch_sin * 0.04;
                assert!(convert >= 0.0);
                let vy_up = vy_grav + convert * 3.2;
                let vz_up = if vz0 > 0.0 {
                    vz0 * (1.0 + pitch_sin * 0.04)
                } else if vz0 == 0.0 {
                    0.0
                } else {
                    vz0 * (1.0 - pitch_sin * 0.04)
                };
                let vy_grav = ();

                // turning
                assert!(vz_up <= -vz0);
                let vz_turn = vz_up * 0.9 - vz0 * 0.1;
                let vz_up = ();
                (vy_up, vz_turn)
            } else {
                // backwards, rising, looking down

                // turning
                let vz_turn = vz0 * 0.8;
                let vz0 = ();
                (vy_grav, vz_turn)
            }
        }
    };

    Vec3 {
        x: 0.0,
        y: vy_fin * 0.98,
        z: vz_fin * 0.99,
    }
}
