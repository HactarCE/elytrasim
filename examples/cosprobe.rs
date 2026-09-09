//! What is cos(90 degrees) in the two trig modes? The sign matters: `look_hor_length` is |cos|,
//! and every conversion term is scaled by `look_angle.z / look_hor_length`, which is the *sign*
//! of cos. If cos(90) is negative, "forward" points at -z for that tick.
use elytrasim::sim::*;
fn main() {
    for m in [TrigMode::Libm, TrigMode::MthLut] {
        set_trig_mode(m);
        println!("--- trig {m}");
        for d in [89.99_f32, 89.999, 89.9999, 90.0] {
            let r = d * (std::f64::consts::PI / 180.0) as f32;
            let c = Mth::cos(r);
            println!("  pitch {d:>9}  cos {c:>15.6e}  sign {:>2}  look_hor {:.6e}",
                     if c < 0.0 { -1 } else { 1 }, c.abs());
        }
    }
}
