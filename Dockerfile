# # Install cmake
# FROM debian:stable-slim AS cmake
# RUN apt-get update && apt-get install -y cmake

# 1. This tells docker to use the Rust official image
# FROM rust:latest

# # COPY --from=cmake /usr/bin/cmake /usr/bin/cmake

# # 2. Copy the files in your machine to the Docker image
# COPY ./ ./

# # Build your program for release
# RUN cargo build --release

# EXPOSE 3000

# # Run the binary
# CMD ["./target/release/mqtt-rs"]


FROM lukemathwalker/cargo-chef:latest-rust-1-alpine3.19 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
COPY --from=planner /app/Rocket.toml Rocket.toml
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin mqtt-rs

# We do not need the Rust toolchain to run the binary!
FROM debian:bullseye-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/mqtt-rs /usr/local/bin
COPY --from=builder /app/Rocket.toml Rocket.toml
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/mqtt-rs"]
