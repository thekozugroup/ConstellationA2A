#!/usr/bin/env bash
set -euo pipefail

SERVER_URL="${CONDUIT_SERVER_URL:-http://localhost:8448}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

check_passed=0
check_failed=0

check() {
    local name="$1"
    local url="$2"

    printf "  %-40s " "$name"

    local http_code
    http_code=$(curl -sf -o /dev/null -w '%{http_code}' "$url" 2>/dev/null || echo "000")

    if [ "$http_code" = "200" ]; then
        echo -e "${GREEN}OK${NC} (HTTP $http_code)"
        check_passed=$((check_passed + 1))
    else
        echo -e "${RED}FAIL${NC} (HTTP $http_code)"
        check_failed=$((check_failed + 1))
    fi
}

echo ""
echo "Constellation Conduit Health Check"
echo "==================================="
echo "  Server: $SERVER_URL"
echo ""

# Core endpoints
check "Client API versions"      "$SERVER_URL/_matrix/client/versions"
check "Server version"            "$SERVER_URL/_matrix/federation/v1/version"
check "Well-known (client)"       "$SERVER_URL/.well-known/matrix/client"

echo ""
echo "-----------------------------------"
printf "  Results: ${GREEN}%d passed${NC}, " "$check_passed"

if [ "$check_failed" -gt 0 ]; then
    printf "${RED}%d failed${NC}\n" "$check_failed"
else
    printf "${GREEN}%d failed${NC}\n" "$check_failed"
fi

echo ""

if [ "$check_failed" -gt 0 ]; then
    echo -e "  ${RED}Conduit is NOT fully healthy.${NC}"
    echo "  Check logs: docker compose logs conduit"
    exit 1
else
    echo -e "  ${GREEN}Conduit is healthy and ready.${NC}"
    exit 0
fi
