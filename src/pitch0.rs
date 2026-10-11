use crate::sim::*;

/// assume that pitch is 0,
/// and that we're falling,
/// and that we're moving forwards.
/// parent is [`crate::nomut::tick`].
pub fn tick(vel: Vec3, rot: Rot) -> Vec3 {
    assert_eq!(vel.x, 0.0);
    assert_eq!(rot.y, 0.0);
    let vy0 = vel.y;
    let vz0 = vel.z;
    let pitch = rot.x as f64;
    let vel = ();
    let rot = ();

    assert!(vy0 <= 0.0);
    assert!(vz0 >= 0.0);
    assert_eq!(pitch, 0.0);

    let vy_grav = vy0 - GRAVITY * 0.25;

    // converts v_down to v_forward if falling
    // let vy_fall = vy_grav - vy_grav * 0.1;
    let vy_fall = vy_grav * 0.9;
    let vz_turn = vz0 - vy_grav * 0.09;

    Vec3 {
        x: 0.0,
        y: vy_fall * 0.98,
        z: vz_turn * 0.99,
    }
}
