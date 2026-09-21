ALTER TABLE characters
    ADD COLUMN hunger REAL NOT NULL DEFAULT 100;

ALTER TABLE characters
    ADD CONSTRAINT characters_hunger_non_negative CHECK (hunger >= 0);
