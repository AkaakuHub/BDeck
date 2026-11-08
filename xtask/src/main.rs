mod updater_sign;

use std::process::exit;

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.first().map(String::as_str) != Some("sign-windows-updater") {
        eprintln!("usage: cargo xtask sign-windows-updater <installer.exe> <version>");
        exit(2);
    }
    if let Err(error) = updater_sign::run(&arguments[1..]) {
        eprintln!("Windows updater signing failed: {error}");
        exit(1);
    }
}
