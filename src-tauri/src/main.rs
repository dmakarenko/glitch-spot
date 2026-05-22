// ABOUTME: Binary entry point for the Glitch Spot desktop application.
// ABOUTME: Delegates all setup to the library crate.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    glitch_spot::run();
}
