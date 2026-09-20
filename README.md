# Frontier Echoes

An open-world multiplayer survival game built with Rust.

The project is being developed as a server-authoritative multiplayer vertical slice first.
The initial world is a small island where players explore, gather resources, build shelters,
and discover abandoned research facilities.

## Workspace

- `crates/client` — client entry point and rendering/input code
- `crates/server` — authoritative world simulation and backend entry point
- `crates/shared` — types shared by the client and server
- `docs/architecture.md` — initial technical boundaries

## Getting started

Install stable Rust, then run:

```bash
cargo check --workspace
cargo test --workspace
cargo run -p frontier-server
cargo run -p frontier-client
```

The first playable slice now includes a Macroquad graphical client and a UDP server.
Start the server in one terminal, then start the client in another:

```bash
cargo run -p frontier-server
cargo run -p frontier-client
```

Use `W`, `A`, `S`, and `D` to move. The server validates the movement and sends the
authoritative position back to the client.
