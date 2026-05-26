// ABOUTME: TunnelManager state machine: owns the spawned tunnel binary, the public URL,
// ABOUTME: and the share-latest entry point. Provider adapters live in sibling files.

pub mod provider;
pub mod cloudflared;
pub mod ngrok;
pub mod tailscale;

// `mock` is used by both the inline unit tests in this module AND by the
// integration test in `tests/share_e2e.rs`. Integration tests cannot see
// `#[cfg(test)]` items, so it has to be compiled unconditionally.
pub mod mock;
