//! One tick, two pitches: what actually differs. usage: tickdiff <file> <tick> <p1> <p2>
use elytrasim::opt::*;
use elytrasim::sim::*;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&a[1]).unwrap();
    let (obj, ps) = { let p = Profile::parse(&text).unwrap(); set_trig_mode(p.trig); (p.obj, p.pitches) };
    if let Some(i) = a.iter().position(|x| x == "--trig") { set_trig_mode(a[i + 1].parse().unwrap()) }
    let t: usize = a[2].parse().unwrap();
    let st = obj.replay(&ps);
    println!("state at tick {t}: vy {:.9} vz {:.9}", st[t].vel.y, st[t].vel.z);
    for k in 3..5 {
        let p: f64 = a[k].parse().unwrap();
        let n1 = ticked(&st[t], p);
        // and the whole tail from there
        let mut s = n1.clone();
        for &q in &ps[t + 1..] { s = ticked(&s, q) }
        let lean = (p as f32) * (std::f64::consts::PI / 180.0) as f32;
        println!("  p {p:>13.6}  Mth::cos {:>12.4e}  Mth::sin {:>12.8}  f64cos^2 {:>10.3e}  \
next vy {:>12.8} vz {:>12.8}   tail dJ {:.4}",
                 Mth::cos(lean), Mth::sin(lean), (lean as f64).cos().powi(2),
                 n1.vel.y, n1.vel.z,
                 obj.j(&s) - obj.j(&State { pos: Vec3::ZERO, vel: obj.v0 }));
    }
    // where does the trajectory diverge?
    let mut q = ps.clone(); q[t] = a[4].parse().unwrap();
    let (s1, s2) = (obj.replay(&ps), obj.replay(&q));
    let mut first = None;
    for i in t..=ps.len() {
        let d = (s1[i].vel.y - s2[i].vel.y).abs() + (s1[i].vel.z - s2[i].vel.z).abs();
        if d > 0.01 && first.is_none() { first = Some(i) }
    }
    println!("first |dv| > 0.01 at tick {:?}; final y {:.3} vs {:.3}", first,
             s1.last().unwrap().pos.y, s2.last().unwrap().pos.y);
}
