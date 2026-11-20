#!/bin/sh
# Cargo's runner on macOS. An ad-hoc signed debug build is a new app to the
# Keychain after every rebuild, so each one asks for the login password again.
# Zephium's own dev executables are re-signed with one identifier and the
# developer's certificate, which the Keychain keeps trusting across rebuilds
# after one "Always Allow".
set -eu

bin=$1
shift

case $(basename -- "$bin") in
    zephium-desktop) ;;
    *) exec "$bin" "$@" ;;
esac

identifier=net.akaaku.bdeck.dev
bundle="$(dirname -- "$bin")/BDeck Dev.app"
executable="$bundle/Contents/MacOS/zephium-desktop"
plist="$bundle/Contents/Info.plist"
mkdir -p "$bundle/Contents/MacOS"
cp -f "$bin" "$executable"
/usr/bin/otool -X -P "$bin" > "$plist"
# WebKit used the executable name as its data directory before bundling.
/usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string zephium-desktop" \
    -c "Add :CFBundleExecutable string zephium-desktop" \
    -c "Add :CFBundlePackageType string APPL" \
    -c "Delete :CFBundleURLTypes" "$plist"

identity=${APPLE_SIGNING_IDENTITY:-}
if [ -z "$identity" ]; then
    identity=$(/usr/bin/security find-identity -v -p codesigning 2>/dev/null |
        sed -n 's/.*"\(Apple Development:[^"]*\)".*/\1/p' | head -n 1)
fi
/usr/bin/codesign --force --sign "${identity:--}" --identifier "$identifier" --timestamp=none "$bundle"

exec "$executable" "$@"
