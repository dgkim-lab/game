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

#[derive(Debug, Clone)]
pub struct Asset {
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct AccountCredentials {
    pub account_id: i64,
    pub password_hash: String,
}

#[derive(Debug, Clone, Copy)]
pub struct AccountCharacter {
    pub account_id: i64,
    pub character_id: i64,
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

    pub async fn create_account(
        &self,
        username: &str,
        password_hash: &str,
    ) -> Result<Option<AccountCharacter>, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let account = sqlx::query(
            "INSERT INTO accounts (username, password_hash)
             VALUES ($1, $2)
             ON CONFLICT (username) DO NOTHING
             RETURNING id",
        )
        .bind(username)
        .bind(password_hash)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(account) = account else {
            return Ok(None);
        };
        let account_id: i64 = account.try_get("id")?;
        let character = sqlx::query(
            "INSERT INTO characters (account_id, display_name)
             VALUES ($1, $2)
             RETURNING id",
        )
        .bind(account_id)
        .bind(format!("character-{account_id}"))
        .fetch_one(&mut *transaction)
        .await?;
        let character_id: i64 = character.try_get("id")?;
        transaction.commit().await?;

        Ok(Some(AccountCharacter {
            account_id,
            character_id,
        }))
    }

    pub async fn find_account(
        &self,
        username: &str,
    ) -> Result<Option<AccountCredentials>, sqlx::Error> {
        let account = sqlx::query(
            "SELECT id, password_hash
             FROM accounts
             WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        account
            .map(|row| {
                Ok(AccountCredentials {
                    account_id: row.try_get("id")?,
                    password_hash: row.try_get("password_hash")?,
                })
            })
            .transpose()
    }

    pub async fn character_for_account(&self, account_id: i64) -> Result<Option<i64>, sqlx::Error> {
        let character =
            sqlx::query("SELECT id FROM characters WHERE account_id = $1 ORDER BY id LIMIT 1")
                .bind(account_id)
                .fetch_optional(&self.pool)
                .await?;
        character.map(|row| row.try_get("id")).transpose()
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

    pub async fn load_asset(&self, asset_key: &str) -> Result<Option<Asset>, sqlx::Error> {
        let asset = sqlx::query(
            "SELECT content_type, data
             FROM game_assets
             WHERE asset_key = $1",
        )
        .bind(asset_key)
        .fetch_optional(&self.pool)
        .await?;

        asset
            .map(|row| {
                Ok(Asset {
                    content_type: row.try_get("content_type")?,
                    data: row.try_get::<String, _>("data")?.into_bytes(),
                })
            })
            .transpose()
    }
}
