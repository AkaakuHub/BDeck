use base64::Engine as _;
use minisign::{PublicKeyBox, SecretKeyBox};
use semver::Version;
use std::fs::File;
use std::io::Write;
use std::path::Path;

const TARGET: &str = "windows-x86_64-nsis";

pub(crate) fn run(arguments: &[String]) -> Result<(), &'static str> {
    let [installer, version] = arguments else {
        return Err("usage: cargo xtask sign-windows-updater <installer.exe> <version>");
    };
    let installer = Path::new(installer);
    if installer
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("exe")
    {
        return Err("the Windows updater artifact must be an NSIS .exe");
    }
    let config: serde_json::Value = serde_json::from_slice(
        &std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../desktop/tauri.conf.json"))
            .map_err(|_| "cannot read the embedded updater public key")?,
    )
    .map_err(|_| "invalid application configuration")?;
    let public = config
        .pointer("/plugins/updater/pubkey")
        .and_then(|key| key.as_str())
        .ok_or("the embedded updater public key is missing")?;
    let secret = std::env::var("TAURI_SIGNING_PRIVATE_KEY")
        .map_err(|_| "TAURI_SIGNING_PRIVATE_KEY is required")?;
    let password = std::env::var("TAURI_SIGNING_PRIVATE_KEY_PASSWORD").unwrap_or_default();
    let signature = sign(installer, version, &secret, password, public)?;
    let output = installer.with_extension("exe.sig");
    let mut temporary =
        tempfile::NamedTempFile::new_in(installer.parent().ok_or("invalid artifact path")?)
            .map_err(|_| "cannot stage updater signature")?;
    temporary
        .write_all(signature.as_bytes())
        .map_err(|_| "cannot write updater signature")?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "cannot sync updater signature")?;
    temporary
        .persist(output)
        .map_err(|_| "cannot publish updater signature")?;
    Ok(())
}

fn sign(
    installer: &Path,
    version: &str,
    secret: &str,
    password: String,
    public: &str,
) -> Result<String, &'static str> {
    let parsed = Version::parse(version).map_err(|_| "invalid release version")?;
    if parsed.to_string() != version {
        return Err("the release version must be canonical semver");
    }
    let decode = |value: &str| {
        if value.len() > 8192 {
            return Err("invalid signing key encoding");
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(value)
            .map_err(|_| "invalid signing key encoding")?;
        String::from_utf8(bytes).map_err(|_| "invalid signing key encoding")
    };
    let public = PublicKeyBox::from_string(&decode(public)?)
        .and_then(|key| key.into_public_key())
        .map_err(|_| "invalid updater public key")?;
    let secret = SecretKeyBox::from_string(&decode(secret)?)
        .and_then(|key| {
            key.clone()
                .into_unencrypted_secret_key()
                .or_else(|_| key.into_secret_key(Some(password)))
        })
        .map_err(|_| "cannot unlock updater signing key")?;
    let comment = format!("zephium-updater-v1 version={version} target={TARGET}");
    let signature = minisign::sign(
        Some(&public),
        &secret,
        File::open(installer).map_err(|_| "cannot open updater artifact")?,
        Some(&comment),
        None,
    )
    .map_err(|_| "cannot sign updater artifact")?;
    minisign::verify(
        &public,
        &signature,
        File::open(installer).map_err(|_| "cannot reopen updater artifact")?,
        true,
        false,
        false,
    )
    .map_err(|_| "updater artifact signature verification failed")?;
    Ok(base64::engine::general_purpose::STANDARD.encode(signature.to_string()))
}
