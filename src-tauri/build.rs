// ABOUTME: Tauri 2 build script. Embeds the macOS Swift runtime rpath so the
// ABOUTME: raw release binary launches without DYLD_FALLBACK_LIBRARY_PATH.

use std::process::Command;

fn git_output(args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(o.stdout)
            } else {
                None
            }
        })
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

    // Re-run when HEAD moves so the embedded SHA stays accurate. Without an
    // explicit trigger, cargo fingerprints only the package's source files —
    // git HEAD is invisible to it, so a commit/checkout/rebase would NOT
    // re-run this script and the embedded SHA would go stale (it did: builds
    // kept baking an old commit across rebases). `logs/HEAD` (the reflog) is
    // rewritten on every HEAD movement — commit, checkout, rebase, reset — and
    // `--git-path` resolves it correctly inside linked worktrees too.
    let logs_head = git_output(&["rev-parse", "--git-path", "logs/HEAD"]);
    if logs_head != "unknown" {
        println!("cargo:rerun-if-changed={logs_head}");
    }

    tauri_build::build()
}
