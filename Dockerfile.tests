FROM docker.io/library/rust:1.99

SHELL ["/bin/bash", "-c"]

# The end-to-end tests of `nvm init` run in every shell it supports and skip
# the ones that are missing, so fish, zsh, ksh and dash are installed next to
# bash. Package versions are not pinned (see DL3008 in .hadolint.yaml).
RUN apt-get update \
    && apt-get install -y --no-install-recommends dash fish ksh zsh \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /work

# rust-toolchain.toml pins the channel, components and targets.
COPY rust-toolchain.toml ./
RUN rustup toolchain install

COPY . .
RUN cargo fetch --locked

CMD ["cargo", "test", "--locked", "--offline"]
