// Per-tick dump of a schedule, for plotting.
use elytrasim::opt::*;
use elytrasim::sim::*;
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let ps = read_pitches(&path);
    let v0 = Vec3::new(0.0, 0.167467, 0.200887);
    let st = replay_from(v0, &ps);
    println!("tick,pitch,vy,vz,speed,gamma,y,z,te");
    for t in 0..ps.len() {
        let s = &st[t];
        println!("{t},{:.6},{:.6},{:.6},{:.6},{:.4},{:.5},{:.4},{:.6}",
                 ps[t], s.vel.y, s.vel.z, s.vel.length(),
                 (-s.vel.y).atan2(s.vel.z).to_degrees(), s.pos.y, s.pos.z, s.total_energy());
    }
}
