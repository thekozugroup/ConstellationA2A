#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SECRET_FILE="$PROJECT_DIR/.registration_secret"
SERVER_URL="http://localhost:8448"
SERVER_NAME="constellation.local"

# Default agent names
DEFAULT_AGENTS=("coordinator" "researcher" "coder")
ROOM_ALIAS="constellation"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# Check prerequisites
check_prerequisites() {
    for cmd in curl jq openssl; do
        if ! command -v "$cmd" &>/dev/null; then
            log_error "$cmd is required but not installed."
            exit 1
        fi
    done

    if [ ! -f "$SECRET_FILE" ]; then
        log_error "Registration secret not found at $SECRET_FILE"
        log_error "Run setup.sh first to generate the secret."
        exit 1
    fi

    SHARED_SECRET="$(cat "$SECRET_FILE")"

    # Check that Conduit is running
    if ! curl -sf "$SERVER_URL/_matrix/client/versions" &>/dev/null; then
        log_error "Conduit is not running at $SERVER_URL"
        log_error "Run setup.sh first to start the server."
        exit 1
    fi
}

# Generate HMAC for registration
# Conduit uses the Synapse-compatible shared secret registration
# HMAC = HMAC-SHA1(shared_secret, nonce + "\x00" + username + "\x00" + password + "\x00" + admin_flag)
generate_hmac() {
    local nonce="$1"
    local username="$2"
    local password="$3"
    local admin="$4"

    local admin_flag
    if [ "$admin" = "true" ]; then
        admin_flag="admin"
    else
        admin_flag="notadmin"
    fi

    local message
    message=$(printf '%s\x00%s\x00%s\x00%s' "$nonce" "$username" "$password" "$admin_flag")

    echo -n "$message" | openssl dgst -sha1 -hmac "$SHARED_SECRET" | awk '{print $NF}'
}

# Register a single agent account
register_agent() {
    local username="$1"
    local password="${2:-$(openssl rand -hex 16)}"
    local admin="${3:-false}"

    log_info "Registering agent: @${username}:${SERVER_NAME}"

    # Step 1: Get a nonce from the registration endpoint
    local nonce_response
    nonce_response=$(curl -sf "$SERVER_URL/_synapse/admin/v1/register" 2>/dev/null || true)

    if [ -z "$nonce_response" ]; then
        log_error "Failed to get registration nonce. Is Conduit running with shared secret registration?"
        return 1
    fi

    local nonce
    nonce=$(echo "$nonce_response" | jq -r '.nonce // empty')

    if [ -z "$nonce" ]; then
        log_error "No nonce in response: $nonce_response"
        return 1
    fi

    # Step 2: Generate HMAC
    local mac
    mac=$(generate_hmac "$nonce" "$username" "$password" "$admin")

    # Step 3: Register the account
    local reg_response
    reg_response=$(curl -sf -X POST "$SERVER_URL/_synapse/admin/v1/register" \
        -H "Content-Type: application/json" \
        -d "{
            \"nonce\": \"$nonce\",
            \"username\": \"$username\",
            \"password\": \"$password\",
            \"mac\": \"$mac\",
            \"admin\": $admin
        }" 2>/dev/null || true)

    if [ -z "$reg_response" ]; then
        log_error "Registration request failed for $username"
        return 1
    fi

    local user_id
    user_id=$(echo "$reg_response" | jq -r '.user_id // empty')

    if [ -z "$user_id" ]; then
        # Check if user already exists
        local errcode
        errcode=$(echo "$reg_response" | jq -r '.errcode // empty')
        if [ "$errcode" = "M_USER_IN_USE" ]; then
            log_warn "Agent @${username}:${SERVER_NAME} already exists, skipping."
            # Still need to log in to get access token
            login_agent "$username" "$password"
            return 0
        fi
        log_error "Failed to register $username: $reg_response"
        return 1
    fi

    local access_token
    access_token=$(echo "$reg_response" | jq -r '.access_token // empty')

    log_info "Registered: $user_id"

    # Save credentials
    local creds_dir="$PROJECT_DIR/data/agent-credentials"
    mkdir -p "$creds_dir"
    cat > "$creds_dir/${username}.json" <<EOF
{
    "user_id": "$user_id",
    "username": "$username",
    "password": "$password",
    "access_token": "$access_token",
    "server_url": "$SERVER_URL",
    "server_name": "$SERVER_NAME"
}
EOF
    chmod 600 "$creds_dir/${username}.json"
    log_info "Credentials saved to $creds_dir/${username}.json"

    echo "$access_token"
}

# Log in an existing agent to get an access token
login_agent() {
    local username="$1"
    local password="$2"

    log_info "Logging in as @${username}:${SERVER_NAME}..."

    local login_response
    login_response=$(curl -sf -X POST "$SERVER_URL/_matrix/client/v3/login" \
        -H "Content-Type: application/json" \
        -d "{
            \"type\": \"m.login.password\",
            \"identifier\": {
                \"type\": \"m.id.user\",
                \"user\": \"$username\"
            },
            \"password\": \"$password\"
        }" 2>/dev/null || true)

    if [ -z "$login_response" ]; then
        log_error "Login failed for $username"
        return 1
    fi

    local access_token
    access_token=$(echo "$login_response" | jq -r '.access_token // empty')

    if [ -z "$access_token" ]; then
        log_error "No access token in login response: $login_response"
        return 1
    fi

    local user_id
    user_id=$(echo "$login_response" | jq -r '.user_id // empty')

    # Save credentials
    local creds_dir="$PROJECT_DIR/data/agent-credentials"
    mkdir -p "$creds_dir"
    cat > "$creds_dir/${username}.json" <<EOF
{
    "user_id": "$user_id",
    "username": "$username",
    "password": "$password",
    "access_token": "$access_token",
    "server_url": "$SERVER_URL",
    "server_name": "$SERVER_NAME"
}
EOF
    chmod 600 "$creds_dir/${username}.json"
    log_info "Credentials updated for $username"

    echo "$access_token"
}

# Create the default Constellation room
create_room() {
    local creator_token="$1"

    log_info "Creating room #${ROOM_ALIAS}:${SERVER_NAME}..."

    local room_response
    room_response=$(curl -sf -X POST "$SERVER_URL/_matrix/client/v3/createRoom" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $creator_token" \
        -d "{
            \"room_alias_name\": \"$ROOM_ALIAS\",
            \"name\": \"Constellation\",
            \"topic\": \"Constellation A2A Agent Communication Channel\",
            \"visibility\": \"private\",
            \"preset\": \"private_chat\",
            \"creation_content\": {
                \"m.federate\": false
            }
        }" 2>/dev/null || true)

    if [ -z "$room_response" ]; then
        log_error "Failed to create room"
        return 1
    fi

    local room_id
    room_id=$(echo "$room_response" | jq -r '.room_id // empty')

    if [ -z "$room_id" ]; then
        local errcode
        errcode=$(echo "$room_response" | jq -r '.errcode // empty')
        if [ "$errcode" = "M_ROOM_IN_USE" ]; then
            log_warn "Room #${ROOM_ALIAS}:${SERVER_NAME} already exists."
            # Resolve alias to get room_id
            local alias_encoded
            alias_encoded=$(python3 -c "import urllib.parse; print(urllib.parse.quote('#${ROOM_ALIAS}:${SERVER_NAME}'))")
            local resolve_response
            resolve_response=$(curl -sf "$SERVER_URL/_matrix/client/v3/directory/room/$alias_encoded" \
                -H "Authorization: Bearer $creator_token" 2>/dev/null || true)
            room_id=$(echo "$resolve_response" | jq -r '.room_id // empty')
            if [ -z "$room_id" ]; then
                log_error "Could not resolve existing room alias"
                return 1
            fi
        else
            log_error "Failed to create room: $room_response"
            return 1
        fi
    else
        log_info "Room created: $room_id"
    fi

    echo "$room_id"
}

# Invite an agent to a room
invite_to_room() {
    local token="$1"
    local room_id="$2"
    local user_id="$3"

    log_info "Inviting $user_id to room..."

    local invite_response
    invite_response=$(curl -sf -X POST "$SERVER_URL/_matrix/client/v3/rooms/$room_id/invite" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $token" \
        -d "{\"user_id\": \"$user_id\"}" 2>/dev/null || true)

    # Check for error (but ignore "already in room" type errors)
    local errcode
    errcode=$(echo "$invite_response" | jq -r '.errcode // empty' 2>/dev/null || true)
    if [ -n "$errcode" ] && [ "$errcode" != "null" ]; then
        log_warn "Invite response for $user_id: $errcode"
    fi
}

# Join a room
join_room() {
    local token="$1"
    local room_id="$2"

    curl -sf -X POST "$SERVER_URL/_matrix/client/v3/join/$room_id" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $token" \
        -d '{}' &>/dev/null || true
}

# Main
main() {
    log_info "Constellation Agent Registration"
    echo ""

    check_prerequisites

    # Determine which agents to register
    local agents=()
    if [ $# -gt 0 ]; then
        agents=("$@")
    else
        agents=("${DEFAULT_AGENTS[@]}")
    fi

    log_info "Agents to register: ${agents[*]}"
    echo ""

    # Register all agents and collect tokens
    declare -A agent_tokens
    local first_token=""
    local first_agent=""

    for agent in "${agents[@]}"; do
        local password
        password="$(openssl rand -hex 16)"

        local token
        token=$(register_agent "$agent" "$password" "false")

        if [ -n "$token" ]; then
            agent_tokens[$agent]="$token"
            if [ -z "$first_token" ]; then
                first_token="$token"
                first_agent="$agent"
            fi
        fi
        echo ""
    done

    # Create the default room using the first agent
    if [ -n "$first_token" ]; then
        local room_id
        room_id=$(create_room "$first_token")

        if [ -n "$room_id" ]; then
            # Invite and join all other agents
            for agent in "${agents[@]}"; do
                if [ "$agent" != "$first_agent" ] && [ -n "${agent_tokens[$agent]:-}" ]; then
                    invite_to_room "$first_token" "$room_id" "@${agent}:${SERVER_NAME}"
                    join_room "${agent_tokens[$agent]}" "$room_id"
                fi
            done

            echo ""
            log_info "Room $room_id ready with all agents."
        fi
    fi

    echo ""
    echo "========================================="
    echo "  Agent Registration Complete"
    echo "========================================="
    echo ""
    echo "  Registered agents:"
    for agent in "${agents[@]}"; do
        echo "    - @${agent}:${SERVER_NAME}"
    done
    echo ""
    echo "  Room: #${ROOM_ALIAS}:${SERVER_NAME}"
    echo ""
    echo "  Credentials stored in:"
    echo "    $PROJECT_DIR/data/agent-credentials/"
    echo ""
    echo "========================================="
}

main "$@"
