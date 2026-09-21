use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tracing::instrument;

#[derive(Debug, Clone, Copy)]
pub struct CharacterState {
    pub id: i64,
    pub x: f32,
    pub y: f32,
    pub health: f32,
    pub stamina: f32,
    pub hunger: f32,
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

    pub async fn load_character_for_account(
        &self,
        account_id: i64,
    ) -> Result<Option<CharacterState>, sqlx::Error> {
        let character = sqlx::query(
            "SELECT id, position_x, position_y, health, stamina, hunger
             FROM characters
             WHERE account_id = $1
             ORDER BY id
             LIMIT 1",
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?;

        character
            .map(|row| {
                Ok(CharacterState {
                    id: row.try_get("id")?,
                    x: row.try_get("position_x")?,
                    y: row.try_get("position_y")?,
                    health: row.try_get("health")?,
                    stamina: row.try_get("stamina")?,
                    hunger: row.try_get("hunger")?,
                })
            })
            .transpose()
    }

    #[instrument(skip(self), fields(character.id = character_id))]
    pub async fn save_character(
        &self,
        account_id: i64,
        character_id: i64,
        x: f32,
        y: f32,
        health: f32,
        stamina: f32,
        hunger: f32,
    ) -> Result<(), sqlx::Error> {
        let result = sqlx::query(
            "UPDATE characters
             SET position_x = $1, position_y = $2, health = $3, stamina = $4, hunger = $5, updated_at = now()
             WHERE id = $6 AND account_id = $7",
        )
        .bind(x)
        .bind(y)
        .bind(health)
        .bind(stamina)
        .bind(hunger)
        .bind(character_id)
        .bind(account_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(sqlx::Error::RowNotFound);
        }
        Ok(())
    }

    pub async fn load_inventory(
        &self,
        account_id: i64,
        character_id: i64,
    ) -> Result<([u16; 3], u16), sqlx::Error> {
        let rows = sqlx::query(
            "SELECT item_code, quantity
             FROM inventory_items
             WHERE character_id = $1
               AND EXISTS (
                   SELECT 1 FROM characters
                   WHERE characters.id = inventory_items.character_id
                     AND characters.account_id = $2
               )",
        )
        .bind(character_id)
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;
        let mut inventory = [0; 3];
        let mut crafted_kits = 0;
        for row in rows {
            let item_code: String = row.try_get("item_code")?;
            let quantity: i32 = row.try_get("quantity")?;
            let quantity = quantity.clamp(0, u16::MAX as i32) as u16;
            if item_code == "camp_kit" {
                crafted_kits = quantity;
            } else if let Some(slot) = inventory_slot(&item_code) {
                inventory[slot] = quantity;
            }
        }
        Ok((inventory, crafted_kits))
    }

    pub async fn save_inventory(
        &self,
        account_id: i64,
        character_id: i64,
        inventory: [u16; 3],
        crafted_kits: u16,
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let owned = sqlx::query("SELECT 1 FROM characters WHERE id = $1 AND account_id = $2")
            .bind(character_id)
            .bind(account_id)
            .fetch_optional(&mut *transaction)
            .await?;
        if owned.is_none() {
            return Err(sqlx::Error::RowNotFound);
        }
        sqlx::query("DELETE FROM inventory_items WHERE character_id = $1")
            .bind(character_id)
            .execute(&mut *transaction)
            .await?;

        for (slot, quantity) in inventory.into_iter().enumerate() {
            if quantity == 0 {
                continue;
            }
            sqlx::query(
                "INSERT INTO inventory_items (character_id, item_code, quantity)
                 VALUES ($1, $2, $3)",
            )
            .bind(character_id)
            .bind(inventory_item_code(slot))
            .bind(i32::from(quantity))
            .execute(&mut *transaction)
            .await?;
        }
        if crafted_kits > 0 {
            sqlx::query(
                "INSERT INTO inventory_items (character_id, item_code, quantity)
                 VALUES ($1, 'camp_kit', $2)",
            )
            .bind(character_id)
            .bind(i32::from(crafted_kits))
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await
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

fn inventory_slot(item_code: &str) -> Option<usize> {
    match item_code {
        "wood" => Some(0),
        "stone" => Some(1),
        "berries" => Some(2),
        _ => None,
    }
}

fn inventory_item_code(slot: usize) -> &'static str {
    match slot {
        0 => "wood",
        1 => "stone",
        2 => "berries",
        _ => unreachable!("invalid inventory slot"),
    }
}
