# Dockerfile for axga — minimal production image
# Build: docker build -t axga .
# Run:   docker run -e DEEPSEEK_API_KEY=sk-... axga --telegram --key xxx

FROM rust:1.88-alpine AS builder
RUN apk add --no-cache musl-dev openssl-dev pkgconfig
WORKDIR /app
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl -p axga-cli
RUN strip target/x86_64-unknown-linux-musl/release/axga

FROM alpine:3.21
RUN apk add --no-cache ca-certificates python3 py3-pip && pip install memctrl --break-system-packages
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/axga /usr/local/bin/axga
ENTRYPOINT ["axga"]
CMD ["--help"]
