#!/bin/bash

set -e

VERSION=$(cargo metadata --format-version 1 --no-deps | jq -r '.packages[0].version')

if [[ $1 != "" ]] && [[ $1 == "arm" ]]; then
    BUILD_DIR="target/armv7-unknown-linux-gnueabihf/release"
    DEB_ARCH="armhf"
    
    export PKG_CONFIG_PATH="/usr/lib/arm-linux-gnueabihf/pkgconfig:/usr/share/pkgconfig"
    export PKG_CONFIG_ALLOW_CROSS=1
   
    cargo build --target armv7-unknown-linux-gnueabihf --release
else
    BUILD_DIR="target/debug"
    DEB_ARCH="amd64"

    cargo build
fi

cd ui
npm install
npm run build
cd ..

if [[ $1 != "" ]] && [[ $1 == "arm" ]]; then

    # Build .deb package on arm
    echo "Building .deb package..."

    cp -v Rocket.toml debian/usr/local/share/yoctoprint/ui/dist/
    cp ${BUILD_DIR}/yoctoprint debian/usr/local/bin/
    cp -r ui/dist/* debian/usr/local/share/yoctoprint/ui/dist/
    dpkg-deb --root-owner-group --build debian
    DEB_FILE="yoctoprint_${VERSION}_${DEB_ARCH}.deb"
    mv -v debian.deb "${DEB_FILE}"
    echo "Debian package created: ${DEB_FILE}"
fi