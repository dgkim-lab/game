-- Frontier Echoes initial persistent data schema.
-- This migration expects DATABASE_URL to point at an existing PostgreSQL database.

CREATE TABLE accounts (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE characters (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account_id BIGINT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    display_name TEXT NOT NULL UNIQUE,
    position_x REAL NOT NULL DEFAULT 400,
    position_y REAL NOT NULL DEFAULT 300,
    health REAL NOT NULL DEFAULT 100,
    stamina REAL NOT NULL DEFAULT 100,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT characters_health_non_negative CHECK (health >= 0),
    CONSTRAINT characters_stamina_non_negative CHECK (stamina >= 0)
);

CREATE INDEX characters_account_id_idx ON characters(account_id);

CREATE TABLE inventory_items (
    character_id BIGINT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    item_code TEXT NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (character_id, item_code),
    CONSTRAINT inventory_quantity_non_negative CHECK (quantity >= 0)
);

CREATE TABLE buildings (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_character_id BIGINT NOT NULL REFERENCES characters(id) ON DELETE RESTRICT,
    building_code TEXT NOT NULL,
    position_x REAL NOT NULL,
    position_y REAL NOT NULL,
    rotation REAL NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX buildings_position_idx ON buildings(position_x, position_y);
