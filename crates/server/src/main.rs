mod config;
mod database;
mod telemetry;

use config::Config;
use database::Database;
use frontier_shared::{decode_client_message, encode_server_snapshot, PlayerId, PlayerSnapshot};
use std::{collections::HashMap, net::UdpSocket, time::Duration};
use tokio::time::{sleep, Instant};
use tracing::{error, info, warn};

const WORLD_MIN: f32 = 32.0;
const WORLD_MAX: f32 = 768.0;
const SPEED: f32 = 180.0;

#[derive(Debug)]
struct PlayerState {
    id: PlayerId,
    character_id: i64,
    x: f32,
    y: f32,
    health: f32,
    stamina: f32,
}

#[tokio::main]
async fn main() {
    let config = Config::from_env().unwrap_or_else(|error| {
        eprintln!("configuration error: {error}");
        std::process::exit(1);
    });
    let tracer_provider =
        telemetry::init(config.otlp_endpoint.as_deref()).unwrap_or_else(|error| {
            eprintln!("telemetry configuration error: {error}");
            std::process::exit(1);
        });
    info!(
        otlp_enabled = config.otlp_endpoint.is_some(),
        "telemetry initialized"
    );
    let socket = UdpSocket::bind(&config.server_addr).unwrap_or_else(|error| {
        eprintln!(
            "could not bind game server to {}: {error}",
            config.server_addr
        );
        std::process::exit(1);
    });
    let database = Database::connect(&config.database_url, config.database_max_connections)
        .await
        .unwrap_or_else(|error| {
            eprintln!("could not connect to PostgreSQL: {error}");
            eprintln!("run `sqlx migrate run` before starting the server");
            std::process::exit(1);
        });
    socket
        .set_nonblocking(true)
        .expect("configure non-blocking server socket");

    info!(address = %config.server_addr, "frontier-server listening");
    info!(database = %redact_database_url(&config.database_url), "database configured");
    info!(
        max_connections = config.database_max_connections,
        "database pool configured"
    );
    info!(character = %config.character_name, "loading character");

    let mut next_id: PlayerId = 1;
    let mut players = HashMap::new();
    let mut buffer = [0; 256];
    let tick = Duration::from_secs_f32(config.tick_duration_seconds());
    let save_interval = Duration::from_secs(5);
    let mut next_save = Instant::now() + save_interval;
    let mut shutdown_signal = Box::pin(tokio::signal::ctrl_c());

    loop {
        while let Ok((size, address)) = socket.recv_from(&mut buffer) {
            let Ok(message) = decode_client_message(&buffer[..size]) else {
                continue;
            };
            let input = message.input;

            if !players.contains_key(&address) {
                let character = database
                    .load_or_create_character(&config.character_name)
                    .await
                    .unwrap_or_else(|error| {
                        error!(%error, "could not load character from PostgreSQL");
                        std::process::exit(1);
                    });
                let player = PlayerState {
                    id: next_id,
                    character_id: character.id,
                    x: character.x,
                    y: character.y,
                    health: character.health,
                    stamina: character.stamina,
                };
                info!(player_id = player.id, %address, "player connected");
                next_id += 1;
                players.insert(address, player);
            }

            let player = players
                .get_mut(&address)
                .expect("player inserted immediately above");

            let length = f32::from(input.move_x).hypot(f32::from(input.move_y));
            let (x, y) = if length > 0.0 {
                (
                    f32::from(input.move_x) / length * SPEED * 0.016,
                    f32::from(input.move_y) / length * SPEED * 0.016,
                )
            } else {
                (0.0, 0.0)
            };
            player.x = (player.x + x).clamp(WORLD_MIN, WORLD_MAX);
            player.y = (player.y + y).clamp(WORLD_MIN, WORLD_MAX);

            let snapshot = PlayerSnapshot {
                player_id: player.id,
                x: player.x,
                y: player.y,
            };
            let packet = encode_server_snapshot(message.sequence, snapshot);
            let _ = socket.send_to(&packet, address);
        }

        if Instant::now() >= next_save {
            for player in players.values() {
                if let Err(error) = database
                    .save_character(
                        player.character_id,
                        player.x,
                        player.y,
                        player.health,
                        player.stamina,
                    )
                    .await
                {
                    error!(character_id = player.character_id, %error, "could not save character");
                }
            }
            next_save = Instant::now() + save_interval;
        }

        tokio::select! {
            _ = sleep(tick) => {}
            result = &mut shutdown_signal => {
                if let Err(error) = result {
                    warn!(%error, "shutdown signal listener failed");
                }
                info!("shutdown requested");
                break;
            }
        }
    }

    telemetry::shutdown(tracer_provider);
}

fn redact_database_url(database_url: &str) -> String {
    let Some(at) = database_url.rfind('@') else {
        return "configured".to_owned();
    };
    let Some(scheme_end) = database_url.find("://") else {
        return "configured".to_owned();
    };

    format!(
        "{}://***@{}",
        &database_url[..scheme_end],
        &database_url[at + 1..]
    )
}
