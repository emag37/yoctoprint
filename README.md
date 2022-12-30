# Yoctoprint!

I've been using [Octoprint](https://octoprint.org/) with my Printrbot Simple Metal, but all I have to run it on is a [Beaglebone Black](https://beagleboard.org/black) (Raspberry Pis are expensive in Canada). 

Yoctoprint is my own minimalist version written in Rust to get that sweet native performance.

## Requirements
- Rust
- NodeJS

## Building
Run the `./build.sh` script. I have it set up for a cross-build with a Yocto SDK with `./build.sh arm` or an x86 build with `./build.sh`.