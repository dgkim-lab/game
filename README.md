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
- `docs/database.md` — PostgreSQL migration workflow
- `MILESTONES.md` — staged development roadmap and acceptance criteria

## Getting started

Install stable Rust, then run:

```bash
cargo check --workspace
cargo test --workspace
cargo run -p frontier-server
cargo run -p frontier-client
```

## Server configuration

Copy `.env.example` to `.env` and replace `DATABASE_URL` with the connection string for
your existing PostgreSQL database. The `.env` file is ignored by Git.

```bash
cp .env.example .env
cargo run -p frontier-server
```

The server now connects to PostgreSQL, loads the configured development character, and
saves its position every five seconds. The UDP address, tick rate, character name, and
maximum database connection setting can also be changed in `.env`.

## Server tracing

The server emits structured `tracing` logs to stdout. To export spans over OTLP/gRPC to an
OpenTelemetry Collector or Jaeger, set this in `.env`:

```env
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
RUST_LOG=frontier_server=info
```

For local Jaeger development:

```bash
docker run --rm -p 16686:16686 -p 4317:4317 \
  -e COLLECTOR_OTLP_ENABLED=true jaegertracing/all-in-one:latest
```

View traces at `http://localhost:16686`.

The first playable slice now includes a Macroquad graphical client and a UDP server.
Start the server in one terminal, then start the client in another:

```bash
cargo run -p frontier-server
cargo run -p frontier-client
```

Use `W`, `A`, `S`, and `D` to move. The server validates the movement and sends the
authoritative position back to the client.
