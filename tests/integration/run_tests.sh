#!/usr/bin/env bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

TOTAL_SUITES=0
PASSED_SUITES=0
FAILED_SUITES=0
FAILED_NAMES=()

run_test() {
    local test_script="$1"
    local test_name
    test_name="$(basename "$test_script" .sh)"

    TOTAL_SUITES=$((TOTAL_SUITES + 1))

    echo ""
    echo -e "${BOLD}${CYAN}=======================================${NC}"
    echo -e "${BOLD}${CYAN}  Running: $test_name${NC}"
    echo -e "${BOLD}${CYAN}=======================================${NC}"

    if bash "$test_script"; then
        PASSED_SUITES=$((PASSED_SUITES + 1))
    else
        FAILED_SUITES=$((FAILED_SUITES + 1))
        FAILED_NAMES+=("$test_name")
    fi
}

main() {
    echo ""
    echo -e "${BOLD}=========================================${NC}"
    echo -e "${BOLD}  Constellation Integration Test Suite${NC}"
    echo -e "${BOLD}=========================================${NC}"

    # Run tests in order
    run_test "$SCRIPT_DIR/test_conduit.sh"
    run_test "$SCRIPT_DIR/test_agent_messaging.sh"

    # Summary
    echo ""
    echo ""
    echo -e "${BOLD}=========================================${NC}"
    echo -e "${BOLD}  Test Suite Summary${NC}"
    echo -e "${BOLD}=========================================${NC}"
    echo ""
    echo -e "  Total suites:  $TOTAL_SUITES"
    echo -e "  ${GREEN}Passed:        $PASSED_SUITES${NC}"

    if [ "$FAILED_SUITES" -gt 0 ]; then
        echo -e "  ${RED}Failed:        $FAILED_SUITES${NC}"
        echo ""
        echo -e "  ${RED}Failed suites:${NC}"
        for name in "${FAILED_NAMES[@]}"; do
            echo -e "    ${RED}- $name${NC}"
        done
    else
        echo -e "  ${GREEN}Failed:        0${NC}"
    fi

    echo ""
    echo -e "${BOLD}=========================================${NC}"
    echo ""

    if [ "$FAILED_SUITES" -gt 0 ]; then
        echo -e "${RED}FAILED${NC} - $FAILED_SUITES suite(s) had failures."
        exit 1
    else
        echo -e "${GREEN}ALL TESTS PASSED${NC}"
        exit 0
    fi
}

main "$@"
