# Yoctoprint!

[![Rust](https://github.com/emag37/yoctoprint/actions/workflows/rust.yml/badge.svg?branch=develop)](https://github.com/emag37/yoctoprint/actions/workflows/rust.yml)

I've been using [Octoprint](https://octoprint.org/) with my Printrbot Simple Metal, but all I have to run it on is a [Beaglebone Black](https://beagleboard.org/black) (Raspberry Pis are expensive in Canada). 

Yoctoprint is my own minimalist version written in Rust to get that sweet native performance.

## Requirements
- Rust
- NodeJS

## Building
Run the `./build.sh` script. I have it set up for a cross-build with a Yocto SDK with `./build.sh arm` or an x86 build with `./build.sh`.

## Can I Use This?
YJust because you can doesn't mean you should. I have only tested it with my Printrbot Simple Metal running Marlin 2, and it lacks most of the safety/security features that Octoprint has. That being said, feel free to give it a whack, but please don't stray too far from your printer.
