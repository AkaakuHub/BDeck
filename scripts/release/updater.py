import argparse
import hashlib
import json
import shutil
from pathlib import Path

PLATFORMS = {
    "aarch64-apple-darwin": "darwin-aarch64",
    "x86_64-apple-darwin": "darwin-x86_64",
    "x86_64-pc-windows-msvc": "windows-x86_64",
}


def prepare(target, tag, repository):
    platform = PLATFORMS[target]
    root = Path("target") / target / "release/bundle"
    pattern = "nsis/*.exe" if platform.startswith("windows") else "macos/*.app.tar.gz"
    archives = list(root.glob(pattern))
    if len(archives) != 1:
        raise ValueError(f"Expected one updater archive for {target}, found {len(archives)} matching {pattern} under {root}")
    archive = archives[0]
    signature = Path(f"{archive}.sig").read_text().strip()
    if not signature:
        raise ValueError("Missing updater signature")
    destination = Path("target/installers")
    destination.mkdir(parents=True, exist_ok=True)
    filename = archive.name if platform.startswith("windows") else f"BDeck_{tag}_{platform}.app.tar.gz"
    shutil.copyfile(archive, destination / filename)
    shutil.copyfile(Path(f"{archive}.sig"), destination / f"{filename}.sig")
    (destination / f"{platform}.json").write_text(json.dumps({
        "version": tag.removeprefix("v"),
        "platform": platform,
        "signature": signature,
        "url": f"https://github.com/{repository}/releases/download/{tag}/{filename}",
    }))


def manifest(directory, tag):
    root = Path(directory)
    platforms = {}
    metadata = sorted(root.glob("*.json"))
    if len(metadata) != len(PLATFORMS):
        raise ValueError("Expected metadata from all three platforms")
    for file in metadata:
        entry = json.loads(file.read_text())
        platform = entry.pop("platform")
        version = entry.pop("version")
        if version != tag.removeprefix("v") or platform not in PLATFORMS.values() or platform in platforms:
            raise ValueError("Invalid updater version or platform")
        archive = root / entry["url"].rsplit("/", 1)[1]
        if not archive.is_file() or Path(f"{archive}.sig").read_text().strip() != entry["signature"]:
            raise ValueError("Updater archive and signature must be present")
        platforms[platform] = entry
    if len(list(root.glob("*.dmg"))) != 2 or len(list(root.glob("*.exe"))) != 1:
        raise ValueError("Expected two macOS DMGs and one Windows installer")
    for file in metadata:
        file.unlink()
    (root / "latest.json").write_text(json.dumps({"version": tag.removeprefix("v"), "platforms": platforms}, indent=2) + "\n")
    files = sorted(root.iterdir())
    checksums = [f"{hashlib.file_digest(file.open('rb'), 'sha256').hexdigest()}  {file.name}" for file in files]
    (root / "SHA256SUMS").write_text("\n".join(checksums) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("prepare")
    build.add_argument("--target", choices=PLATFORMS, required=True)
    build.add_argument("--tag", required=True)
    build.add_argument("--repository", required=True)
    feed = commands.add_parser("manifest")
    feed.add_argument("--directory", required=True)
    feed.add_argument("--tag", required=True)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.target, args.tag, args.repository)
    else:
        manifest(args.directory, args.tag)
