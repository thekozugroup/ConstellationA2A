# Researcher-specific Dockerfile (can also use base.Dockerfile via compose)
# Stage 1: Build constellation SDK Python wheel from Rust source
FROM rust:1.77-bookworm AS builder

RUN apt-get update && apt-get install -y \
    python3-dev \
    python3-pip \
    python3-venv \
    && rm -rf /var/lib/apt/lists/*

RUN pip3 install --break-system-packages maturin

WORKDIR /build
COPY sdk/ ./sdk/

WORKDIR /build/sdk/constellation-py
RUN maturin build --release --out /build/wheels

# Stage 2: Slim Python runtime
FROM python:3.11-slim-bookworm

RUN apt-get update && apt-get install -y \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/wheels/*.whl /tmp/wheels/
RUN pip install --no-cache-dir /tmp/wheels/*.whl && rm -rf /tmp/wheels

WORKDIR /app
COPY agents/researcher/agent.py /app/agent.py

RUN useradd --create-home agent
USER agent

STOPSIGNAL SIGTERM
CMD ["python3", "/app/agent.py"]
