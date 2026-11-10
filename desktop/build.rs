use std::{env, error::Error, fs, io, path::Path};

use serde_json::Value;
use tauri_utils::platform::Target;

const MAIN_LABEL: &str = "main";
const LINUX_APP_ID: &str = "net.akaaku.bdeck";
const LINUX_DESKTOP_TEMPLATE: &str = "linux/bdeck.desktop.hbs";
const PACKAGE_LICENSE: &str = "MPL-2.0";
const LEGAL_RESOURCES: [(&str, &str); 1] = [("../LICENSE", "licenses/BDeck-MPL-2.0.txt")];

fn main() {
    record_source_revision();
    validate_privileged_window_ownership().expect("Tauri window configuration is invalid");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        tauri_build::try_build(
            tauri_build::Attributes::new()
                .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest()),
        )
        .expect("Tauri Windows build configuration");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
    } else {
        tauri_build::build();
    }
}

fn record_source_revision() {
    let git = |args: &[&str]| std::process::Command::new("git").args(args).output().ok();
    let revision = git(&["rev-parse", "HEAD"])
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|revision| revision.trim().to_owned())
        .filter(|revision| {
            revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .unwrap_or_else(|| "unknown".into());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty());
    println!(
        "cargo:rustc-env=ZEPHIUM_BUILD_REVISION={revision}{}",
        match dirty {
            Some(true) => "+dirty",
            Some(false) => "",
            None => "+state-unknown",
        }
    );
    for name in ["HEAD", "index"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name])
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
        {
            println!("cargo:rerun-if-changed={}", path.trim());
        }
    }
}

fn validate_privileged_window_ownership() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-env-changed=TAURI_CONFIG");
    let root = env::current_dir()?;
    for name in [
        "tauri.conf.json",
        "tauri.macos.conf.json",
        "tauri.linux.conf.json",
        "tauri.windows.conf.json",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(name).display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        root.join(LINUX_DESKTOP_TEMPLATE).display()
    );
    for (source, _) in LEGAL_RESOURCES {
        println!("cargo:rerun-if-changed={}", root.join(source).display());
    }
    let config_override = match env::var("TAURI_CONFIG") {
        Ok(raw) => Some(serde_json::from_str::<Value>(&raw)?),
        Err(env::VarError::NotPresent) => None,
        Err(error) => return Err(Box::new(error)),
    };
    for target in [Target::MacOS, Target::Linux, Target::Windows] {
        let (mut config, paths) = tauri_utils::config::parse::read_from(target, &root)?;
        for path in paths {
            println!("cargo:rerun-if-changed={}", path.display());
        }
        validate_target_window(target, "repository", &config)?;
        validate_legal_resources("repository", &config, &root)?;
        if matches!(target, Target::Linux) {
            validate_linux_identity("repository", &config, &root)?;
        }
        if let Some(override_config) = &config_override {
            json_patch::merge(&mut config, override_config);
            validate_target_window(target, "effective", &config)?;
            validate_legal_resources("effective", &config, &root)?;
            if matches!(target, Target::Linux)
                && env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux")
            {
                validate_linux_identity("effective", &config, &root)?;
            }
        }
    }
    Ok(())
}

fn validate_legal_resources(source: &str, config: &Value, root: &Path) -> io::Result<()> {
    if config.pointer("/bundle/license").and_then(Value::as_str) != Some(PACKAGE_LICENSE) {
        return Err(io::Error::other(format!(
            "{source} configuration must declare package license `{PACKAGE_LICENSE}`"
        )));
    }
    let resources = config
        .pointer("/bundle/resources")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            io::Error::other(format!("{source} configuration has no legal resources"))
        })?;
    for (path, destination) in LEGAL_RESOURCES {
        if resources.get(path).and_then(Value::as_str) != Some(destination) {
            return Err(io::Error::other(format!(
                "{source} configuration must map legal resource `{path}` to `{destination}`"
            )));
        }
        let metadata = fs::symlink_metadata(root.join(path))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(io::Error::other(format!(
                "legal resource `{path}` must be a regular file"
            )));
        }
    }
    Ok(())
}

fn validate_linux_identity(source: &str, config: &Value, root: &Path) -> io::Result<()> {
    for (pointer, expected) in [
        ("/productName", "BDeck"),
        ("/identifier", LINUX_APP_ID),
        ("/bundle/linux/deb/desktopTemplate", LINUX_DESKTOP_TEMPLATE),
        ("/bundle/linux/rpm/desktopTemplate", LINUX_DESKTOP_TEMPLATE),
    ] {
        let actual = config.pointer(pointer).and_then(Value::as_str);
        if actual != Some(expected) {
            return Err(io::Error::other(format!(
                "Linux {source} configuration must set {pointer} to `{expected}`"
            )));
        }
    }
    if config.pointer("/app/enableGTKAppId") != Some(&Value::Bool(true)) {
        return Err(io::Error::other(format!(
            "Linux {source} configuration must set app.enableGTKAppId=true"
        )));
    }
    validate_linux_desktop_template(&root.join(LINUX_DESKTOP_TEMPLATE))
}

fn validate_linux_desktop_template(path: &Path) -> io::Result<()> {
    let template = fs::read_to_string(path)?;
    for exact in [
        "Exec={{exec}} %U",
        "StartupWMClass=net.akaaku.bdeck",
        "Icon={{icon}}",
        "Name=BDeck",
        "Terminal=false",
        "Type=Application",
    ] {
        if template.lines().filter(|line| *line == exact).count() != 1 {
            return Err(io::Error::other(format!(
                "Linux desktop template must contain exactly one `{exact}` entry"
            )));
        }
    }
    if !template.lines().any(|line| {
        line.strip_prefix("MimeType=").is_some_and(|types| {
            let types: Vec<_> = types.split(';').collect();
            types.contains(&"x-scheme-handler/http") && types.contains(&"x-scheme-handler/https")
        })
    }) {
        return Err(io::Error::other(
            "Linux desktop template must declare the http and https scheme handlers",
        ));
    }
    if template.contains("Name={{name}}") {
        return Err(io::Error::other(
            "Linux desktop template must not expose productName",
        ));
    }
    Ok(())
}

fn validate_target_window(target: Target, source: &str, config: &Value) -> io::Result<()> {
    let windows = config
        .pointer("/app/windows")
        .and_then(Value::as_array)
        .ok_or_else(|| io::Error::other(format!("{target} {source} has no app.windows array")))?;
    if windows.len() != 1 {
        return Err(io::Error::other(format!(
            "{target} {source} must expose exactly one main window"
        )));
    }
    let main = &windows[0];
    if main.get("label").and_then(Value::as_str) != Some(MAIN_LABEL) {
        return Err(io::Error::other(format!(
            "{target} {source} must preserve the `{MAIN_LABEL}` window label"
        )));
    }
    if main.get("create").and_then(Value::as_bool) != Some(false) {
        return Err(io::Error::other(format!(
            "{target} {source} must set app.windows[0].create=false"
        )));
    }
    Ok(())
}
