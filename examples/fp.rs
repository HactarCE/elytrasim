// Bit-exact fingerprint of the physics, for comparing builds.
use elytrasim::sim::*;
fn main() {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut feed = |b: u64| { h ^= b; h = h.wrapping_mul(0x100000001b3); };
    let mut v = Vec3::new(0.0, 0.167467, 0.200887);
    for i in 0..20000i32 {
        let p = ((i % 361) - 180) as f32 * 0.5;
        v = update_fall_flying_movement(v, Rot { x: p, y: 0.0 });
        if !v.y.is_finite() || v.y.abs() > 1e6 { v = Vec3::new(0.0, 0.167467, 0.200887) }
        feed(v.y.to_bits()); feed(v.z.to_bits());
    }
    println!("physics {h:016x}");
    let r = Rot { x: 37.25, y: 0.0 }.look_angle();
    println!("look_angle {:016x} {:016x} {:016x}", r.x.to_bits(), r.y.to_bits(), r.z.to_bits());
}
