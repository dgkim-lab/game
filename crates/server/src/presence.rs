use redis::{aio::MultiplexedConnection, AsyncCommands};

const PRESENCE_TTL_SECONDS: u64 = 5;

#[derive(Clone)]
pub struct Presence {
    connection: MultiplexedConnection,
}

impl Presence {
    pub async fn connect(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        let connection = client.get_multiplexed_async_connection().await?;
        Ok(Self { connection })
    }

    pub async fn refresh(&mut self, player_id: u64) -> Result<(), redis::RedisError> {
        let _: () = self
            .connection
            .set_ex(self.key(player_id), "online", PRESENCE_TTL_SECONDS)
            .await?;
        Ok(())
    }

    pub async fn is_active(&mut self, player_id: u64) -> Result<bool, redis::RedisError> {
        self.connection.exists(self.key(player_id)).await
    }

    fn key(&self, player_id: u64) -> String {
        format!("frontier:presence:{player_id}")
    }
}
