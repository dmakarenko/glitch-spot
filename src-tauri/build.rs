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
    let mut sha = git_output(&["rev-parse", "--short", "HEAD"]);
    // `git status --porcelain` prints one line per modified/staged/untracked
    // entry and nothing on a clean tree. Flag dirty builds so we can tell two
    // builds of the "same" commit apart in the debug menu row. We need the
    // success-with-empty-output case distinguished from a git invocation
    // failure, which `git_output` collapses to "unknown" — hence the inline
    // Command rather than reusing the helper here.
    let is_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    if is_dirty {
        sha.push_str(" (dirty)");
    }
    println!("cargo:rustc-env=GIT_BRANCH={branch}");
    println!("cargo:rustc-env=GIT_SHA={sha}");

    // System `date` honours the developer's TZ + locale, which is what we want
    // in the debug menu row — no extra crate, no UTC translation in the head.
    let built_at = Command::new("date")
        .args(["+%Y-%m-%d %H:%M:%S %Z"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(o.stdout) } else { None })
        .and_then(|b| String::from_utf8(b).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BUILT_AT={built_at}");

    // No `rerun-if-changed` lines on purpose: we want BUILT_AT (and the git
    // values) to refresh on *every* rebuild, including pure src-edit cycles
    // that don't touch git. With no explicit triggers, cargo falls back to
    // scanning the package source tree — which already changes whenever a
    // rebuild is meaningful, so build.rs reruns exactly when we want it to.

    tauri_build::build()
}
