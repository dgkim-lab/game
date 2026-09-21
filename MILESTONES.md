# Frontier Echoes milestones

This roadmap keeps development focused on a playable multiplayer slice before expanding
the world or adding MMO-scale systems.

## M0 — Foundation ✅

- [x] Rust workspace with client, server, and shared crates
- [x] Project rules and architecture documentation
- [x] Macroquad graphical client
- [x] UDP client/server connection
- [x] Server-authoritative player movement
- [x] Basic island scene and HUD

## M1 — Reliable local multiplayer

Goal: two local clients can share a stable small world.

- [x] Server tick loop with explicit fixed timestep
- [x] Connection IDs, heartbeats, timeout, and disconnect handling
- [ ] Server snapshots containing all visible players
- [ ] Client interpolation for remote players
- [ ] Input validation and movement speed limits
- [ ] Nearby-player replication and broadcast fan-out
- [ ] Sequence validation and stale/out-of-order packet rejection
- [ ] Basic protocol version and message error handling
- [ ] Integration test for connect, move, snapshot, and disconnect

Acceptance criteria: two client windows can move independently and see each other without
visible teleporting during normal local play.

## M2 — Playable survival loop

Goal: a player can gather, craft, and survive for a short session.

- [ ] Resource nodes such as wood, stone, and berries
- [ ] Interaction system using `E`
- [ ] Inventory with item stack limits
- [ ] Crafting recipes and a simple crafting panel
- [ ] Health, stamina, hunger, and damage
- [ ] One hostile creature with basic AI
- [ ] Respawn at a safe starting location

Acceptance criteria: a new player can gather resources, craft a tool, defeat one enemy,
and recover after death.

## M3 — Persistence and accounts

Goal: player progress survives a server restart.

- [ ] Account registration and login flow
- [ ] Secure password handling and session tokens
- [ ] One authenticated session mapped to one database character
- [ ] PostgreSQL persistence layer
- [ ] Character position, inventory, health, and crafted items saved
- [ ] Periodic saves and graceful shutdown save
- [ ] Migration strategy for database schema changes
- [ ] Server-side ownership checks for all saved data
- [ ] Reconnect resumes the authenticated player's own character

Acceptance criteria: a player can leave, restart the server, reconnect, and retain their
character progress without duplicating or losing items.

## M4 — Building and shared world

Goal: players can create persistent shelters and cooperate.

- [ ] Build mode and placement preview
- [ ] Server validation for placement and resource costs
- [ ] Walls, floors, storage, and campfires
- [ ] Persistent world structures
- [ ] Land claims or permission controls
- [ ] Shared storage permissions
- [ ] Resource respawn and world reset tools for development

Acceptance criteria: two players can build a shelter together, reconnect later, and use
their shared storage according to permissions.

## M5 — Exploration content

Goal: the island offers meaningful reasons to explore.

- [ ] Larger streamed map divided into regions
- [ ] Abandoned research facilities
- [ ] Environmental hazards and weather
- [ ] Resource tiers and improved equipment
- [ ] Short quest and discovery system
- [ ] Map markers and discovered locations
- [ ] Audio, ambient effects, and improved visual assets

Acceptance criteria: a new player can discover several locations and complete a short
sequence of objectives without developer intervention.

## M6 — Public test readiness

Goal: a small group can play safely on a hosted server.

- [ ] Production transport and authentication review
- [ ] Rate limiting and abuse prevention
- [ ] Multiplayer load test with packet-loss and reconnect scenarios
- [ ] Server metrics, structured logs, and health checks
- [ ] Crash recovery and automated backups
- [ ] Admin commands and moderation tools
- [ ] Configuration through environment variables
- [ ] Docker-based deployment
- [ ] Load test with the target concurrent player count and documented limits
- [ ] End-to-end test from clean install to saved character

Acceptance criteria: the server can run unattended, recover from common failures, and
support the planned closed-test player count with documented operational procedures.

## Working rule

Do not start a milestone until the previous milestone's acceptance criteria are met. If a
feature grows beyond the current milestone, split it into a smaller testable slice and
record the follow-up work here.
