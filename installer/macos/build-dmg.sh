#!/bin/sh
# Builds FreeWriter.dmg on a Mac: FreeWriter.app for both Apple silicon and
# Intel Macs, in a disk image with a shortcut to Applications to drag it to.
#
#     rustup target add aarch64-apple-darwin x86_64-apple-darwin
#     sh installer/macos/build-dmg.sh
#
# The result is target/macos/FreeWriter.dmg.
set -eu
cd "$(dirname "$0")/../.."

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
out=target/macos
app=$out/FreeWriter.app
rm -rf "$out"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

# Rust's oldest macOS for Apple silicon; the Intel build matches it.
export MACOSX_DEPLOYMENT_TARGET=11.0
for target in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --release --target "$target"
done
lipo -create -output "$app/Contents/MacOS/freewriter" \
    target/aarch64-apple-darwin/release/freewriter \
    target/x86_64-apple-darwin/release/freewriter
sed "s/@VERSION@/$version/g" installer/macos/Info.plist > "$app/Contents/Info.plist"

# The icon in every size macOS asks for, from the 256-pixel one.
iconset=$out/FreeWriter.iconset
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" assets/freewriter.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" assets/freewriter.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/FreeWriter.icns"

# There's no developer certificate, but Apple silicon Macs won't open an app
# with no signature at all, so it gets a signature of its own.
codesign --force --deep --sign - "$app"

stage=$out/dmg
mkdir -p "$stage"
ditto "$app" "$stage/FreeWriter.app"
ln -s /Applications "$stage/Applications"
# hdiutil now and then finds the disk busy on build machines; it's tried again.
for try in 1 2 3 4 5; do
    if hdiutil create -volname FreeWriter -srcfolder "$stage" -ov -format UDZO "$out/FreeWriter.dmg"; then
        exit 0
    fi
    echo "hdiutil failed (try $try), trying again"
    sleep 5
done
exit 1
