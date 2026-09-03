// Does routing sin/cos through a branch change the f32 results at all?
use std::hint::black_box;

#[inline] fn direct_sin(x: f32) -> f32 { x.sin() }
#[inline] fn direct_cos(x: f32) -> f32 { x.cos() }

#[inline] fn branched_sin(x: f32) -> f32 { if black_box(true) { x.sin() } else { 0.0 } }
#[inline] fn branched_cos(x: f32) -> f32 { if black_box(true) { x.cos() } else { 0.0 } }

fn main() {
    // the pattern in Rot::look_angle: two sin/cos pairs at the same arguments
    let mut ndiff = 0u32;
    for i in -90000..=90000i32 {
        let x = i as f32 * 0.001 * (std::f64::consts::PI / 180.0) as f32;
        let (a, b) = (direct_sin(x), direct_cos(x));
        let (c, d) = (branched_sin(x), branched_cos(x));
        if a.to_bits() != c.to_bits() || b.to_bits() != d.to_bits() { ndiff += 1 }
    }
    println!("direct vs branched: {ndiff} differing of 180001");

    // and sin/cos adjacent (sincos-able) vs far apart
    let mut ndiff2 = 0u32;
    for i in -90000..=90000i32 {
        let x = i as f32 * 0.001 * (std::f64::consts::PI / 180.0) as f32;
        let pair_s = x.sin();
        let pair_c = x.cos();
        let lone_s = { let v = black_box(x); v.sin() };
        let lone_c = { let v = black_box(x); v.cos() };
        if pair_s.to_bits() != lone_s.to_bits() || pair_c.to_bits() != lone_c.to_bits() { ndiff2 += 1 }
    }
    println!("paired vs isolated:  {ndiff2} differing of 180001");
}
