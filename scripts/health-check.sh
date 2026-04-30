#!/usr/bin/env bash
set -euo pipefail
PORT="${PORT:-7777}"
HOST="${HOST:-127.0.0.1}"
URL="http://$HOST:$PORT/.well-known/agent.json"
echo "GET $URL"
curl --fail --silent --show-error --max-time 5 "$URL" | head -c 400
echo
