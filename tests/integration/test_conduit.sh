#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
SECRET_FILE="$PROJECT_DIR/.registration_secret"
SERVER_URL="http://localhost:8448"
SERVER_NAME="constellation.local"
TEST_USER="test_conduit_$$"
TEST_PASSWORD="$(openssl rand -hex 16)"
ACCESS_TOKEN=""
ROOM_ID=""
COMPOSE_STARTED=false

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log_test()  { echo -e "${CYAN}[TEST]${NC}  $*"; }
log_pass()  { echo -e "${GREEN}[PASS]${NC}  $*"; }
log_fail()  { echo -e "${RED}[FAIL]${NC}  $*"; }
log_info()  { echo -e "${YELLOW}[INFO]${NC}  $*"; }

TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

assert_ok() {
    local name="$1"
    local condition="$2"
    TESTS_RUN=$((TESTS_RUN + 1))
    if eval "$condition"; then
        log_pass "$name"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        log_fail "$name"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# Detect compose command
detect_compose() {
    if docker compose version &>/dev/null 2>&1; then
        echo "docker compose"
    elif command -v docker-compose &>/dev/null; then
        echo "docker-compose"
    else
        echo ""
    fi
}

COMPOSE_CMD="$(detect_compose)"

# Cleanup on exit
cleanup() {
    log_info "Cleaning up..."

    # Deregister / cleanup is best-effort
    if [ -n "$ACCESS_TOKEN" ]; then
        # Leave and forget room if created
        if [ -n "$ROOM_ID" ]; then
            curl -sf -X POST "$SERVER_URL/_matrix/client/v3/rooms/$ROOM_ID/leave" \
                -H "Authorization: Bearer $ACCESS_TOKEN" \
                -H "Content-Type: application/json" -d '{}' &>/dev/null || true
        fi
    fi

    if [ "$COMPOSE_STARTED" = true ] && [ -n "$COMPOSE_CMD" ]; then
        log_info "Stopping Conduit..."
        cd "$PROJECT_DIR" && $COMPOSE_CMD down --timeout 10 &>/dev/null || true
    fi

    log_info "Cleanup complete."
}

trap cleanup EXIT

# Generate HMAC for shared secret registration
generate_hmac() {
    local nonce="$1" username="$2" password="$3" admin="$4" secret="$5"
    local admin_flag
    if [ "$admin" = "true" ]; then admin_flag="admin"; else admin_flag="notadmin"; fi
    local message
    message=$(printf '%s\x00%s\x00%s\x00%s' "$nonce" "$username" "$password" "$admin_flag")
    echo -n "$message" | openssl dgst -sha1 -hmac "$secret" | awk '{print $NF}'
}

# ---- Test Steps ----

start_conduit() {
    log_test "Starting Conduit via docker compose..."

    if [ -z "$COMPOSE_CMD" ]; then
        log_fail "Docker Compose not found"
        exit 1
    fi

    cd "$PROJECT_DIR"
    $COMPOSE_CMD up -d --build conduit
    COMPOSE_STARTED=true
    log_info "Conduit container started."
}

wait_for_health() {
    log_test "Waiting for Conduit health check..."

    local max_attempts=30
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        attempt=$((attempt + 1))
        if curl -sf "$SERVER_URL/_matrix/client/versions" &>/dev/null; then
            assert_ok "Conduit is healthy" "true"
            return 0
        fi
        sleep 2
    done

    assert_ok "Conduit is healthy" "false"
    log_fail "Conduit did not start within $((max_attempts * 2))s"
    exit 1
}

register_test_user() {
    log_test "Registering test user: $TEST_USER"

    if [ ! -f "$SECRET_FILE" ]; then
        log_fail "Registration secret not found at $SECRET_FILE"
        exit 1
    fi

    local shared_secret
    shared_secret="$(cat "$SECRET_FILE")"

    # Get nonce
    local nonce_response
    nonce_response=$(curl -sf "$SERVER_URL/_synapse/admin/v1/register")
    local nonce
    nonce=$(echo "$nonce_response" | jq -r '.nonce')

    assert_ok "Got registration nonce" '[ -n "$nonce" ] && [ "$nonce" != "null" ]'

    # Generate HMAC
    local mac
    mac=$(generate_hmac "$nonce" "$TEST_USER" "$TEST_PASSWORD" "false" "$shared_secret")

    # Register
    local reg_response
    reg_response=$(curl -sf -X POST "$SERVER_URL/_synapse/admin/v1/register" \
        -H "Content-Type: application/json" \
        -d "{
            \"nonce\": \"$nonce\",
            \"username\": \"$TEST_USER\",
            \"password\": \"$TEST_PASSWORD\",
            \"mac\": \"$mac\",
            \"admin\": false
        }")

    local user_id
    user_id=$(echo "$reg_response" | jq -r '.user_id // empty')
    assert_ok "Registered test user" '[ -n "$user_id" ]'
}

login_test_user() {
    log_test "Logging in as $TEST_USER"

    local login_response
    login_response=$(curl -sf -X POST "$SERVER_URL/_matrix/client/v3/login" \
        -H "Content-Type: application/json" \
        -d "{
            \"type\": \"m.login.password\",
            \"identifier\": {
                \"type\": \"m.id.user\",
                \"user\": \"$TEST_USER\"
            },
            \"password\": \"$TEST_PASSWORD\"
        }")

    ACCESS_TOKEN=$(echo "$login_response" | jq -r '.access_token // empty')
    assert_ok "Got access token" '[ -n "$ACCESS_TOKEN" ]'
}

create_test_room() {
    log_test "Creating test room"

    local room_response
    room_response=$(curl -sf -X POST "$SERVER_URL/_matrix/client/v3/createRoom" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $ACCESS_TOKEN" \
        -d "{
            \"name\": \"Test Room $$\",
            \"topic\": \"Integration test room\",
            \"visibility\": \"private\",
            \"preset\": \"private_chat\"
        }")

    ROOM_ID=$(echo "$room_response" | jq -r '.room_id // empty')
    assert_ok "Created test room" '[ -n "$ROOM_ID" ]'
}

send_message() {
    log_test "Sending message to room"

    local txn_id
    txn_id="txn_$(date +%s)_$$"

    local send_response
    send_response=$(curl -sf -X PUT \
        "$SERVER_URL/_matrix/client/v3/rooms/$ROOM_ID/send/m.room.message/$txn_id" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $ACCESS_TOKEN" \
        -d '{
            "msgtype": "m.text",
            "body": "Hello from integration test"
        }')

    local event_id
    event_id=$(echo "$send_response" | jq -r '.event_id // empty')
    assert_ok "Sent message (got event_id)" '[ -n "$event_id" ]'
}

retrieve_and_verify_message() {
    log_test "Retrieving message via sync"

    # Small delay to ensure message is processed
    sleep 1

    local sync_response
    sync_response=$(curl -sf \
        "$SERVER_URL/_matrix/client/v3/sync?filter=%7B%22room%22%3A%7B%22timeline%22%3A%7B%22limit%22%3A5%7D%7D%7D" \
        -H "Authorization: Bearer $ACCESS_TOKEN")

    # Extract message body from the sync response
    local message_body
    message_body=$(echo "$sync_response" | jq -r \
        ".rooms.join[\"$ROOM_ID\"].timeline.events[] | select(.type == \"m.room.message\") | .content.body" \
        2>/dev/null | head -1)

    assert_ok "Retrieved message content matches" '[ "$message_body" = "Hello from integration test" ]'
}

# ---- Main ----

main() {
    echo ""
    echo "========================================="
    echo "  Conduit Integration Test"
    echo "========================================="
    echo ""

    start_conduit
    wait_for_health
    register_test_user
    login_test_user
    create_test_room
    send_message
    retrieve_and_verify_message

    echo ""
    echo "========================================="
    echo "  Results: $TESTS_PASSED/$TESTS_RUN passed"
    echo "========================================="
    echo ""

    if [ "$TESTS_FAILED" -gt 0 ]; then
        log_fail "$TESTS_FAILED test(s) failed"
        exit 1
    else
        log_pass "All tests passed"
        exit 0
    fi
}

main "$@"
