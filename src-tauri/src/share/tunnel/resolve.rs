// ABOUTME: Resolves a tunnel binary to an absolute path so GUI-launched builds
// ABOUTME: can find Homebrew-installed tools the bare launchd PATH omits.

use std::path::{Path, PathBuf};

/// Resolve `name` to an absolute executable path, searching (in order):
///   1. the inherited `PATH` — covers dev launches and anything already on it,
///   2. the login shell's `PATH` — recovers mise/asdf/nix/brew setups that only
///      live in `~/.zshrc` and friends, and
///   3. standard macOS install locations as a last resort.
///
/// macOS apps launched from Finder/Dock inherit only the bare launchd PATH
/// (`/usr/bin:/bin:/usr/sbin:/sbin`), so a bare-name `Command::new` can't find
/// Homebrew-installed tools like cloudflared or ngrok. Returns the first match,
/// or `None` if the binary is nowhere to be found.
pub fn resolve_executable(name: &str) -> Option<PathBuf> {
    find_in_dirs(name, &candidate_dirs())
}

/// Off-runtime wrapper for [`resolve_executable`]: the login-shell fallback
/// spawns a subprocess, so resolution runs on a blocking thread to keep the
/// async runtime free.
pub async fn resolve_executable_async(name: &'static str) -> Option<PathBuf> {
    tokio::task::spawn_blocking(move || resolve_executable(name))
        .await
        .ok()
        .flatten()
}

/// Return the first directory in `dirs` that holds an executable named `name`,
/// joined into the full path. Pure: it only stats the filesystem.
pub fn find_in_dirs(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().find_map(|dir| {
        let candidate = dir.join(name);
        is_executable_file(&candidate).then_some(candidate)
    })
}

/// The ordered directory list resolution searches: inherited PATH, then the
/// login shell's PATH, then standard install locations.
fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = inherited_path_dirs();
    dirs.extend(login_shell_path_dirs());
    dirs.extend(standard_install_dirs());
    dirs
}

fn inherited_path_dirs() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default()
}

/// Recover the user's interactive `PATH` by asking their login shell for it.
/// Mirrors the approach the `fix-path` Electron package uses. Returns an empty
/// list (never errors) if `$SHELL` is unset, the shell fails, or it prints
/// nothing usable.
fn login_shell_path_dirs() -> Vec<PathBuf> {
    let shell = match std::env::var("SHELL") {
        Ok(s) if !s.is_empty() => s,
        _ => return Vec::new(),
    };
    let output = std::process::Command::new(&shell)
        .args(["-ilc", "echo $PATH"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // Interactive rc files may print banners before our echo, so take
            // the last non-empty line as the PATH.
            stdout
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(|line| std::env::split_paths(line.trim()).collect())
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn standard_install_dirs() -> Vec<PathBuf> {
    [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    // metadata() follows symlinks, so Homebrew's symlinked binaries resolve.
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[cfg(unix)]
    fn write_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, b"#!/bin/sh\n").unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    #[test]
    fn finds_executable_in_listed_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        write_executable(&dir.path().join("faketool"));
        let found = find_in_dirs("faketool", &[dir.path().to_path_buf()]);
        assert_eq!(found, Some(dir.path().join("faketool")));
    }

    #[test]
    fn returns_none_when_binary_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        assert_eq!(find_in_dirs("faketool", &[dir.path().to_path_buf()]), None);
    }

    #[test]
    fn skips_missing_dirs_and_returns_first_match() {
        let first = tempfile::TempDir::new().unwrap();
        let second = tempfile::TempDir::new().unwrap();
        write_executable(&second.path().join("faketool"));
        let dirs = vec![
            PathBuf::from("/no/such/dir"),
            first.path().to_path_buf(), // exists but lacks the binary
            second.path().to_path_buf(),
        ];
        assert_eq!(
            find_in_dirs("faketool", &dirs),
            Some(second.path().join("faketool"))
        );
    }

    #[test]
    fn earlier_dir_wins_over_later() {
        let first = tempfile::TempDir::new().unwrap();
        let second = tempfile::TempDir::new().unwrap();
        write_executable(&first.path().join("faketool"));
        write_executable(&second.path().join("faketool"));
        let dirs = vec![first.path().to_path_buf(), second.path().to_path_buf()];
        assert_eq!(
            find_in_dirs("faketool", &dirs),
            Some(first.path().join("faketool"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn ignores_non_executable_file() {
        let dir = tempfile::TempDir::new().unwrap();
        fs::write(dir.path().join("faketool"), b"not executable").unwrap();
        assert_eq!(find_in_dirs("faketool", &[dir.path().to_path_buf()]), None);
    }
}
