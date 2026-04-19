# syntax=docker/dockerfile:1.7

# Сборка бинарника telemt-admin (без systemd внутри образа).
FROM docker.io/library/rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    <<'EOF'
set -eux
cargo build --release
strip target/release/telemt-admin
EOF

FROM docker.io/library/debian:bookworm-slim
RUN <<'EOF'
set -eux
apt-get update
apt-get install -y --no-install-recommends ca-certificates
rm -rf /var/lib/apt/lists/*
install -d -m 0755 /etc/telemt-admin
install -d -m 0755 /var/lib/telemt-admin
EOF
COPY --from=builder /app/target/release/telemt-admin /usr/local/bin/telemt-admin
COPY deploy/docker/telemt-admin.docker.toml.example /usr/share/doc/telemt-admin/docker-default.toml.example

ENV RUST_LOG=info
VOLUME ["/etc/telemt-admin", "/var/lib/telemt-admin"]
ENTRYPOINT ["/usr/local/bin/telemt-admin"]
CMD ["--config", "/etc/telemt-admin/telemt-admin.toml"]
