use std::{env, error::Error, fmt};

#[derive(Debug, Clone)]
pub struct Config {
    pub server_addr: String,
    pub database_url: String,
    pub redis_url: String,
    pub character_name: String,
    pub otlp_endpoint: Option<String>,
    pub asset_addr: String,
    pub tick_hz: u64,
    pub database_max_connections: u32,
}

#[derive(Debug)]
pub struct ConfigError(String);

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ConfigError {}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();

        let database_url = required("DATABASE_URL")?;
        let redis_url = required("REDIS_URL")?;
        let server_addr = optional("GAME_SERVER_ADDR", "127.0.0.1:4000");
        let character_name = optional("GAME_CHARACTER_NAME", "local-player");
        let otlp_endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();
        let asset_addr = optional("GAME_ASSET_ADDR", "127.0.0.1:4100");
        let tick_hz = parse_positive("GAME_TICK_HZ", 60)?;
        let database_max_connections = parse_positive("DATABASE_MAX_CONNECTIONS", 5)? as u32;

        Ok(Self {
            server_addr,
            database_url,
            redis_url,
            character_name,
            otlp_endpoint,
            asset_addr,
            tick_hz,
            database_max_connections,
        })
    }

    pub fn tick_duration_seconds(&self) -> f32 {
        1.0 / self.tick_hz as f32
    }
}

fn required(name: &str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError(format!("missing required environment variable {name}")))
}

fn optional(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn parse_positive<T>(name: &str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr + PartialOrd + Default + fmt::Display,
{
    let value = optional(name, &default.to_string());
    let parsed = value
        .parse::<T>()
        .map_err(|_| ConfigError(format!("{name} must be a positive number, got {value}")))?;

    if parsed <= T::default() {
        return Err(ConfigError(format!("{name} must be greater than zero")));
    }

    Ok(parsed)
}
