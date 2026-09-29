# syntax=docker/dockerfile:1
FROM rust:1.98-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy real sources; Cargo reuses cached dependencies when application code changes.
COPY morningstar_model/ morningstar_model/
COPY morningstar_rt/Cargo.toml morningstar_rt/Cargo.toml
COPY morningstar_rt/Cargo.lock morningstar_rt/Cargo.lock
COPY morningstar_parser/Cargo.toml morningstar_parser/Cargo.toml
COPY morningstar_parser/Cargo.lock morningstar_parser/Cargo.lock

COPY morningstar_rt/src/ morningstar_rt/src/
COPY morningstar_parser/src/ morningstar_parser/src/
COPY morningstar_fe/index.html morningstar_fe/index.html

# Cache downloads and compiled artifacts across builds, including model changes.
# Export binaries while mounts are active: cache contents aren't stored in the layer.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/morningstar_rt/target,sharing=locked \
    --mount=type=cache,target=/build/morningstar_parser/target,sharing=locked \
    cargo build --locked --release --manifest-path morningstar_rt/Cargo.toml \
 && cargo build --locked --release --manifest-path morningstar_parser/Cargo.toml \
 && install -D morningstar_rt/target/release/morningstar_rt /out/morningstar_rt \
 && install -D morningstar_parser/target/release/morningstar_parser /out/morningstar_parser

# --- Runtime ---
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libc6 && rm -rf /var/lib/apt/lists/*

# The parser writes and refreshes ./tt.ron in the working directory.
RUN groupadd --gid 10001 morningstar \
 && useradd --uid 10001 --gid morningstar --no-create-home --shell /usr/sbin/nologin morningstar \
 && install -d -o morningstar -g morningstar /usr/local/share/morningstar_parser

COPY --from=builder /out/morningstar_rt /usr/local/bin/morningstar_rt
COPY --from=builder /out/morningstar_parser /usr/local/bin/morningstar_parser

EXPOSE 3000

WORKDIR /usr/local/share/morningstar_parser

USER 10001:10001

ENTRYPOINT ["morningstar_rt"]
