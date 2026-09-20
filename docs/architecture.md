# Initial architecture

```text
┌──────────────┐       messages        ┌──────────────┐
│ Game client  │ ◄────────────────────► │ Game server  │
│ input/render │                        │ world/state  │
└──────────────┘                        └──────┬───────┘
                                              │
                                              ▼
                                       ┌──────────────┐
                                       │ Persistence  │
                                       │ player/world │
                                       └──────────────┘
```

## Boundaries

The client owns input collection, rendering, audio, local prediction, and presentation.
The server owns authentication, validation, simulation, combat, inventories, resource
respawns, building, and the canonical world state.

The shared crate contains protocol-shaped data only. It should not depend on rendering,
database, or operating-system APIs.

## Planned backend layers

1. Transport: the prototype uses localhost UDP with fixed-size messages. Connection
   lifecycle, framing, authentication, and rate limits are still pending.
2. Protocol: versioned client commands and server snapshots/events.
3. Simulation: fixed server ticks and deterministic gameplay systems.
4. Persistence: accounts, characters, inventories, buildings, and world changes.
5. Operations: structured logs, metrics, health checks, and graceful shutdown.

The current implementation uses UDP to exercise the client/server boundary early. It is
intended for local development only; production networking will need authentication,
timeouts, replay protection, protocol versioning, and a more robust transport strategy.

## Protocol envelope

Gameplay packets use a shared versioned envelope:

```text
magic | version | message type | sequence | payload length | payload
  2B       1B          1B            4B           2B
```

The shared crate validates the envelope before decoding a message. New messages should be
added as new `MessageType` variants and should preserve compatibility with existing packet
versions whenever possible.
