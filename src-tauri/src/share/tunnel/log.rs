// ABOUTME: Append-only tee for tunnel-binary stderr/stdout to ~/.glitch-spot/tunnel.log.
// ABOUTME: Best-effort: lookup/create/write errors are silently dropped — never crash the share path.

use std::io::Write;
use std::path::PathBuf;

pub fn log_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".glitch-spot").join("tunnel.log"))
}

pub fn append(provider: &str, line: &str) {
    let Some(path) = log_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "[{provider}] {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "mutates process-wide HOME env var; run via `cargo test -- --ignored`"]
    fn append_creates_and_appends_when_home_is_set() {
        let temp = tempfile::TempDir::new().unwrap();
        // SAFETY: no other threads in this test process are reading HOME at this point.
        // The test is marked #[ignore] so it only runs in an isolated invocation.
        unsafe { std::env::set_var("HOME", temp.path()) };
        append("test-provider", "hello world");
        append("test-provider", "second line");
        let contents =
            std::fs::read_to_string(temp.path().join(".glitch-spot").join("tunnel.log")).unwrap();
        assert!(contents.contains("[test-provider] hello world"));
        assert!(contents.contains("[test-provider] second line"));
    }
}
