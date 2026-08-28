FROM rust:1.93-slim AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy manifests first to maximize dependency layer reuse.
COPY morningstar_model/ morningstar_model/
COPY morningstar_rt/Cargo.toml morningstar_rt/Cargo.toml
COPY morningstar_rt/Cargo.lock morningstar_rt/Cargo.lock
COPY morningstar_parser/Cargo.toml morningstar_parser/Cargo.toml
COPY morningstar_parser/Cargo.lock morningstar_parser/Cargo.lock

# Create placeholder sources so Cargo can compile dependencies before app code changes.
RUN mkdir -p morningstar_rt/src morningstar_parser/src \
    && printf 'fn main() {}\n' > morningstar_rt/src/main.rs \
    && printf 'fn main() {}\n' > morningstar_parser/src/main.rs

RUN cargo build --release --manifest-path morningstar_rt/Cargo.toml \
    && cargo build --release --manifest-path morningstar_parser/Cargo.toml

RUN rm -rf morningstar_rt/src morningstar_parser/src

COPY morningstar_rt/src/ morningstar_rt/src/
COPY morningstar_parser/src/ morningstar_parser/src/
COPY morningstar_fe/index.html morningstar_fe/index.html

RUN touch morningstar_rt/src/main.rs morningstar_parser/src/main.rs \
    && cargo build --release --manifest-path morningstar_rt/Cargo.toml \
    && cargo build --release --manifest-path morningstar_parser/Cargo.toml

FROM debian:trixie-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libc6 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/morningstar_rt/target/release/morningstar_rt /usr/local/bin/morningstar_rt
COPY --from=builder /build/morningstar_parser/target/release/morningstar_parser /usr/local/bin/morningstar_parser

EXPOSE 3000

ENTRYPOINT ["morningstar_rt"]
