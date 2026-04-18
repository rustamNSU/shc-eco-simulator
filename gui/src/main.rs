#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

slint::include_modules!();

mod app;
mod backend;
mod editor_state;
mod visuals;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}
