FROM rust:latest

RUN rustup target add armv7-unknown-linux-gnueabihf

RUN dpkg --add-architecture armhf && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        gcc-arm-linux-gnueabihf \
        libc6-dev:armhf \
        libudev-dev:armhf \
        pkg-config \
        jq \
        && \
    rm -rf /var/lib/apt/lists/*

RUN useradd -m builder
USER builder
WORKDIR /home/builder

RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.4/install.sh | bash

ENV NVM_DIR="/home/builder/.nvm"
ENV NODE_VERSION=24

RUN bash -c "source $NVM_DIR/nvm.sh && nvm install $NODE_VERSION"


ENTRYPOINT ["bash", "-c", "source $NVM_DIR/nvm.sh && exec \"$@\"", "--"]

CMD ["/bin/bash"]