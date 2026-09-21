mod config;
mod database;
mod presence;
mod telemetry;

use config::Config;
use database::Database;
use frontier_shared::{
    decode_client_message, encode_server_snapshot, PlayerId, PlayerInput, PlayerSnapshot,
};
use presence::Presence;
use std::{collections::HashMap, net::UdpSocket, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{interval, Instant, MissedTickBehavior};
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
    input: PlayerInput,
    sequence: u32,
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
    let mut presence = Presence::connect(&config.redis_url)
        .await
        .unwrap_or_else(|error| {
            error!(%error, "could not connect to Redis");
            std::process::exit(1);
        });
    socket
        .set_nonblocking(true)
        .expect("configure non-blocking server socket");

    info!(address = %config.server_addr, "frontier-server listening");
    info!(database = %redact_database_url(&config.database_url), "database configured");
    info!(redis = %redact_redis_url(&config.redis_url), "redis configured");
    info!(
        max_connections = config.database_max_connections,
        "database pool configured"
    );
    info!(character = %config.character_name, "loading character");
    let asset_addr = config.asset_addr.clone();
    let asset_database = database.clone();
    tokio::spawn(async move {
        if let Err(error) = run_asset_server(&asset_addr, asset_database).await {
            error!(%error, "asset server stopped");
        }
    });
    info!(address = %config.asset_addr, "asset server started");

    let mut next_id: PlayerId = 1;
    let mut players = HashMap::new();
    let mut buffer = [0; 256];
    let tick = Duration::from_secs_f32(config.tick_duration_seconds());
    let save_interval = Duration::from_secs(5);
    let mut next_save = Instant::now() + save_interval;
    let mut ticker = interval(tick);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
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
                    input: PlayerInput::default(),
                    sequence: message.sequence,
                };
                info!(player_id = player.id, %address, "player connected");
                next_id += 1;
                players.insert(address, player);
            }

            let player = players
                .get_mut(&address)
                .expect("player inserted immediately above");

            player.input = input;
            player.sequence = message.sequence;
            if let Err(error) = presence.refresh(player.id).await {
                error!(player_id = player.id, %error, "could not refresh Redis presence");
            }
        }

        tokio::select! {
            _ = ticker.tick() => {
                disconnect_expired_players(&mut players, &mut presence, &database).await;
                advance_players(&mut players, config.tick_duration_seconds());
                send_snapshots(&socket, &players);

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
            }
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

fn advance_players(players: &mut HashMap<std::net::SocketAddr, PlayerState>, delta_seconds: f32) {
    for player in players.values_mut() {
        let length = f32::from(player.input.move_x).hypot(f32::from(player.input.move_y));
        if length == 0.0 {
            continue;
        }

        player.x = (player.x + f32::from(player.input.move_x) / length * SPEED * delta_seconds)
            .clamp(WORLD_MIN, WORLD_MAX);
        player.y = (player.y + f32::from(player.input.move_y) / length * SPEED * delta_seconds)
            .clamp(WORLD_MIN, WORLD_MAX);
    }
}

fn send_snapshots(socket: &UdpSocket, players: &HashMap<std::net::SocketAddr, PlayerState>) {
    for (address, player) in players {
        let snapshot = PlayerSnapshot {
            player_id: player.id,
            x: player.x,
            y: player.y,
        };
        let packet = encode_server_snapshot(player.sequence, snapshot);
        let _ = socket.send_to(&packet, address);
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

fn redact_redis_url(redis_url: &str) -> String {
    let Some(at) = redis_url.rfind('@') else {
        return redis_url.to_owned();
    };
    let Some(scheme_end) = redis_url.find("://") else {
        return "configured".to_owned();
    };

    format!(
        "{}://***@{}",
        &redis_url[..scheme_end],
        &redis_url[at + 1..]
    )
}

async fn disconnect_expired_players(
    players: &mut HashMap<std::net::SocketAddr, PlayerState>,
    presence: &mut Presence,
    database: &Database,
) {
    let player_addresses = players
        .iter()
        .map(|(address, player)| (*address, player.id))
        .collect::<Vec<_>>();

    for (address, player_id) in player_addresses {
        match presence.is_active(player_id).await {
            Ok(true) => {}
            Ok(false) => {
                if let Some(player) = players.remove(&address) {
                    info!(player_id = player.id, %address, "player disconnected after presence expired");
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
                        error!(character_id = player.character_id, %error, "could not save disconnected player");
                    }
                }
            }
            Err(error) => {
                warn!(player_id, %error, "could not check Redis presence; keeping player connected");
            }
        }
    }
}

async fn run_asset_server(address: &str, database: Database) -> std::io::Result<()> {
    let listener = TcpListener::bind(address).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let request_database = database.clone();
        tokio::spawn(async move {
            if let Err(error) = serve_asset(stream, request_database).await {
                tracing::debug!(%error, "asset request failed");
            }
        });
    }
}

async fn serve_asset(mut stream: TcpStream, database: Database) -> std::io::Result<()> {
    let mut request = [0; 1024];
    let size = stream.read(&mut request).await?;
    let request = String::from_utf8_lossy(&request[..size]);
    let path = request.split_whitespace().nth(1).unwrap_or_default();

    if path != "/assets/world.json" {
        let body = b"not found";
        let response = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).await?;
        stream.write_all(body).await?;
        return Ok(());
    }

    let asset = database
        .load_asset("world.json")
        .await
        .map_err(std::io::Error::other)?
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "asset not found"))?;
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        asset.content_type,
        asset.data.len()
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(&asset.data).await
}
