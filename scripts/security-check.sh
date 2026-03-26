#!/usr/bin/env bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

CHECKS_TOTAL=0
CHECKS_PASS=0
CHECKS_WARN=0
CHECKS_FAIL=0

result_pass() {
    CHECKS_TOTAL=$((CHECKS_TOTAL + 1))
    CHECKS_PASS=$((CHECKS_PASS + 1))
    echo -e "  ${GREEN}[PASS]${NC} $1"
}

result_warn() {
    CHECKS_TOTAL=$((CHECKS_TOTAL + 1))
    CHECKS_WARN=$((CHECKS_WARN + 1))
    echo -e "  ${YELLOW}[WARN]${NC} $1"
}

result_fail() {
    CHECKS_TOTAL=$((CHECKS_TOTAL + 1))
    CHECKS_FAIL=$((CHECKS_FAIL + 1))
    echo -e "  ${RED}[FAIL]${NC} $1"
}

# ---- Checks ----

check_registration() {
    echo -e "\n${BOLD}Registration${NC}"

    # Check active config (prefer hardened, fall back to standard)
    local config=""
    for candidate in "$PROJECT_DIR/conduit/conduit-hardened.toml" "$PROJECT_DIR/conduit/conduit.toml"; do
        if [ -f "$candidate" ]; then
            config="$candidate"
            break
        fi
    done

    if [ -z "$config" ]; then
        result_fail "No Conduit config found"
        return
    fi

    local reg_value
    reg_value=$(grep -E '^\s*allow_registration\s*=' "$config" | tail -1 | sed 's/.*=\s*//' | tr -d ' ')

    if [ "$reg_value" = "false" ]; then
        result_pass "Registration is disabled ($config)"
    elif [ "$reg_value" = "true" ]; then
        result_warn "Registration is OPEN ($config) - disable for production"
    else
        result_warn "Could not determine registration setting in $config"
    fi
}

check_shared_secret() {
    echo -e "\n${BOLD}Shared Secret${NC}"

    local secret_file="$PROJECT_DIR/.registration_secret"
    if [ ! -f "$secret_file" ]; then
        result_warn "No .registration_secret file found (OK if not yet initialized)"
        return
    fi

    local secret
    secret="$(cat "$secret_file")"

    if [ "$secret" = "CHANGE_ME_ON_FIRST_RUN" ] || [ "$secret" = "change-me-in-production" ]; then
        result_fail "Shared secret is still the default value - generate a real secret"
    elif [ "${#secret}" -lt 32 ]; then
        result_warn "Shared secret is short (${#secret} chars) - recommend at least 32 chars"
    else
        result_pass "Shared secret is set and has sufficient length (${#secret} chars)"
    fi

    # Check file permissions
    local perms
    perms=$(stat -f '%Lp' "$secret_file" 2>/dev/null || stat -c '%a' "$secret_file" 2>/dev/null || echo "unknown")
    if [ "$perms" = "600" ]; then
        result_pass "Secret file permissions are 600"
    elif [ "$perms" = "unknown" ]; then
        result_warn "Could not determine secret file permissions"
    else
        result_fail "Secret file permissions are $perms (should be 600)"
    fi
}

check_env_file() {
    echo -e "\n${BOLD}Environment File${NC}"

    local env_file="$PROJECT_DIR/.env"
    if [ ! -f "$env_file" ]; then
        result_pass "No .env file present (secrets managed elsewhere)"
        return
    fi

    local has_defaults=false

    while IFS= read -r line; do
        # Skip comments and empty lines
        [[ "$line" =~ ^[[:space:]]*# ]] && continue
        [[ -z "$line" ]] && continue

        local value
        value=$(echo "$line" | cut -d'=' -f2- | tr -d '"' | tr -d "'")

        # Check for common default/placeholder passwords
        case "$value" in
            password|changeme|secret|admin|default|test|""|CHANGE_ME*|change-me*|TODO*|xxx*|placeholder*)
                local key
                key=$(echo "$line" | cut -d'=' -f1)
                result_fail ".env contains default/placeholder value for $key"
                has_defaults=true
                ;;
        esac
    done < "$env_file"

    if [ "$has_defaults" = false ]; then
        result_pass ".env has no obvious default passwords"
    fi
}

check_credential_permissions() {
    echo -e "\n${BOLD}Agent Credential Files${NC}"

    local creds_dir="$PROJECT_DIR/data/agent-credentials"
    if [ ! -d "$creds_dir" ]; then
        result_warn "No agent credentials directory found (OK if agents not yet registered)"
        return
    fi

    local all_ok=true
    local checked=0

    for cred_file in "$creds_dir"/*.json; do
        [ -f "$cred_file" ] || continue
        checked=$((checked + 1))

        local perms
        perms=$(stat -f '%Lp' "$cred_file" 2>/dev/null || stat -c '%a' "$cred_file" 2>/dev/null || echo "unknown")

        if [ "$perms" != "600" ] && [ "$perms" != "unknown" ]; then
            result_fail "$(basename "$cred_file") has permissions $perms (should be 600)"
            all_ok=false
        fi
    done

    if [ "$checked" -eq 0 ]; then
        result_warn "No credential files found in $creds_dir"
    elif [ "$all_ok" = true ]; then
        result_pass "All $checked credential files have correct permissions (600)"
    fi
}

check_docker_network() {
    echo -e "\n${BOLD}Docker Network${NC}"

    if ! command -v docker &>/dev/null; then
        result_warn "Docker not available - skipping network check"
        return
    fi

    # Look for constellation-related networks
    local networks
    networks=$(docker network ls --format '{{.Name}}' 2>/dev/null | grep -i "constellation" || true)

    if [ -z "$networks" ]; then
        result_warn "No constellation Docker network found (OK if not yet started)"
        return
    fi

    while IFS= read -r net; do
        local is_internal
        is_internal=$(docker network inspect "$net" --format '{{.Internal}}' 2>/dev/null || echo "unknown")

        if [ "$is_internal" = "true" ]; then
            result_pass "Network '$net' is internal-only"
        elif [ "$is_internal" = "false" ]; then
            result_warn "Network '$net' is NOT internal-only - consider using an internal network for production"
        else
            result_warn "Could not inspect network '$net'"
        fi
    done <<< "$networks"
}

check_container_root() {
    echo -e "\n${BOLD}Container User${NC}"

    if ! command -v docker &>/dev/null; then
        result_warn "Docker not available - skipping container user check"
        return
    fi

    local containers
    containers=$(docker ps --format '{{.Names}}' 2>/dev/null | grep -i "constellation\|conduit" || true)

    if [ -z "$containers" ]; then
        result_warn "No running constellation/conduit containers found"
        return
    fi

    while IFS= read -r container; do
        local user
        user=$(docker exec "$container" whoami 2>/dev/null || echo "unknown")

        if [ "$user" = "root" ]; then
            result_warn "Container '$container' is running as root - consider a non-root user"
        elif [ "$user" = "unknown" ]; then
            result_warn "Could not determine user for container '$container'"
        else
            result_pass "Container '$container' runs as non-root user '$user'"
        fi
    done <<< "$containers"
}

# ---- Main ----

main() {
    echo ""
    echo -e "${BOLD}=========================================${NC}"
    echo -e "${BOLD}  Constellation Security Audit${NC}"
    echo -e "${BOLD}=========================================${NC}"

    check_registration
    check_shared_secret
    check_env_file
    check_credential_permissions
    check_docker_network
    check_container_root

    echo ""
    echo -e "${BOLD}=========================================${NC}"
    echo -e "${BOLD}  Security Report Summary${NC}"
    echo -e "${BOLD}=========================================${NC}"
    echo ""
    echo -e "  Total checks:  $CHECKS_TOTAL"
    echo -e "  ${GREEN}PASS:          $CHECKS_PASS${NC}"
    echo -e "  ${YELLOW}WARN:          $CHECKS_WARN${NC}"
    echo -e "  ${RED}FAIL:          $CHECKS_FAIL${NC}"
    echo ""

    if [ "$CHECKS_FAIL" -gt 0 ]; then
        echo -e "  ${RED}ACTION REQUIRED: $CHECKS_FAIL issue(s) need attention before production deployment.${NC}"
        echo ""
        exit 1
    elif [ "$CHECKS_WARN" -gt 0 ]; then
        echo -e "  ${YELLOW}Review $CHECKS_WARN warning(s) before production deployment.${NC}"
        echo ""
        exit 0
    else
        echo -e "  ${GREEN}All checks passed. System is production-ready.${NC}"
        echo ""
        exit 0
    fi
}

main "$@"
