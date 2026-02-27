FROM rust:1.93-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy dependency crates first for layer caching
COPY morningstar_model/ morningstar_model/
COPY morningstar_rt/Cargo.toml morningstar_rt/Cargo.toml
COPY morningstar_parser/Cargo.toml morningstar_parser/Cargo.toml

# Create dummy mains to pre-build dependencies
RUN mkdir morningstar_rt/src && echo 'fn main() {}' > morningstar_rt/src/main.rs
RUN mkdir morningstar_parser/src && echo 'fn main() {}' > morningstar_parser/src/main.rs
RUN cargo build --release --manifest-path morningstar_rt/Cargo.toml \
 && cargo build --release --manifest-path morningstar_parser/Cargo.toml

# Copy actual sources and rebuild
RUN rm -rf morningstar_rt/src morningstar_parser/src
COPY morningstar_rt/src/ morningstar_rt/src/
COPY morningstar_parser/src/ morningstar_parser/src/
COPY morningstar_fe/index.html morningstar_fe/index.html
RUN touch morningstar_rt/src/main.rs morningstar_parser/src/main.rs
RUN cargo build --release --manifest-path morningstar_rt/Cargo.toml \
 && cargo build --release --manifest-path morningstar_parser/Cargo.toml

# --- Runtime ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/morningstar_rt/target/release/morningstar_rt /usr/local/bin/morningstar_rt
COPY --from=builder /build/morningstar_parser/target/release/morningstar_parser /usr/local/bin/morningstar_parser

EXPOSE 3000

ENTRYPOINT ["morningstar_rt"]
