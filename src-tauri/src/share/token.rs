// ABOUTME: Per-share access token: generate, persist to disk, verify constant-time.
// ABOUTME: 128-bit OsRng → base64url, 22 chars; stored at {snapshot_dir}/share-token (0600).

use std::io;
use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use subtle::ConstantTimeEq;

const TOKEN_FILE: &str = "share-token";

pub fn generate() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn verify_ct(provided: &str, stored: &str) -> bool {
    // Reject length mismatches explicitly — ConstantTimeEq panics on differing lengths.
    if provided.len() != stored.len() {
        return false;
    }
    provided.as_bytes().ct_eq(stored.as_bytes()).into()
}

pub fn write_for(snapshot_dir: &Path, token: &str) -> io::Result<()> {
    use std::io::Write;
    let final_path = snapshot_dir.join(TOKEN_FILE);
    let temp_path = snapshot_dir.join(format!(".{TOKEN_FILE}.tmp"));

    // Best-effort cleanup of a stale temp from a prior crashed write.
    // We ignore NotFound; any other error will surface from the next open() anyway.
    let _ = std::fs::remove_file(&temp_path);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp_path)?;
        file.write_all(token.as_bytes())?;
        file.write_all(b"\n")?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&temp_path, format!("{token}\n"))?;
    }

    // Atomic rename — overwrites any existing share-token in one syscall,
    // and the new file inherits the 0o600 mode set at create_new time.
    std::fs::rename(&temp_path, &final_path)
}

pub fn read_for(snapshot_dir: &Path) -> io::Result<Option<String>> {
    let path = snapshot_dir.join(TOKEN_FILE);
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s.trim().to_string())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_returns_22_char_base64url() {
        let t = generate();
        assert_eq!(t.len(), 22, "expected 22 chars, got {}", t.len());
        assert!(
            t.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "token contains non-base64url char: {t}"
        );
    }

    #[test]
    fn generate_returns_different_values() {
        let a = generate();
        let b = generate();
        assert_ne!(
            a, b,
            "two consecutive generate() calls produced the same token"
        );
    }

    #[test]
    fn verify_ct_accepts_matching_token() {
        let t = generate();
        assert!(verify_ct(&t, &t));
    }

    #[test]
    fn verify_ct_rejects_mismatch_same_length() {
        let a = generate();
        let mut b = a.clone();
        // flip the first character
        let first = b.remove(0);
        let replacement = if first == 'A' { 'B' } else { 'A' };
        b.insert(0, replacement);
        assert!(!verify_ct(&a, &b));
    }

    #[test]
    fn verify_ct_rejects_different_length() {
        assert!(!verify_ct("abc", "abcd"));
    }

    #[test]
    fn write_and_read_round_trip() {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path();
        let t = generate();
        write_for(dir, &t).unwrap();
        let read = read_for(dir).unwrap().expect("token file should exist");
        assert_eq!(read, t);
        assert!(
            verify_ct(&read, &t),
            "round-tripped token must verify_ct against original"
        );
    }

    #[test]
    fn read_for_returns_none_when_no_token_file() {
        let temp = tempfile::TempDir::new().unwrap();
        assert!(read_for(temp.path()).unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn write_sets_0600_mode() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::TempDir::new().unwrap();
        write_for(temp.path(), "deadbeef").unwrap();
        let meta = std::fs::metadata(temp.path().join("share-token")).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected mode 0o600, got {mode:o}");
    }
}
