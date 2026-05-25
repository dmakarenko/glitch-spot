// ABOUTME: Filesystem persistence for snapshots — writes BuiltSnapshot to
// ABOUTME: ~/.glitch-spot/snapshots/{id}/ and sweeps dirs older than 24h.

use std::env;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Returns `~/.glitch-spot/snapshots/`, creating it (and `.glitch-spot/`) if absent.
///
/// Resolves `$HOME` directly (macOS-only product). Returns `io::Error::NotFound`
/// equivalent (kind = `NotFound`) if `HOME` is unset.
pub fn snapshots_dir() -> io::Result<PathBuf> {
    let home = env::var("HOME")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    let dir = PathBuf::from(home).join(".glitch-spot").join("snapshots");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
