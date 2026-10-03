FROM docker.io/library/rust:1.99

SHELL ["/bin/bash", "-c"]

# .cargo/config.toml sets linkers per target: clang + mold on x86_64 Linux and
# aarch64-linux-gnu-gcc on aarch64 Linux, so all of them are installed.
# Package versions are not pinned (see DL3008 in .hadolint.yaml).
RUN apt-get update \
    && apt-get install -y --no-install-recommends clang gcc-aarch64-linux-gnu mold \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /work

# rust-toolchain.toml pins the channel, components and targets.
COPY rust-toolchain.toml ./
RUN rustup toolchain install

COPY . .
RUN cargo fetch --locked

CMD ["cargo", "test", "--locked", "--offline"]
