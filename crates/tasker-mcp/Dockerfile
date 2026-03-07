FROM rust:1.83-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .
RUN cargo build --release -p tasker-mcp --all-features

FROM debian:bookworm-slim
COPY --from=builder /build/target/release/tasker-mcp /usr/local/bin/
ENTRYPOINT ["tasker-mcp"]
