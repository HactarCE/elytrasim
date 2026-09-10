//! How wide is the optimum in pitch, right at the top of the range?
//!
//! `look_hor_length` is |cos(pitch)| and every conversion term is scaled by
//! `look_angle.z / look_hor_length`, which is the *sign* of cos. So the turning term is
//! `(sign * |v_z| - v_z) * 0.1`: zero when cos is positive, and -0.2 * v_z -- twenty percent of
//! your forward speed, every tick -- when it is negative. Meanwhile `lift_force = cos^2` is
//! already zero to fifteen digits well before that. There is therefore a pitch just below 90
//! degrees where the aerodynamics are entirely off and the turning term still costs nothing, and
//! a hair above it where forward reverses.
//!
//! This scans one tick's pitch across that region at whatever resolution you ask for, printing
//! the whole schedule's dJ. usage: razor <file> <tick> [lo hi n] [--trig libm|mth_lut]
use elytrasim::opt::*;
use elytrasim::sim::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    // --trig, if given, overrides every profile header for every file in this invocation.
    let forced: Option<TrigMode> =
        a.iter().position(|x| x == "--trig").map(|i| a[i + 1].parse().unwrap());
    let f = &a[1];
    let t: usize = a[2].parse().unwrap();
    let (lo, hi, n) = if a.len() > 5 {
        (a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse::<usize>().unwrap())
    } else { (89.9995, 90.0, 2001) };
    let text = std::fs::read_to_string(f).unwrap();
    let (obj, ps) = {
        // Each file picks its own mode from its own header, and an explicit --trig overrides
        // every header. Setting the mode inside the loop without resetting it let a
        // headerless file silently inherit the previous profile's physics.
        let parsed = Profile::parse(&text);
        set_trig_mode(forced.unwrap_or_else(|| parsed.as_ref().map(|p| p.trig).unwrap_or_default()));
        set_flight_mode(parsed.as_ref().map(|p| p.flight).unwrap_or_default());
        match parsed {
            Ok(p) => (p.obj, p.pitches),
            Err(_) => (Objective { v0: V0, n: 0, lambda: 0.0 }, read_pitches(f)),
        }
    };
    let obj = Objective { n: ps.len(), ..obj };
    let j0 = obj.j(&State { pos: Vec3::ZERO, vel: obj.v0 });
    eprintln!("# {f} tick {t} current pitch {:.9}  trig {}", ps[t], trig_mode());
    println!("pitch,cos,dJ");
    let mut q = ps.clone();
    for i in 0..n {
        let p = lo + (hi - lo) * i as f64 / (n - 1) as f64;
        q[t] = p as f32 as f64;
        let c = Mth::cos((p as f32) * (std::f64::consts::PI / 180.0) as f32);
        println!("{:.9},{:.6e},{:.6}", q[t], c, obj.eval(&q) - j0);
    }
}
