# Frontier Echoes

## Project overview

Frontier Echoes is a persistent multiplayer open-world survival game written in Rust.
The first milestone is a small, server-authoritative multiplayer island. Players should be
able to connect, move, gather resources, save progress, and build a simple structure.

The project is intentionally starting as an MMO-lite foundation rather than attempting a
large seamless MMO immediately.

## Repository layout

```text
crates/
  client/   Game client and presentation layer
  server/   Authoritative simulation and networking entry point
  shared/   Protocol types and data shared by client and server
docs/       Design and architecture notes
```

## Development principles

- Keep the server authoritative. The client may predict input and render effects, but it
  must not decide final movement, combat, inventory, or world state.
- Keep shared types free of client- or server-specific behavior.
- Prefer small, deterministic systems over large objects with hidden state.
- Make persistence explicit. In-memory state is acceptable for prototypes, but gameplay
  state that must survive a restart belongs behind a persistence boundary.
- Build a vertical slice before expanding the map, content, or player count.
- Avoid adding a dependency until the need is demonstrated by the current milestone.

## Rust conventions

- Use stable Rust and edition 2021 unless the workspace is deliberately upgraded.
- Format with `cargo fmt --all`.
- Check with `cargo check --workspace`.
- Run tests with `cargo test --workspace`.
- Use `Result` for recoverable failures and include context at system boundaries.
- Do not use `unwrap()` or `expect()` in long-running server paths unless the invariant is
  documented and impossible to violate after validation.
- Keep public APIs small and document protocol-facing types.

## Networking rules

- Treat all client input as untrusted.
- Validate message size, frequency, identifiers, and ownership on the server.
- Version protocol messages when compatibility becomes necessary.
- Never trust client-provided position, inventory, damage, or resource values.
- Add reconnect and timeout behavior before adding more world features.

## Current milestone: local vertical slice

1. Define the shared connection and input messages.
2. Start a server and connect a local client.
3. Synchronize one player position using server ticks.
4. Add one gatherable resource and a saved inventory.
5. Add one buildable object and basic persistence.

## Useful commands

```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo run -p frontier-server
cargo run -p frontier-client
```

Keep design decisions and protocol changes documented in `docs/` when they affect more
than one crate.
