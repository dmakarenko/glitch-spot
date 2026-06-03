// ABOUTME: Filesystem persistence for snapshots — writes BuiltSnapshot to
// ABOUTME: ~/.glitch-spot/snapshots/{id}/ and sweeps dirs older than 24h.

use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::snapshot::build::BuiltSnapshot;
use crate::snapshot::{annotated_png_filename, clean_png_filename, snapshot_json_filename};

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

/// Writes the three files (`{id}_clean.png`, `{id}_annotated.png`, `{id}.json`)
/// to `{base}/{built.snapshot.id}/`. If any per-file write fails, attempts
/// to remove the `{id}/` directory so callers never observe a partial state,
/// then propagates the original error.
pub fn write(base: &Path, built: &BuiltSnapshot) -> io::Result<()> {
    let id = &built.snapshot.id;
    let dir = base.join(id);
    std::fs::create_dir_all(&dir)?;

    let attempt = || -> io::Result<()> {
        std::fs::write(dir.join(clean_png_filename(id)), &built.clean_bytes)?;
        std::fs::write(dir.join(annotated_png_filename(id)), &built.annotated_bytes)?;
        let json = serde_json::to_vec_pretty(&built.snapshot)
            .map_err(io::Error::other)?;
        std::fs::write(dir.join(snapshot_json_filename(id)), json)?;
        Ok(())
    };

    if let Err(e) = attempt() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(e);
    }
    Ok(())
}

/// Deletes immediate subdirectories of `base` whose mtime is older than
/// `max_age`. Per-entry failures are logged via `eprintln!` and skipped.
/// Returns the number of directories successfully removed.
pub fn cleanup(base: &Path, max_age: Duration) -> io::Result<usize> {
    let mut removed = 0usize;
    let now = std::time::SystemTime::now();

    for entry in std::fs::read_dir(base)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[glitch-spot persist] read_dir entry error: {e}");
                continue;
            }
        };

        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "[glitch-spot persist] metadata error on {}: {e}",
                    path.display()
                );
                continue;
            }
        };

        if !metadata.is_dir() {
            continue;
        }

        let mtime = match metadata.modified() {
            Ok(t) => t,
            Err(e) => {
                eprintln!(
                    "[glitch-spot persist] mtime error on {}: {e}",
                    path.display()
                );
                continue;
            }
        };

        // Future-dated mtime (clock skew) ⇒ treat as age 0, i.e. keep it.
        let age = now.duration_since(mtime).unwrap_or(Duration::ZERO);
        if age <= max_age {
            continue;
        }

        match std::fs::remove_dir_all(&path) {
            Ok(()) => removed += 1,
            Err(e) => eprintln!(
                "[glitch-spot persist] failed to remove {}: {e}",
                path.display()
            ),
        }
    }

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::build::BuiltSnapshot;
    use crate::snapshot::{Annotation, Context, Coords, Images, OsInfo, ScreenInfo, Snapshot};
    use filetime::{set_file_mtime, FileTime};
    use std::time::SystemTime;

    fn fake_built(id: &str, base: &std::path::Path) -> BuiltSnapshot {
        let dir = base.join(id);
        let snapshot = Snapshot {
            id: id.to_string(),
            created_at: "2026-05-25T12:00:00Z".to_string(),
            mode: "frozen".to_string(),
            annotations: vec![Annotation {
                id: "anno_1".to_string(),
                shape: "rect".to_string(),
                coords: Coords { x: 0.0, y: 0.0, w: 10.0, h: 10.0 },
                note: "fix this".to_string(),
                label: "01".to_string(),
            }],
            context: Context {
                screen: ScreenInfo { width: 1440, height: 900, scale_factor: 2.0, display_index: 0 },
                os: OsInfo { platform: "macos".to_string(), version: "26.0".to_string() },
                window: None,
            },
            images: Images {
                clean: dir.join(clean_png_filename(id)).to_string_lossy().into_owned(),
                annotated: Some(dir.join(annotated_png_filename(id)).to_string_lossy().into_owned()),
            },
            agent_prompt: crate::snapshot::default_agent_prompt(),
        };
        BuiltSnapshot {
            snapshot,
            clean_bytes: b"clean-bytes-marker".to_vec(),
            annotated_bytes: b"annotated-bytes-marker".to_vec(),
        }
    }

    #[test]
    fn write_creates_directory_and_three_files() {
        let temp = tempfile::TempDir::new().unwrap();
        let built = fake_built("snap_w01", temp.path());

        write(temp.path(), &built).unwrap();

        let dir = temp.path().join("snap_w01");
        let clean = dir.join(clean_png_filename("snap_w01"));
        let annotated = dir.join(annotated_png_filename("snap_w01"));
        let json = dir.join(snapshot_json_filename("snap_w01"));

        assert!(dir.is_dir(), "{} should be a directory", dir.display());
        assert_eq!(std::fs::read(&clean).unwrap(), b"clean-bytes-marker");
        assert_eq!(std::fs::read(&annotated).unwrap(), b"annotated-bytes-marker");

        let parsed: Snapshot =
            serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
        assert_eq!(parsed.id, "snap_w01");
        assert_eq!(parsed.annotations.len(), 1);
        assert_eq!(parsed.annotations[0].note, "fix this");
    }

    fn write_and_backdate(temp: &tempfile::TempDir, id: &str, age_secs: u64) {
        let built = fake_built(id, temp.path());
        write(temp.path(), &built).unwrap();
        let dir = temp.path().join(id);
        let past = SystemTime::now() - Duration::from_secs(age_secs);
        set_file_mtime(&dir, FileTime::from_system_time(past)).unwrap();
    }

    #[test]
    fn cleanup_removes_dir_older_than_max_age() {
        let temp = tempfile::TempDir::new().unwrap();
        write_and_backdate(&temp, "snap_old", 25 * 60 * 60);

        let removed = cleanup(temp.path(), Duration::from_secs(24 * 60 * 60)).unwrap();

        assert_eq!(removed, 1);
        assert!(!temp.path().join("snap_old").exists());
    }

    #[test]
    fn cleanup_keeps_dir_newer_than_max_age() {
        let temp = tempfile::TempDir::new().unwrap();
        let built = fake_built("snap_fresh", temp.path());
        write(temp.path(), &built).unwrap();

        let removed = cleanup(temp.path(), Duration::from_secs(24 * 60 * 60)).unwrap();

        assert_eq!(removed, 0);
        assert!(temp.path().join("snap_fresh").exists());
    }

    #[test]
    fn cleanup_returns_count_of_removed_dirs() {
        let temp = tempfile::TempDir::new().unwrap();
        write_and_backdate(&temp, "snap_old1", 25 * 60 * 60);
        write_and_backdate(&temp, "snap_old2", 48 * 60 * 60);
        write_and_backdate(&temp, "snap_old3", 30 * 60 * 60);
        let built_fresh = fake_built("snap_fresh", temp.path());
        write(temp.path(), &built_fresh).unwrap();

        let removed = cleanup(temp.path(), Duration::from_secs(24 * 60 * 60)).unwrap();

        assert_eq!(removed, 3);
        assert!(temp.path().join("snap_fresh").exists());
        assert!(!temp.path().join("snap_old1").exists());
        assert!(!temp.path().join("snap_old2").exists());
        assert!(!temp.path().join("snap_old3").exists());
    }

    #[test]
    fn cleanup_tolerates_non_directory_entries() {
        let temp = tempfile::TempDir::new().unwrap();
        // Stray file in the base dir (e.g. .DS_Store).
        std::fs::write(temp.path().join(".DS_Store"), b"junk").unwrap();
        write_and_backdate(&temp, "snap_old", 25 * 60 * 60);

        let removed = cleanup(temp.path(), Duration::from_secs(24 * 60 * 60)).unwrap();

        assert_eq!(removed, 1);
        assert!(
            temp.path().join(".DS_Store").exists(),
            "non-directory entries must be left alone"
        );
        assert!(!temp.path().join("snap_old").exists());
    }

    #[test]
    fn write_rolls_back_on_partial_failure() {
        let temp = tempfile::TempDir::new().unwrap();
        let built = fake_built("snap_rollback", temp.path());
        let snap_dir = temp.path().join("snap_rollback");

        // Pre-create the annotated PNG path as a directory so `fs::write` to
        // that path will fail.
        std::fs::create_dir_all(snap_dir.join(annotated_png_filename("snap_rollback"))).unwrap();

        let err = write(temp.path(), &built).expect_err("write must fail");
        // We don't assert on the exact error kind (platform-dependent), just
        // that an error came back and the snapshot dir is gone afterwards.
        let _ = err;
        assert!(
            !snap_dir.exists(),
            "{} must be removed on partial-write failure",
            snap_dir.display()
        );
    }
}
