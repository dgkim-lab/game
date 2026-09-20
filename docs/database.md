# Database migrations

Frontier Echoes uses an existing PostgreSQL database configured through `DATABASE_URL`.
The game server does not create a database and does not silently change the schema when a
player connects.

## Setup

Install the SQLx command-line tool once:

```bash
cargo install sqlx-cli --no-default-features --features postgres,rustls
```

Create a local `.env` from the example and set the URL for the existing database:

```bash
cp .env.example .env
```

## Commands

Apply all pending migrations:

```bash
sqlx migrate run
```

Inspect migration status:

```bash
sqlx migrate info
```

Create a new migration:

```bash
sqlx migrate add -r describe_the_change
```

The migration files are committed to the repository. Never edit a migration that has
already been applied to a shared database; create a new migration instead.

## Deployment order

1. Set `DATABASE_URL` in the deployment environment.
2. Run `sqlx migrate run`.
3. Start `frontier-server`.

The initial migration creates accounts, characters, inventory items, and buildings. The
server currently only validates the database configuration; repository queries and actual
save/load behavior will be implemented in the persistence milestone.
