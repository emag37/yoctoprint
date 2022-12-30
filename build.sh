#!/bin/bash

set -e

RELEASE_PKG_DIR="pkg"

if [[ $1 != "" ]] && [[ $1 == "arm" ]]; then
    RELEASE_PKG_DIR="${RELEASE_PKG_DIR}_arm"
    BUILD_DIR="target/arm-unknown-linux-gnueabihf/release"

    . /opt/beaglebone-octoprint/3.1.21/environment-setup-cortexa8hf-neon-poky-linux-gnueabi
    cargo build --target arm-unknown-linux-gnueabihf -Zbuild-std --release
else
    BUILD_DIR="target/debug"

    cargo build
fi

cd ui
npm run build

cd ..
mkdir -p ${RELEASE_PKG_DIR}/ui
cp -r ui/dist ${RELEASE_PKG_DIR}/ui
cp ${BUILD_DIR}/yoctoprint ${RELEASE_PKG_DIR}/
tar -czf ${RELEASE_PKG_DIR}.tar.gz ${RELEASE_PKG_DIR}