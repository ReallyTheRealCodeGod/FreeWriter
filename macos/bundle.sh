#!/bin/sh
# Builds Writer.app, a Mac app with its icon, in target/release.
#
#   sh macos/bundle.sh             build it
#   sh macos/bundle.sh --install   build it and put it in ~/Applications
#
# Needs Rust (https://rustup.rs) and the Xcode Command Line Tools.
set -eu
cd "$(dirname "$0")/.."

cargo build --release

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
app=target/release/Writer.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/writer "$app/Contents/MacOS/writer"
cp assets/writer.icns "$app/Contents/Resources/writer.icns"
sed "s/VERSION/$version/g" macos/Info.plist > "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"
# Signed for this Mac only ("ad hoc"), which Apple silicon needs to run it.
codesign --force --sign - "$app"
echo "Built $app"

if [ "${1:-}" = "--install" ]; then
    mkdir -p "$HOME/Applications"
    rm -rf "$HOME/Applications/Writer.app"
    cp -R "$app" "$HOME/Applications/"
    echo "Installed in ~/Applications. Open it from Launchpad or Spotlight."
fi
