#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CONDUIT_DIR="$PROJECT_DIR/conduit"
DATA_DIR="$PROJECT_DIR/data/conduit"
SECRET_FILE="$PROJECT_DIR/.registration_secret"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# Check prerequisites
check_prerequisites() {
    local missing=0

    if ! command -v docker &>/dev/null; then
        log_error "Docker is not installed. Please install Docker first."
        missing=1
    fi

    if ! docker compose version &>/dev/null 2>&1; then
        if ! command -v docker-compose &>/dev/null; then
            log_error "Docker Compose is not installed. Please install Docker Compose first."
            missing=1
        fi
    fi

    if [ "$missing" -eq 1 ]; then
        exit 1
    fi

    log_info "Prerequisites check passed."
}

# Create necessary directories
create_directories() {
    log_info "Creating data directories..."
    mkdir -p "$DATA_DIR"
    log_info "Data directory ready: $DATA_DIR"
}

# Generate a random registration shared secret if not already set
generate_secret() {
    if [ -f "$SECRET_FILE" ]; then
        log_info "Registration secret already exists."
        REGISTRATION_SECRET="$(cat "$SECRET_FILE")"
    else
        log_info "Generating new registration shared secret..."
        REGISTRATION_SECRET="$(openssl rand -hex 32)"
        echo "$REGISTRATION_SECRET" > "$SECRET_FILE"
        chmod 600 "$SECRET_FILE"
        log_info "Secret saved to $SECRET_FILE"
    fi

    # Update the conduit.toml with the generated secret
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s/registration_shared_secret = \"CHANGE_ME_ON_FIRST_RUN\"/registration_shared_secret = \"$REGISTRATION_SECRET\"/" "$CONDUIT_DIR/conduit.toml"
    else
        sed -i "s/registration_shared_secret = \"CHANGE_ME_ON_FIRST_RUN\"/registration_shared_secret = \"$REGISTRATION_SECRET\"/" "$CONDUIT_DIR/conduit.toml"
    fi

    export CONDUIT_REGISTRATION_SHARED_SECRET="$REGISTRATION_SECRET"
    log_info "Registration secret configured."
}

# Build and start Conduit
start_conduit() {
    log_info "Building and starting Conduit..."
    cd "$PROJECT_DIR"

    # Use docker compose (v2) or docker-compose (v1)
    if docker compose version &>/dev/null 2>&1; then
        COMPOSE_CMD="docker compose"
    else
        COMPOSE_CMD="docker-compose"
    fi

    $COMPOSE_CMD up -d --build conduit

    log_info "Conduit container started."
}

# Wait for Conduit to be healthy
wait_for_healthy() {
    log_info "Waiting for Conduit to be healthy..."
    local max_attempts=30
    local attempt=0
    local url="http://localhost:8448/_matrix/client/versions"

    while [ $attempt -lt $max_attempts ]; do
        attempt=$((attempt + 1))

        if curl -sf "$url" &>/dev/null; then
            log_info "Conduit is healthy and responding!"
            return 0
        fi

        echo -n "."
        sleep 2
    done

    echo ""
    log_error "Conduit did not become healthy within $((max_attempts * 2)) seconds."
    log_error "Check logs with: docker compose logs conduit"
    exit 1
}

# Print status
print_status() {
    echo ""
    echo "========================================="
    echo "  Constellation Conduit Server Ready"
    echo "========================================="
    echo ""
    echo "  Server:   http://localhost:8448"
    echo "  Domain:   constellation.local"
    echo "  API:      http://localhost:8448/_matrix/client/versions"
    echo ""
    echo "  Registration secret stored in: $SECRET_FILE"
    echo ""
    echo "  Next steps:"
    echo "    1. Register agent accounts:"
    echo "       ./scripts/register-agents.sh"
    echo ""
    echo "    2. Check health:"
    echo "       ./scripts/health-check.sh"
    echo ""
    echo "========================================="
}

# Main
main() {
    log_info "Setting up Constellation Conduit server..."
    echo ""

    check_prerequisites
    create_directories
    generate_secret
    start_conduit
    wait_for_healthy
    print_status
}

main "$@"
