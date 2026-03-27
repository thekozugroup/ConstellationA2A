# Stage 1: Build a tiny static healthcheck binary
FROM rust:1.85-bookworm AS healthcheck-builder
RUN rustup target add aarch64-unknown-linux-musl && apt-get update && apt-get install -y musl-tools && rm -rf /var/lib/apt/lists/*
RUN cat > /tmp/healthcheck.rs << 'RUST_EOF'
use std::net::TcpStream;
use std::process::exit;
use std::time::Duration;

fn main() {
    match TcpStream::connect_timeout(
        &"127.0.0.1:6167".parse().unwrap(),
        Duration::from_secs(3),
    ) {
        Ok(_) => exit(0),
        Err(_) => exit(1),
    }
}
RUST_EOF
RUN rustc --target aarch64-unknown-linux-musl -C target-feature=+crt-static -o /tmp/healthcheck /tmp/healthcheck.rs

# Stage 2: Conduit with healthcheck
FROM matrixconduit/matrix-conduit:latest

# Copy static healthcheck binary
COPY --from=healthcheck-builder /tmp/healthcheck /usr/local/bin/healthcheck

# Copy Conduit configuration
COPY conduit.toml /etc/conduit/conduit.toml

# Set environment variable to point to config
ENV CONDUIT_CONFIG="/etc/conduit/conduit.toml"

# Data persistence volume
VOLUME ["/var/lib/conduit"]

# Expose the Conduit port
EXPOSE 6167

# Built-in healthcheck
HEALTHCHECK --interval=5s --timeout=3s --retries=10 --start-period=5s \
    CMD ["/usr/local/bin/healthcheck"]

# The base image already sets the entrypoint to the conduit binary
