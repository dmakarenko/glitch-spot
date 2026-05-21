// ABOUTME: Entry point for the Glitch Spot Tauri 2 application library.
// ABOUTME: Builds and runs the Tauri app with registered Tauri commands.
mod spike_capture;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![spike_capture::spike_capture])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
