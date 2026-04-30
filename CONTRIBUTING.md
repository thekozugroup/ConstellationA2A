# Contributing to Constellation A2A

Thanks for your interest. The project is a Rust workspace; the entire stack
builds with stable Rust ≥ 1.75.

## Local development

```bash
git clone https://github.com/thekozugroup/ConstellationA2A.git
cd ConstellationA2A
cargo build --workspace
cargo test --workspace
```

## Required checks

Before opening a pull request, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs these on every push and pull request to `main`.

## Project layout

- `crates/constellation-a2a` — A2A protocol wire types (no I/O).
- `crates/constellation-store` — SQLite persistence.
- `crates/constellation-discovery` — Tailscale + mDNS peer discovery.
- `crates/constellation-server` — Axum HTTP A2A server.
- `crates/constellation-client` — A2A JSON-RPC client.
- `crates/constellation-cli` — `constellation` binary.

## Commit conventions

Use [Conventional Commits](https://www.conventionalcommits.org/) where it
helps clarify intent (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`,
`chore:`). One logical change per commit.

## Security

See `docs/SECURITY.md` for the trust model. Do not open public issues for
security reports — email `thekozugroup@gmail.com` instead.
