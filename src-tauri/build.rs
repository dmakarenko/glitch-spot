// ABOUTME: Tauri 2 build script. Embeds the macOS Swift runtime rpath so the
// ABOUTME: raw release binary launches without DYLD_FALLBACK_LIBRARY_PATH.

use std::process::Command;

fn git_output(args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(o.stdout) } else { None })
        .and_then(|b| String::from_utf8(b).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        // libswift_Concurrency.dylib (and friends) ship in /usr/lib/swift on
        // recent macOS but aren't on dyld's default search path for unsigned
        // dev builds. Adding the rpath at link time means the binary finds
        // them without a per-launch DYLD_FALLBACK_LIBRARY_PATH override.
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }

    let branch = git_output(&["rev-parse", "--abbrev-ref", "HEAD"]);
    let sha = git_output(&["rev-parse", "--short", "HEAD"]);
    println!("cargo:rustc-env=GIT_BRANCH={branch}");
    println!("cargo:rustc-env=GIT_SHA={sha}");

    // Rerun whenever HEAD or the branch pointer changes.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs");

    tauri_build::build()
}
