# syntax=docker/dockerfile:1
# Target build stages:
#   base     the Rust toolchain on Alpine (musl), plus zig and a macOS SDK for darwin cross builds
#   test     cargo test of the crates this fork changes; TEST_ARGS is appended to the command
#   build    the binary for TARGET (linux-x64 | linux-arm64 | darwin-arm64)
#   export   that binary alone, for `docker build --output`
ARG RUST_VERSION=1.95.0

FROM rust:${RUST_VERSION}-alpine AS base
ARG RUST_VERSION
# build-base and perl: compile the vendored curl and OpenSSL of Linux release builds;
# openssl-* and zlib-*: system OpenSSL for test builds and zlib for every Linux build,
# as static libraries because musl links statically;
# protobuf-dev: protoc for `cargo xtask proto` after editing .proto files;
# zig: the C compiler and linker for the darwin target
RUN apk add --no-cache bash xz build-base perl pkgconf protobuf-dev zig \
      openssl-dev openssl-libs-static zlib-dev zlib-static \
 && rustup component add rustfmt clippy \
 && rustup target add aarch64-apple-darwin wasm32-wasip1 \
 && cargo install --locked cargo-zigbuild@0.23.4
# Apple's SDK provides the macOS frameworks and system libraries to link against.
ARG MACOS_SDK_URL=https://github.com/joseluisq/macosx-sdks/releases/download/15.5/MacOSX15.5.sdk.tar.xz
ARG MACOS_SDK_SHA256=c15cf0f3f17d714d1aa5a642da8e118db53d79429eb015771ba816aa7c6c1cbd
RUN wget -qO /tmp/sdk.tar.xz "$MACOS_SDK_URL" \
 && echo "$MACOS_SDK_SHA256  /tmp/sdk.tar.xz" | sha256sum -c - \
 && mkdir -p /opt/macos-sdk \
 && tar -C /opt/macos-sdk --strip-components=1 -xJf /tmp/sdk.tar.xz \
 && rm /tmp/sdk.tar.xz
# curl-sys links the macOS system libcurl only when `curl-config` reports HTTP/2, as in the real Mac;
# otherwise it compiles curl from source and then wants Xcode's libclang_rt.osx.a.
# This stub gives the darwin cross build the same libcurl a native build gets.
RUN printf '#!/bin/sh\n[ "$1" = --features ] && echo HTTP2\n' >/usr/local/bin/curl-config \
 && chmod +x /usr/local/bin/curl-config
# RUSTUP_TOOLCHAIN pins the image's toolchain instead of the one in src/rust-toolchain.toml,
# so a bump of rust-version in Cargo.toml fails instead of downloading a toolchain at build time.
ENV RUSTUP_TOOLCHAIN=${RUST_VERSION} OPENSSL_STATIC=1 SDKROOT=/opt/macos-sdk
WORKDIR /work

FROM base AS source
COPY tool.env ./
COPY src ./src

FROM source AS test
ARG TARGETARCH
ARG TEST_ARGS=""
# try_write_to_fd_returns_zero_on_stuck_pty races the kernel's tty buffering:
# it fails now and then on GitHub Action, so it is skipped.
RUN --mount=type=cache,id=zellij-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=zellij-target-${TARGETARCH},target=/work/src/target \
    cd src && cargo test --locked --no-default-features \
      -p zellij-utils -p zellij-client -p zellij-server $TEST_ARGS \
      -- --skip try_write_to_fd_returns_zero_on_stuck_pty

FROM source AS build
ARG TARGETARCH
ARG TARGET=linux-x64
# --no-default-features: no web server, and plugins from the checked-in assets
# (see zellij-utils/Cargo.toml).
# Linux builds compile curl and OpenSSL in for a static binary, whereas
# the macOS build links the system curl.
RUN --mount=type=cache,id=zellij-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=zellij-target-${TARGETARCH},target=/work/src/target \
    cd src && case "$TARGET" in \
      linux-x64) triple=x86_64-unknown-linux-musl; arch=x86_64 ;; \
      linux-arm64) triple=aarch64-unknown-linux-musl; arch=aarch64 ;; \
      darwin-arm64) triple=aarch64-apple-darwin; arch= ;; \
      *) echo "unknown TARGET '$TARGET'" >&2; exit 1 ;; \
    esac \
 && if [ -n "$arch" ] && [ "$(uname -m)" != "$arch" ]; then \
      echo "$TARGET must be built in a $arch container (docker build --platform)" >&2; exit 1; \
    fi \
 && if [ "$TARGET" = darwin-arm64 ]; then \
      cargo zigbuild --release --locked --target "$triple" --no-default-features; \
    else \
      cargo build --release --locked --target "$triple" \
        --no-default-features --features vendored_curl; \
    fi \
 && mkdir -p /out && cp "target/$triple/release/zellij" "/out/zellij-$TARGET"

FROM scratch AS export
COPY --from=build /out/ /
