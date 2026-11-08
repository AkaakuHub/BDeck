#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    zephium_desktop_lib::run();
    std::process::ExitCode::SUCCESS
}
