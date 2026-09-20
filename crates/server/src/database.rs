use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tracing::instrument;

#[derive(Debug, Clone, Copy)]
pub struct CharacterState {
    pub id: i64,
    pub x: f32,
    pub y: f32,
    pub health: f32,
    pub stamina: f32,
}

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    #[instrument(skip(self), fields(character.name = character_name))]
    pub async fn load_or_create_character(
        &self,
        character_name: &str,
    ) -> Result<CharacterState, sqlx::Error> {
        let account = sqlx::query(
            "INSERT INTO accounts (username, password_hash)
             VALUES ($1, $2)
             ON CONFLICT (username) DO UPDATE SET username = EXCLUDED.username
             RETURNING id",
        )
        .bind(format!("{character_name}-account"))
        .bind("local-development-placeholder")
        .fetch_one(&self.pool)
        .await?;
        let account_id: i64 = account.try_get("id")?;

        let character = sqlx::query(
            "INSERT INTO characters (account_id, display_name)
             VALUES ($1, $2)
             ON CONFLICT (display_name) DO UPDATE SET display_name = EXCLUDED.display_name
             RETURNING id, position_x, position_y, health, stamina",
        )
        .bind(account_id)
        .bind(character_name)
        .fetch_one(&self.pool)
        .await?;

        Ok(CharacterState {
            id: character.try_get("id")?,
            x: character.try_get("position_x")?,
            y: character.try_get("position_y")?,
            health: character.try_get("health")?,
            stamina: character.try_get("stamina")?,
        })
    }

    #[instrument(skip(self), fields(character.id = character_id))]
    pub async fn save_character(
        &self,
        character_id: i64,
        x: f32,
        y: f32,
        health: f32,
        stamina: f32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE characters
             SET position_x = $1, position_y = $2, health = $3, stamina = $4, updated_at = now()
             WHERE id = $5",
        )
        .bind(x)
        .bind(y)
        .bind(health)
        .bind(stamina)
        .bind(character_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
