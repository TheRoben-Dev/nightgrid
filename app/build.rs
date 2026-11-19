use std::process::Command;

fn main() {
    // Commit time of the source this app (and its bundled interface) is built
    // from: an interface patch is only used when it was built from a later
    // commit (D-185). 0 outside a git checkout.
    let built = Command::new("git")
        .args(["log", "-1", "--format=%ct"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);
    println!("cargo:rustc-env=NIGHTGRID_UI_BUILT={built}");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    tauri_build::build()
}
