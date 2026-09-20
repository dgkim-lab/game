mod config;
mod database;

use config::Config;
use database::Database;
use frontier_shared::{PlayerId, PlayerInput, PlayerSnapshot};
use std::{collections::HashMap, net::UdpSocket, time::Duration};
use tokio::time::{sleep, Instant};

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

    println!("frontier-server listening on {}", config.server_addr);
    println!(
        "database configured: {}",
        redact_database_url(&config.database_url)
    );
    println!(
        "database max connections: {}",
        config.database_max_connections
    );
    println!("loading character: {}", config.character_name);

    let mut next_id: PlayerId = 1;
    let mut players = HashMap::new();
    let mut buffer = [0; PlayerInput::BYTE_LEN];
    let tick = Duration::from_secs_f32(config.tick_duration_seconds());
    let save_interval = Duration::from_secs(5);
    let mut next_save = Instant::now() + save_interval;

    loop {
        while let Ok((size, address)) = socket.recv_from(&mut buffer) {
            let Some(input) = PlayerInput::decode(&buffer[..size]) else {
                continue;
            };

            if !players.contains_key(&address) {
                let character = database
                    .load_or_create_character(&config.character_name)
                    .await
                    .unwrap_or_else(|error| {
                        eprintln!("could not load character from PostgreSQL: {error}");
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
                println!("player {} connected from {address}", player.id);
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
            let _ = socket.send_to(&snapshot.encode(), address);
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
                    eprintln!("could not save character {}: {error}", player.character_id);
                }
            }
            next_save = Instant::now() + save_interval;
        }

        sleep(tick).await;
    }
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
