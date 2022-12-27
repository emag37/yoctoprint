#!/bin/bash

if [[ $1 != "" -a $1 == "clean" ]]; then
    cargo clean
else
    if [[ $2 != "" -a $2 == "arm" ]]; then
        . /opt/beaglebone-octoprint/3.1.21/environment-setup-cortexa8hf-neon-poky-linux-gnueabi
        cargo build --target arm-unknown-linux-gnueabihf -Zbuild-std
    else
        cargo build
    fi
fi

cd ui
npm run build