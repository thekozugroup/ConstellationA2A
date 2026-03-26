#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
SECRET_FILE="$PROJECT_DIR/.registration_secret"
SERVER_URL="http://localhost:8448"
SERVER_NAME="constellation.local"

AGENT_A_USER="agent_alpha_$$"
AGENT_B_USER="agent_bravo_$$"
AGENT_A_PASSWORD="$(openssl rand -hex 16)"
AGENT_B_PASSWORD="$(openssl rand -hex 16)"
AGENT_A_TOKEN=""
AGENT_B_TOKEN=""
AGENT_A_USERID=""
AGENT_B_USERID=""
ROOM_ID=""

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

# Cleanup on exit
cleanup() {
    log_info "Cleaning up agent messaging test..."

    for token in "$AGENT_A_TOKEN" "$AGENT_B_TOKEN"; do
        if [ -n "$token" ] && [ -n "$ROOM_ID" ]; then
            curl -sf -X POST "$SERVER_URL/_matrix/client/v3/rooms/$ROOM_ID/leave" \
                -H "Authorization: Bearer $token" \
                -H "Content-Type: application/json" -d '{}' &>/dev/null || true
        fi
    done

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

# Register an agent and return the access token
register_agent() {
    local username="$1"
    local password="$2"
    local shared_secret
    shared_secret="$(cat "$SECRET_FILE")"

    # Get nonce
    local nonce_response
    nonce_response=$(curl -sf "$SERVER_URL/_synapse/admin/v1/register")
    local nonce
    nonce=$(echo "$nonce_response" | jq -r '.nonce')

    # Generate HMAC
    local mac
    mac=$(generate_hmac "$nonce" "$username" "$password" "false" "$shared_secret")

    # Register
    local reg_response
    reg_response=$(curl -sf -X POST "$SERVER_URL/_synapse/admin/v1/register" \
        -H "Content-Type: application/json" \
        -d "{
            \"nonce\": \"$nonce\",
            \"username\": \"$username\",
            \"password\": \"$password\",
            \"mac\": \"$mac\",
            \"admin\": false
        }")

    local user_id
    user_id=$(echo "$reg_response" | jq -r '.user_id // empty')
    local access_token
    access_token=$(echo "$reg_response" | jq -r '.access_token // empty')

    if [ -z "$user_id" ] || [ -z "$access_token" ]; then
        return 1
    fi

    echo "$user_id|$access_token"
}

# Do an initial sync to get a since token
initial_sync() {
    local token="$1"
    local sync_response
    sync_response=$(curl -sf \
        "$SERVER_URL/_matrix/client/v3/sync?filter=%7B%22room%22%3A%7B%22timeline%22%3A%7B%22limit%22%3A0%7D%7D%7D" \
        -H "Authorization: Bearer $token")
    echo "$sync_response" | jq -r '.next_batch // empty'
}

# ---- Test Steps ----

check_server() {
    log_test "Checking Conduit is running..."
    if ! curl -sf "$SERVER_URL/_matrix/client/versions" &>/dev/null; then
        log_fail "Conduit is not running at $SERVER_URL. Start it first."
        exit 1
    fi
    log_info "Conduit is running."

    if [ ! -f "$SECRET_FILE" ]; then
        log_fail "Registration secret not found. Run setup.sh first."
        exit 1
    fi
}

register_agents() {
    log_test "Registering Agent A: $AGENT_A_USER"
    local result_a
    result_a=$(register_agent "$AGENT_A_USER" "$AGENT_A_PASSWORD")
    AGENT_A_USERID=$(echo "$result_a" | cut -d'|' -f1)
    AGENT_A_TOKEN=$(echo "$result_a" | cut -d'|' -f2)
    assert_ok "Agent A registered" '[ -n "$AGENT_A_TOKEN" ]'

    log_test "Registering Agent B: $AGENT_B_USER"
    local result_b
    result_b=$(register_agent "$AGENT_B_USER" "$AGENT_B_PASSWORD")
    AGENT_B_USERID=$(echo "$result_b" | cut -d'|' -f1)
    AGENT_B_TOKEN=$(echo "$result_b" | cut -d'|' -f2)
    assert_ok "Agent B registered" '[ -n "$AGENT_B_TOKEN" ]'
}

create_shared_room() {
    log_test "Agent A creates shared room"

    local room_response
    room_response=$(curl -sf -X POST "$SERVER_URL/_matrix/client/v3/createRoom" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $AGENT_A_TOKEN" \
        -d "{
            \"name\": \"Agent Messaging Test $$\",
            \"topic\": \"Agent-to-agent messaging test\",
            \"visibility\": \"private\",
            \"preset\": \"private_chat\",
            \"invite\": [\"$AGENT_B_USERID\"]
        }")

    ROOM_ID=$(echo "$room_response" | jq -r '.room_id // empty')
    assert_ok "Shared room created" '[ -n "$ROOM_ID" ]'

    # Agent B joins the room
    log_test "Agent B joins the room"
    local join_response
    join_response=$(curl -sf -X POST "$SERVER_URL/_matrix/client/v3/join/$ROOM_ID" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $AGENT_B_TOKEN" \
        -d '{}')

    local joined_room
    joined_room=$(echo "$join_response" | jq -r '.room_id // empty')
    assert_ok "Agent B joined room" '[ -n "$joined_room" ]'
}

test_mention_message() {
    log_test "Agent A sends @-mention message to Agent B"

    # Get a since token for Agent B so we only see new events
    local since_token
    since_token=$(initial_sync "$AGENT_B_TOKEN")

    sleep 1

    # Agent A sends a message mentioning Agent B
    local txn_id="txn_mention_$(date +%s)_$$"
    local mention_body="Hey @${AGENT_B_USER}, please process this task."
    local formatted_body="Hey <a href=\"https://matrix.to/#/${AGENT_B_USERID}\">@${AGENT_B_USER}</a>, please process this task."

    local send_response
    send_response=$(curl -sf -X PUT \
        "$SERVER_URL/_matrix/client/v3/rooms/$ROOM_ID/send/m.room.message/$txn_id" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $AGENT_A_TOKEN" \
        -d "{
            \"msgtype\": \"m.text\",
            \"body\": \"$mention_body\",
            \"format\": \"org.matrix.custom.html\",
            \"formatted_body\": \"$formatted_body\",
            \"m.mentions\": {
                \"user_ids\": [\"$AGENT_B_USERID\"]
            }
        }")

    local event_id
    event_id=$(echo "$send_response" | jq -r '.event_id // empty')
    assert_ok "Mention message sent" '[ -n "$event_id" ]'

    # Agent B syncs to receive the message
    sleep 1

    log_test "Agent B receives mention message via sync"
    local sync_response
    sync_response=$(curl -sf \
        "$SERVER_URL/_matrix/client/v3/sync?since=$since_token&filter=%7B%22room%22%3A%7B%22timeline%22%3A%7B%22limit%22%3A10%7D%7D%7D" \
        -H "Authorization: Bearer $AGENT_B_TOKEN")

    # Check that Agent B received the message
    local received_body
    received_body=$(echo "$sync_response" | jq -r \
        ".rooms.join[\"$ROOM_ID\"].timeline.events[] | select(.type == \"m.room.message\") | .content.body" \
        2>/dev/null | head -1)

    assert_ok "Agent B received the message" '[ "$received_body" = "$mention_body" ]'

    # Verify formatted_body contains the @-mention
    local received_formatted
    received_formatted=$(echo "$sync_response" | jq -r \
        ".rooms.join[\"$ROOM_ID\"].timeline.events[] | select(.type == \"m.room.message\") | .content.formatted_body" \
        2>/dev/null | head -1)

    assert_ok "@-mention present in formatted_body" 'echo "$received_formatted" | grep -q "matrix.to"'
}

test_custom_metadata() {
    log_test "Agent A sends message with custom constellation metadata"

    # Get a fresh since token for Agent B
    local since_token
    since_token=$(initial_sync "$AGENT_B_TOKEN")

    sleep 1

    local txn_id="txn_meta_$(date +%s)_$$"

    local send_response
    send_response=$(curl -sf -X PUT \
        "$SERVER_URL/_matrix/client/v3/rooms/$ROOM_ID/send/m.room.message/$txn_id" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $AGENT_A_TOKEN" \
        -d "{
            \"msgtype\": \"m.text\",
            \"body\": \"Task assignment with metadata\",
            \"io.constellation.metadata\": {
                \"task_id\": \"task-42\",
                \"priority\": \"high\",
                \"agent_role\": \"researcher\",
                \"payload\": {
                    \"query\": \"What is the meaning of life?\",
                    \"max_results\": 10
                }
            }
        }")

    local event_id
    event_id=$(echo "$send_response" | jq -r '.event_id // empty')
    assert_ok "Metadata message sent" '[ -n "$event_id" ]'

    # Agent B syncs to receive the message
    sleep 1

    log_test "Verifying custom metadata is preserved"
    local sync_response
    sync_response=$(curl -sf \
        "$SERVER_URL/_matrix/client/v3/sync?since=$since_token&filter=%7B%22room%22%3A%7B%22timeline%22%3A%7B%22limit%22%3A10%7D%7D%7D" \
        -H "Authorization: Bearer $AGENT_B_TOKEN")

    # Extract the custom metadata
    local task_id
    task_id=$(echo "$sync_response" | jq -r \
        ".rooms.join[\"$ROOM_ID\"].timeline.events[] | select(.type == \"m.room.message\") | .content[\"io.constellation.metadata\"].task_id" \
        2>/dev/null | head -1)

    assert_ok "task_id metadata preserved" '[ "$task_id" = "task-42" ]'

    local priority
    priority=$(echo "$sync_response" | jq -r \
        ".rooms.join[\"$ROOM_ID\"].timeline.events[] | select(.type == \"m.room.message\") | .content[\"io.constellation.metadata\"].priority" \
        2>/dev/null | head -1)

    assert_ok "priority metadata preserved" '[ "$priority" = "high" ]'

    local query
    query=$(echo "$sync_response" | jq -r \
        ".rooms.join[\"$ROOM_ID\"].timeline.events[] | select(.type == \"m.room.message\") | .content[\"io.constellation.metadata\"].payload.query" \
        2>/dev/null | head -1)

    assert_ok "nested payload metadata preserved" '[ "$query" = "What is the meaning of life?" ]'
}

# ---- Main ----

main() {
    echo ""
    echo "========================================="
    echo "  Agent-to-Agent Messaging Test"
    echo "========================================="
    echo ""

    check_server
    register_agents
    create_shared_room
    test_mention_message
    test_custom_metadata

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
