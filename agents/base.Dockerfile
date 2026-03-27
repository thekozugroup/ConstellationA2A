# Stage 1: Build constellation SDK Python wheel from Rust source
FROM rust:1.85-bookworm AS builder

# Install Python, maturin build dependencies, and native libs needed by
# matrix-sdk (openssl, sqlite, cmake) and pyo3 (python3-dev).
RUN apt-get update && apt-get install -y \
    python3-dev \
    python3-pip \
    python3-venv \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Install maturin for building PyO3 bindings
RUN pip3 install --break-system-packages maturin[patchelf]

# Copy SDK source code
WORKDIR /build
COPY sdk/ ./sdk/

# Build the Python wheel from the constellation-py crate
WORKDIR /build/sdk/constellation-py
RUN maturin build --release --out /build/wheels

# Stage 2: Slim Python runtime
FROM python:3.11-slim-bookworm

# Install runtime libraries that the compiled wheel links against,
# plus curl for health checks and debugging.
RUN apt-get update && apt-get install -y \
    libssl3 \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install the constellation SDK wheel
COPY --from=builder /build/wheels/*.whl /tmp/wheels/
RUN pip install --no-cache-dir /tmp/wheels/*.whl && rm -rf /tmp/wheels

# Install Python requirements if any
COPY agents/requirements.txt /tmp/requirements.txt
RUN pip install --no-cache-dir -r /tmp/requirements.txt 2>/dev/null || true

# Set up the agent working directory
WORKDIR /app

# Copy shared agent utilities
COPY agents/common.py /app/common.py

# Copy the agent script (overridden per-agent via docker-compose volume mount,
# but provide a default for standalone builds)
COPY agents/coordinator/agent.py /app/agent.py

# Run as non-root
RUN useradd --create-home agent
USER agent

# Graceful shutdown support
STOPSIGNAL SIGTERM

CMD ["python3", "/app/agent.py"]
