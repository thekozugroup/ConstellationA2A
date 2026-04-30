#!/usr/bin/env bash
set -euo pipefail

echo "==> cargo audit"
if ! command -v cargo-audit >/dev/null 2>&1; then
  cargo install cargo-audit --locked
fi
cargo audit

echo "==> bind sanity"
CFG="${1:-${HOME}/.config/constellation/config.toml}"
if [ -f "$CFG" ]; then
  bind=$(awk -F\" '/^bind/ { print $2 }' "$CFG" || true)
  case "$bind" in
    0.0.0.0:*|"[::]":*)
      echo "WARN: bind=$bind exposes the listener on all interfaces."
      echo "      Acceptable on a tailscale-only host. Otherwise change to a"
      echo "      tailscale or loopback IP. See docs/SECURITY.md."
      ;;
    "")
      echo "WARN: could not parse bind from $CFG"
      ;;
    *)
      echo "OK: bind=$bind"
      ;;
  esac
else
  echo "no config at $CFG (skipping bind check)"
fi
