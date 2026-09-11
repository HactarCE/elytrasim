use std::process::Command;

fn main() {
    // A source-only export (rsync to a compute cluster, a tarball) has no .git to interrogate,
    // so the hash has to be able to come from the exporter instead. The override is trusted as
    // stated: nothing here can check it against a tree that is not present.
    let hash = match std::env::var("ELYTRASIM_COMMIT_OVERRIDE") {
        Ok(h) if !h.trim().is_empty() => h.trim().to_string(),
        _ => {
            let out = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output();
            let mut hash = out.ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "unknown".into());
            // A dirty tree cannot be reconstructed from a hash, so say so rather than imply otherwise.
            if Command::new("git").args(["diff", "--quiet", "HEAD"]).status().map(|s| !s.success()).unwrap_or(false) {
                hash.push_str("-dirty");
            }
            hash
        }
    };
    println!("cargo:rustc-env=ELYTRASIM_COMMIT={hash}");
    println!("cargo:rerun-if-env-changed=ELYTRASIM_COMMIT_OVERRIDE");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}
