# Security model

Constellation A2A trusts its transport. The binary itself does not
authenticate callers and does not encrypt traffic — the assumption is that
peers reach each other over a [Tailscale](https://tailscale.com/) tailnet,
which provides device identity and WireGuard encryption between peers.

## Trust boundary

- **Trusted:** every device on your tailnet (or the LAN, if `mdns` discovery
  is enabled and `bind` is set to a LAN-reachable address).
- **Untrusted:** the public internet. The binary must never be exposed
  there.

## Default bind

`bind = "0.0.0.0:7777"` so the listener is reachable on whichever interface
you advertise. **If you are not on a tailnet and your LAN is hostile**, change
`bind` to one of:

- `127.0.0.1:7777` — local only (useful for dev / single-host testing).
- A specific Tailscale IP such as `100.76.147.110:7777` — only the tailnet.

## Hardening checklist

1. Run `tailscale status` and confirm the device is in the expected tailnet.
2. Set `[network] discovery = ["tailscale"]` (drop `mdns`) on hosts that
   should never accept LAN peers.
3. Set `bind` to your Tailscale IP, not `0.0.0.0`, on multi-homed boxes.
4. Run `./scripts/security-check.sh` before exposing a new node — it asserts
   the bind is not a public IP and runs `cargo audit`.
5. Rotate Tailscale auth keys via the Tailscale admin console if a peer is
   suspected compromised.

## What the binary does **not** do

- It does not execute task content. The LLM agent reads tasks and decides what
  to do; the binary only persists and forwards.
- It does not store secrets in agent cards.
- It does not expose any verbs beyond the documented A2A JSON-RPC methods and
  `/.well-known/agent.json`.

## Reporting

Report vulnerabilities privately to `thekozugroup@gmail.com`.
