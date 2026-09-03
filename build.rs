use std::process::Command;

fn main() {
    let out = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output();
    let mut hash = out.ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    // A dirty tree cannot be reconstructed from a hash, so say so rather than imply otherwise.
    if Command::new("git").args(["diff", "--quiet", "HEAD"]).status().map(|s| !s.success()).unwrap_or(false) {
        hash.push_str("-dirty");
    }
    println!("cargo:rustc-env=ELYTRASIM_COMMIT={hash}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}
