//! Exact replay data for the interactive floor-profile figure.

use elytrasim::opt::{Objective, read_pitches};
use elytrasim::sim::{FlightMode, State, TrigMode, Vec3, set_flight_mode, set_trig_mode};

fn arg(flag: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == flag).and_then(|i| a.get(i + 1)).cloned()
}

fn replay(pitches: &[f64]) {
    let obj = Objective { v0: Vec3::ZERO, n: pitches.len(), lambda: 0.0 };
    let states = obj.replay(pitches);
    println!("t,pitch,y,z,vy,vz,te");
    for (t, state) in states.iter().enumerate() {
        let pitch = pitches.get(t).copied().unwrap_or(f64::NAN);
        println!("{t},{pitch},{},{},{},{},{}", state.pos.y, state.pos.z,
                 state.vel.y, state.vel.z, state.total_energy());
    }
}

fn exit_value(states: &[State], depth: f64, distance: bool) -> f64 {
    for k in 1..states.len() {
        let (a, b) = (states[k - 1].pos.y + depth, states[k].pos.y + depth);
        if b < 0.0 {
            let f = a / (a - b);
            return if distance {
                states[k - 1].pos.z + f * (states[k].pos.z - states[k - 1].pos.z)
            } else {
                (k - 1) as f64 + f
            };
        }
    }
    f64::NEG_INFINITY
}

fn best_hold(depth: f64, n: usize, distance: bool) -> f64 {
    let obj = Objective { v0: Vec3::ZERO, n, lambda: 0.0 };
    let score = |pitch: f64| exit_value(&obj.replay(&vec![pitch; n]), depth, distance);
    let (mut best_pitch, mut best_score) = (0.0, f64::NEG_INFINITY);
    for i in -900..=900 {
        let pitch = i as f64 / 10.0;
        let value = score(pitch);
        if value > best_score { best_pitch = pitch; best_score = value; }
    }
    for i in -100..=100 {
        let pitch = best_pitch + i as f64 / 1000.0;
        let value = score(pitch);
        if value > best_score { best_pitch = pitch; best_score = value; }
    }
    best_pitch
}

fn main() {
    set_trig_mode(TrigMode::MthLut);
    set_flight_mode(FlightMode::Reference);
    let n = arg("--n").map_or(450, |x| x.parse().expect("bad --n"));
    let pitches = if let Some(path) = arg("--file") {
        read_pitches(&path)
    } else if std::env::args().any(|x| x == "--best-hold") {
        let depth = arg("--y0").expect("--best-hold needs --y0").parse().expect("bad --y0");
        let distance = arg("--mode").map_or(false, |x| x == "dist");
        let pitch = best_hold(depth, n, distance);
        eprintln!("best_hold={pitch}");
        vec![pitch; n]
    } else if let Some(pitch) = arg("--hold") {
        vec![pitch.parse().expect("bad --hold"); n]
    } else {
        panic!("usage: floor_trace (--file <pitches> | --hold <degrees>) [--n <ticks>]")
    };
    replay(&pitches);
}
