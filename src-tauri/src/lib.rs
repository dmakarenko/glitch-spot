// ABOUTME: Entry point for the Glitch Spot Tauri 2 application library.
// ABOUTME: Builds and runs the Tauri app with an empty command handler set.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
