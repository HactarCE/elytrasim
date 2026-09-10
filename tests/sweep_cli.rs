use elytrasim::opt::Profile;
use std::process::Command;

#[test]
fn flick_at_cli_polish_keeps_the_first_crossing_at_that_tick() {
    let output = Command::new(env!("CARGO_BIN_EXE_sweep"))
        .args(["polish", "--n", "24", "--passes", "1", "--tol", "0",
               "--flick-at", "12", "--flick-pitch", "-80"])
        .output().expect("run sweep polish");
    assert!(output.status.success(), "sweep failed: {}",
            String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).expect("profile is UTF-8");
    let profile = Profile::parse(&text).expect("CLI output is a profile");
    assert_eq!(profile.rough.flick_at, Some(12));
    assert_eq!(profile.rough.flick_pitch, -80.0);
    assert_eq!(profile.pitches.iter().position(|&p| p <= -80.0), Some(12));
    assert_eq!(profile.rough.violation(&profile.pitches), 0.0);
}
