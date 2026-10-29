#!/bin/sh
# Builds the Windows and Linux beta (crates/desktop) for a release, in Docker:
#   dist/desktop/figghost-<version>-windows-x86_64.zip      (MinGW)
#   dist/desktop/figghost-<version>-linux-x86_64.tar.gz     (Ubuntu 22.04, glibc 2.35)
#   dist/desktop/figghost-<version>-linux-aarch64.tar.gz
# Then: gh release upload v<version> dist/desktop/*
#
#   scripts/package-desktop.sh
set -eu
cd "$(dirname "$0")/.."
VERSION=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
OUT=dist/desktop
rm -rf "$OUT"
mkdir -p "$OUT"

# Linux on Ubuntu 22.04, so that the program runs on every distribution with that glibc or a newer one.
LINUX_IMAGE=figghost-desktop-linux
for arch in amd64 arm64; do
  docker build -q --platform "linux/$arch" -t "$LINUX_IMAGE-$arch" - >/dev/null <<'EOF'
FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive PATH=/root/.cargo/bin:$PATH
RUN apt-get update && apt-get install -y --no-install-recommends \
      curl ca-certificates build-essential pkg-config \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev libxdo-dev libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/* \
    && curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
EOF
  docker run --rm --platform "linux/$arch" \
    -v "$PWD":/src:ro -v "figghost-desktop-target-$arch":/target -v "figghost-desktop-cargo-$arch":/root/.cargo/registry \
    -v "$PWD/$OUT":/out -e CARGO_TARGET_DIR=/target -w /src "$LINUX_IMAGE-$arch" sh -euc '
      cargo build -q --release --locked -p figghost-desktop
      name=figghost-'"$VERSION"'-linux-$(uname -m)
      mkdir -p /tmp/$name && cp /target/release/figghost LICENSE /tmp/$name/
      tar -C /tmp -czf /out/$name.tar.gz $name
    '
done

# Windows with MinGW, from the image of scripts/check-desktop-linux.sh.
WINDOWS_IMAGE=figghost-desktop-check
docker build -q -t "$WINDOWS_IMAGE" - >/dev/null <<'EOF'
FROM rust:1-trixie
RUN apt-get update && apt-get install -y --no-install-recommends \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev libxdo-dev libsqlite3-dev \
      xvfb xclip xdotool imagemagick dbus-x11 fonts-noto-core mingw-w64 zip \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add x86_64-pc-windows-gnu
EOF
docker run --rm \
  -v "$PWD":/src:ro -v figghost-desktop-target:/target -v figghost-desktop-cargo:/usr/local/cargo/registry \
  -v "$PWD/$OUT":/out -e CARGO_TARGET_DIR=/target -w /src "$WINDOWS_IMAGE" sh -euc '
    cargo build -q --release --locked -p figghost-desktop --target x86_64-pc-windows-gnu
    echo "DLLs the program loads:"; x86_64-w64-mingw32-objdump -p /target/x86_64-pc-windows-gnu/release/figghost.exe | sed -n "s/.*DLL Name: //p" | sort | tr "\n" " "; echo
    name=figghost-'"$VERSION"'-windows-x86_64
    mkdir -p /tmp/$name && cp /target/x86_64-pc-windows-gnu/release/figghost.exe LICENSE /tmp/$name/
    # The GNU build links WebView2Loader.dll dynamically, so it has to sit next to figghost.exe.
    loader=$(find /target/x86_64-pc-windows-gnu/release/build -path "*/out/x64/WebView2Loader.dll" | head -n 1)
    [ -n "$loader" ] || { echo "WebView2Loader.dll not found" >&2; exit 1; }
    cp "$loader" /tmp/$name/
    cd /tmp && zip -qr /out/$name.zip $name
  '
ls -l "$OUT"
