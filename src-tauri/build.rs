// ABOUTME: Tauri 2 build script. Embeds the macOS Swift runtime rpath so the
// ABOUTME: raw release binary launches without DYLD_FALLBACK_LIBRARY_PATH.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        // libswift_Concurrency.dylib (and friends) ship in /usr/lib/swift on
        // recent macOS but aren't on dyld's default search path for unsigned
        // dev builds. Adding the rpath at link time means the binary finds
        // them without a per-launch DYLD_FALLBACK_LIBRARY_PATH override.
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
    tauri_build::build()
}
