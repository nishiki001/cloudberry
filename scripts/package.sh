#!/usr/bin/env bash
# Builds one release format into dist/ (the release workflow runs exactly this).
#   scripts/package.sh tar | deb | rpm | appimage | flatpak | macos
# Needs: cargo; deb → cargo-deb; rpm → cargo-generate-rpm; appimage → linuxdeploy;
#        flatpak → flatpak-builder; macos → a Mac (lipo, codesign, hdiutil).
# Set NO_BUILD=1 to package an existing target/release/cloudberry.
set -euo pipefail
cd "$(dirname "$0")/.."
ID=io.github.nishiki001.Cloudberry
ARCH=$(uname -m)                      # x86_64 | aarch64 | arm64
[ "$ARCH" = arm64 ] && ARCH=aarch64
FORMAT=${1:?usage: package.sh tar|deb|rpm|appimage|flatpak|macos}
mkdir -p dist

build() { [ -n "${NO_BUILD:-}" ] || cargo build --release --locked; }

# the files every format installs: <root>/bin, share/applications, share/metainfo, hicolor icons
stage() {
  local root=$1
  install -Dm755 target/release/cloudberry "$root/usr/bin/cloudberry"
  install -Dm644 "packaging/$ID.desktop" "$root/usr/share/applications/$ID.desktop"
  install -Dm644 "packaging/$ID.metainfo.xml" "$root/usr/share/metainfo/$ID.metainfo.xml"
  for p in assets/icons/app/png/cloudberry-*.png; do
    n=${p##*-}; n=${n%.png}
    [ "$n" = 1024 ] && continue
    install -Dm644 "$p" "$root/usr/share/icons/hicolor/${n}x${n}/apps/$ID.png"
  done
}

case "$FORMAT" in
  tar)
    build
    T=$(mktemp -d); stage "$T/cloudberry"
    cp README.md LICENSE "$T/cloudberry/"
    tar -C "$T" -czf "dist/cloudberry-linux-$ARCH.tar.gz" cloudberry
    ;;
  deb)
    build
    cargo deb --no-build -o "dist/cloudberry_$([ "$ARCH" = x86_64 ] && echo amd64 || echo arm64).deb"
    ;;
  rpm)
    build
    cargo generate-rpm -o "dist/cloudberry.$ARCH.rpm"
    ;;
  appimage)
    build
    A=$(mktemp -d)/AppDir; stage "$A"
    # run without FUSE (CI containers): extract-and-run
    export APPIMAGE_EXTRACT_AND_RUN=1 ARCH
    OUTPUT="dist/Cloudberry-$ARCH.AppImage" linuxdeploy --appdir "$A" \
      -d "packaging/$ID.desktop" -i assets/icons/app/png/cloudberry-256.png --output appimage
    ;;
  flatpak)
    # the bundle for people who cannot wait for Flathub: dist/Cloudberry-<arch>.flatpak
    FB=flatpak-builder; command -v flatpak-builder >/dev/null || FB="flatpak run org.flatpak.Builder"
    $FB --user --force-clean --disable-rofiles-fuse --repo=dist/flatpak-repo \
      dist/flatpak-build "packaging/flatpak/$ID.yml"
    flatpak build-bundle dist/flatpak-repo "dist/Cloudberry-$ARCH.flatpak" "$ID" \
      --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo
    ;;
  macos)
    # universal Cloudberry.app in a .dmg; ad-hoc signed, real signing when APPLE_CERT is set
    for t in aarch64-apple-darwin x86_64-apple-darwin; do
      rustup target add $t >/dev/null; cargo build --release --locked --target $t
    done
    APP=dist/Cloudberry.app; rm -rf "$APP"; mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
    lipo -create -output "$APP/Contents/MacOS/cloudberry" \
      target/aarch64-apple-darwin/release/cloudberry target/x86_64-apple-darwin/release/cloudberry
    python3 assets/icons/app/make_icons.py >/dev/null
    cp assets/icons/app/cloudberry.icns "$APP/Contents/Resources/Cloudberry.icns"
    V=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
    sed "s/@VERSION@/$V/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
    codesign --force --deep -s - "$APP"
    if [ -n "${APPLE_CERT:-}" ]; then packaging/macos/sign-notarize.sh "$APP"; fi
    D=$(mktemp -d); cp -R "$APP" "$D/"; ln -s /Applications "$D/Applications"
    hdiutil create -volname Cloudberry -srcfolder "$D" -ov -format UDZO dist/Cloudberry-macos-universal.dmg
    ;;
  *) echo "unknown format $FORMAT" >&2; exit 2 ;;
esac
ls -la dist | tail -n +2
