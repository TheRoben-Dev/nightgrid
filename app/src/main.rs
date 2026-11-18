// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().skip(1).any(|a| a == "--mcp") {
        std::process::exit(nightgrid_lib::mcp_stdio());
    }
    nightgrid_lib::run()
}
