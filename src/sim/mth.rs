//! Emulation of `Mth.java`.
//!
//! Vanilla does not call `Math.sin`. It builds a 65536-entry table of `sin` over one turn and
//! indexes it with a truncating cast, so its trig is a step function with steps of
//! `2*pi/65536 ~ 0.0055 deg`. Reproducing that matters for two reasons: the physics we are
//! measuring is Minecraft's, not libm's; and libm differs between platforms, so a profile
//! optimized on a laptop has to reproduce on a cluster. The snap phase resolves pitch to
//! ~0.01 deg, which is the same order as the quantization -- close enough that guessing is
//! not good enough.
//!
//! Both implementations stay in the binary, selected by [`set_trig_mode`], so the difference is
//! measurable rather than assumed. `Libm` remains the default: it is what every existing number
//! in `README-myopic.md` was measured with.
//!
//! Note that `lift_force` in `entity.rs` genuinely is `Math.cos` on a double in vanilla, not
//! `Mth.cos`, so it is not routed through here and stays platform-dependent either way.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TrigMode {
    /// Rust's `f32::sin`/`f32::cos`. Not what Minecraft does, and not portable across libms.
    #[default]
    Libm,
    /// Vanilla's `Mth.SIN` lookup table.
    MthLut,
}

static MODE: AtomicU8 = AtomicU8::new(0);

/// Select the trig implementation. Call once, before any physics; the sweep sets it from the
/// profile header so a file is always replayed under the physics it was optimized against.
pub fn set_trig_mode(m: TrigMode) {
    MODE.store(m as u8, Ordering::Relaxed);
}

pub fn trig_mode() -> TrigMode {
    match MODE.load(Ordering::Relaxed) {
        0 => TrigMode::Libm,
        _ => TrigMode::MthLut,
    }
}

impl std::str::FromStr for TrigMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "libm" => Ok(TrigMode::Libm),
            "mth_lut" | "mth" | "lut" => Ok(TrigMode::MthLut),
            _ => Err(format!("unknown trig mode {s:?}, want libm or mth_lut")),
        }
    }
}

impl std::fmt::Display for TrigMode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            TrigMode::Libm => "libm",
            TrigMode::MthLut => "mth_lut",
        })
    }
}

/// `Mth.SIN`: `sin(i * 2pi / 65536)` computed in double and rounded to float, exactly as
/// `Mth.java` builds it. 256 KiB, so it is built once and leaked into a `OnceLock`.
pub fn sin_table() -> &'static [f32; 65536] {
    static T: OnceLock<Box<[f32; 65536]>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new([0.0f32; 65536]);
        for (i, v) in t.iter_mut().enumerate() {
            *v = (i as f64 * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32;
        }
        t
    })
}

/// Vanilla's index arithmetic: `SIN[(int)(f * 10430.378F) & 0xFFFF]`.
///
/// Java's float-to-int cast truncates toward zero and saturates, which is what Rust's `as`
/// does too, so the two agree including on negative and out-of-range inputs.
#[inline]
fn lut(x: f32) -> f32 {
    sin_table()[(x as i32 & 0xFFFF) as usize]
}

pub struct Mth;

impl Mth {
    #[inline]
    pub fn sin(x: f32) -> f32 {
        match trig_mode() {
            TrigMode::Libm => x.sin(),
            TrigMode::MthLut => lut(x * 10430.378),
        }
    }

    #[inline]
    pub fn cos(x: f32) -> f32 {
        match trig_mode() {
            TrigMode::Libm => x.cos(),
            TrigMode::MthLut => lut(x * 10430.378 + 16384.0),
        }
    }

    pub fn square(x: f64) -> f64 {
        x * x
    }
}
