mod auth;
mod config;
mod database;
mod presence;
mod telemetry;

use config::Config;
use database::Database;
use frontier_shared::{
    decode_client_message, encode_server_state_with_crafting_and_vitals, ClientMessage,
    CraftRecipe, EnemySnapshot, InventoryStack, PlayerId, PlayerInput, PlayerSnapshot,
    PlayerVitals, ResourceKind, ResourceSnapshot,
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
const RESOURCE_INTERACTION_DISTANCE: f32 = 64.0;
const RESOURCE_GATHER_AMOUNT: u16 = 1;
const MAX_STACK: u16 = 20;
const CAMP_KIT_WOOD_COST: u16 = 3;
const CAMP_KIT_STONE_COST: u16 = 2;
const STAMINA_DRAIN_PER_SECOND: f32 = 18.0;
const STAMINA_REGEN_PER_SECOND: f32 = 12.0;
const HUNGER_DECAY_PER_SECOND: f32 = 0.5;
const STARVATION_DAMAGE_PER_SECOND: f32 = 4.0;
const ENEMY_SPEED: f32 = 55.0;
const ENEMY_ATTACK_DISTANCE: f32 = 30.0;
const ENEMY_ATTACK_DAMAGE: f32 = 12.0;
const ENEMY_ATTACK_COOLDOWN: f32 = 1.0;
const PLAYER_ATTACK_DISTANCE: f32 = 58.0;
const PLAYER_ATTACK_DAMAGE: f32 = 25.0;

#[derive(Debug)]
struct PlayerState {
    id: PlayerId,
    character_id: i64,
    x: f32,
    y: f32,
    health: f32,
    stamina: f32,
    hunger: f32,
    input: PlayerInput,
    sequence: u32,
    inventory: [u16; 3],
    crafted_kits: u16,
}

#[derive(Debug, Clone, Copy)]
struct ResourceNode {
    id: u32,
    kind: ResourceKind,
    x: f32,
    y: f32,
    remaining: u16,
}

#[derive(Debug)]
struct EnemyState {
    id: u32,
    x: f32,
    y: f32,
    health: f32,
    attack_cooldown: f32,
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
    let redis_client = redis::Client::open(config.redis_url.as_str()).unwrap_or_else(|error| {
        error!(%error, "invalid Redis URL");
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
    let asset_addr = config.asset_addr.clone();
    let asset_database = database.clone();
    tokio::spawn(async move {
        if let Err(error) = run_asset_server(&asset_addr, asset_database).await {
            error!(%error, "asset server stopped");
        }
    });
    info!(address = %config.asset_addr, "asset server started");
    let api_addr = config.api_addr.clone();
    let auth_state = auth::AuthState {
        database: database.clone(),
        redis: redis_client.clone(),
    };
    tokio::spawn(async move {
        if let Err(error) = auth::run_api(&api_addr, auth_state).await {
            error!(%error, "auth API stopped");
        }
    });
    info!(address = %config.api_addr, "auth API started");

    let mut next_id: PlayerId = 1;
    let mut players = HashMap::new();
    let mut resources = initial_resources();
    let mut enemies = initial_enemies();
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
            match message {
                ClientMessage::Authenticate { sequence, token } => {
                    if players.contains_key(&address) {
                        continue;
                    }
                    let Some(account_id) = auth::account_id_for_token(&redis_client, &token)
                        .await
                        .unwrap_or_else(|error| {
                            warn!(%error, %address, "could not resolve game session");
                            None
                        })
                    else {
                        warn!(%address, "rejected game connection with invalid session");
                        continue;
                    };
                    let Some(character) = database
                        .load_character_for_account(account_id)
                        .await
                        .unwrap_or_else(|error| {
                            error!(%error, account_id, "could not load character from PostgreSQL");
                            None
                        })
                    else {
                        warn!(%address, account_id, "account has no character");
                        continue;
                    };
                    let (inventory, crafted_kits) = database
                        .load_inventory(character.id)
                        .await
                        .unwrap_or_else(|error| {
                            error!(%error, character_id = character.id, "could not load character inventory");
                            ([0; 3], 0)
                        });
                    let player = PlayerState {
                        id: next_id,
                        character_id: character.id,
                        x: character.x,
                        y: character.y,
                        health: character.health,
                        stamina: character.stamina,
                        hunger: character.hunger,
                        input: PlayerInput::default(),
                        sequence,
                        inventory,
                        crafted_kits,
                    };
                    info!(player_id = player.id, account_id, character_id = player.character_id, %address, "player authenticated");
                    presence.refresh(player.id).await.ok();
                    next_id += 1;
                    players.insert(address, player);
                }
                ClientMessage::Input { sequence, input } => {
                    let Some(player) = players.get_mut(&address) else {
                        continue;
                    };
                    if !input.is_valid() {
                        warn!(
                            player_id = player.id,
                            ?input,
                            "rejected invalid player input"
                        );
                        continue;
                    }
                    if !is_newer_sequence(sequence, player.sequence) {
                        warn!(
                            player_id = player.id,
                            sequence,
                            last_sequence = player.sequence,
                            "rejected stale player input"
                        );
                        continue;
                    }
                    player.input = input;
                    player.sequence = sequence;
                    if let Err(error) = presence.refresh(player.id).await {
                        error!(player_id = player.id, %error, "could not refresh Redis presence");
                    }
                }
                ClientMessage::Interact {
                    sequence,
                    resource_id,
                } => {
                    let Some(player) = players.get_mut(&address) else {
                        continue;
                    };
                    if !is_newer_sequence(sequence, player.sequence) {
                        continue;
                    }
                    player.sequence = sequence;
                    gather_resource(player, resource_id, &mut resources);
                    if let Err(error) = presence.refresh(player.id).await {
                        error!(player_id = player.id, %error, "could not refresh Redis presence");
                    }
                }
                ClientMessage::Craft { sequence, recipe } => {
                    let Some(player) = players.get_mut(&address) else {
                        continue;
                    };
                    if !is_newer_sequence(sequence, player.sequence) {
                        continue;
                    }
                    player.sequence = sequence;
                    craft_recipe(player, recipe);
                    if let Err(error) = presence.refresh(player.id).await {
                        error!(player_id = player.id, %error, "could not refresh Redis presence");
                    }
                }
                ClientMessage::Consume { sequence, kind } => {
                    let Some(player) = players.get_mut(&address) else {
                        continue;
                    };
                    if !is_newer_sequence(sequence, player.sequence) {
                        continue;
                    }
                    player.sequence = sequence;
                    consume_item(player, kind);
                    if let Err(error) = presence.refresh(player.id).await {
                        error!(player_id = player.id, %error, "could not refresh Redis presence");
                    }
                }
                ClientMessage::Attack { sequence } => {
                    let Some(player) = players.get_mut(&address) else {
                        continue;
                    };
                    if !is_newer_sequence(sequence, player.sequence) {
                        continue;
                    }
                    player.sequence = sequence;
                    attack_enemy(player, &mut enemies);
                    if let Err(error) = presence.refresh(player.id).await {
                        error!(player_id = player.id, %error, "could not refresh Redis presence");
                    }
                }
            }
        }

        tokio::select! {
            _ = ticker.tick() => {
                disconnect_expired_players(&mut players, &mut presence, &database).await;
                advance_players(&mut players, config.tick_duration_seconds());
                update_survival(&mut players, config.tick_duration_seconds());
                update_enemies(&mut enemies, &mut players, config.tick_duration_seconds());
                respawn_dead_players(&mut players);
                send_snapshots(&socket, &players, &resources, &enemies);

                if Instant::now() >= next_save {
                    for player in players.values() {
                        if let Err(error) = database
                            .save_character(
                                player.character_id,
                                player.x,
                                player.y,
                                player.health,
                                player.stamina,
                                player.hunger,
                            )
                            .await
                        {
                            error!(character_id = player.character_id, %error, "could not save character");
                        }
                        if let Err(error) = database
                            .save_inventory(
                                player.character_id,
                                player.inventory,
                                player.crafted_kits,
                            )
                            .await
                        {
                            error!(character_id = player.character_id, %error, "could not save character inventory");
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

fn is_newer_sequence(sequence: u32, previous: u32) -> bool {
    sequence != previous && sequence.wrapping_sub(previous) < (u32::MAX / 2) + 1
}

fn advance_players(players: &mut HashMap<std::net::SocketAddr, PlayerState>, delta_seconds: f32) {
    for player in players.values_mut() {
        let length = f32::from(player.input.move_x).hypot(f32::from(player.input.move_y));
        if length == 0.0 {
            player.stamina = (player.stamina + STAMINA_REGEN_PER_SECOND * delta_seconds).min(100.0);
            continue;
        }
        if player.stamina <= 0.0 {
            player.stamina = (player.stamina + STAMINA_REGEN_PER_SECOND * delta_seconds).min(100.0);
            continue;
        }

        player.stamina = (player.stamina - STAMINA_DRAIN_PER_SECOND * delta_seconds).max(0.0);
        player.x = (player.x + f32::from(player.input.move_x) / length * SPEED * delta_seconds)
            .clamp(WORLD_MIN, WORLD_MAX);
        player.y = (player.y + f32::from(player.input.move_y) / length * SPEED * delta_seconds)
            .clamp(WORLD_MIN, WORLD_MAX);
    }
}

fn update_survival(players: &mut HashMap<std::net::SocketAddr, PlayerState>, delta_seconds: f32) {
    for player in players.values_mut() {
        player.hunger = (player.hunger - HUNGER_DECAY_PER_SECOND * delta_seconds).max(0.0);
        if player.hunger == 0.0 {
            player.health = (player.health - STARVATION_DAMAGE_PER_SECOND * delta_seconds).max(0.0);
        }
    }
}

fn initial_enemies() -> Vec<EnemyState> {
    vec![EnemyState {
        id: 1,
        x: 650.0,
        y: 220.0,
        health: 100.0,
        attack_cooldown: 0.0,
    }]
}

fn update_enemies(
    enemies: &mut [EnemyState],
    players: &mut HashMap<std::net::SocketAddr, PlayerState>,
    delta_seconds: f32,
) {
    for enemy in enemies {
        enemy.attack_cooldown = (enemy.attack_cooldown - delta_seconds).max(0.0);
        let Some((target_address, target_x, target_y, distance)) = players
            .iter()
            .filter(|(_, player)| player.health > 0.0)
            .map(|(address, player)| {
                (
                    *address,
                    player.x,
                    player.y,
                    (player.x - enemy.x).hypot(player.y - enemy.y),
                )
            })
            .min_by(|left, right| left.3.total_cmp(&right.3))
        else {
            continue;
        };

        if distance <= ENEMY_ATTACK_DISTANCE {
            if enemy.attack_cooldown == 0.0 {
                if let Some(player) = players.get_mut(&target_address) {
                    player.health = (player.health - ENEMY_ATTACK_DAMAGE).max(0.0);
                }
                enemy.attack_cooldown = ENEMY_ATTACK_COOLDOWN;
            }
        } else if distance > 0.0 {
            enemy.x = (enemy.x + (target_x - enemy.x) / distance * ENEMY_SPEED * delta_seconds)
                .clamp(WORLD_MIN, WORLD_MAX);
            enemy.y = (enemy.y + (target_y - enemy.y) / distance * ENEMY_SPEED * delta_seconds)
                .clamp(WORLD_MIN, WORLD_MAX);
        }
    }
}

fn attack_enemy(player: &PlayerState, enemies: &mut Vec<EnemyState>) {
    let Some((index, distance)) = enemies
        .iter()
        .enumerate()
        .filter(|(_, enemy)| enemy.health > 0.0)
        .map(|(index, enemy)| (index, (player.x - enemy.x).hypot(player.y - enemy.y)))
        .min_by(|left, right| left.1.total_cmp(&right.1))
    else {
        return;
    };
    if distance <= PLAYER_ATTACK_DISTANCE {
        enemies[index].health = (enemies[index].health - PLAYER_ATTACK_DAMAGE).max(0.0);
        enemies.retain(|enemy| enemy.health > 0.0);
    }
}

fn respawn_dead_players(players: &mut HashMap<std::net::SocketAddr, PlayerState>) {
    for player in players.values_mut() {
        if player.health > 0.0 {
            continue;
        }
        info!(player_id = player.id, "player defeated; respawning");
        player.x = 400.0;
        player.y = 300.0;
        player.health = 100.0;
        player.stamina = 100.0;
        player.hunger = 100.0;
        player.input = PlayerInput::default();
    }
}

fn send_snapshots(
    socket: &UdpSocket,
    players: &HashMap<std::net::SocketAddr, PlayerState>,
    resources: &[ResourceNode],
    enemies: &[EnemyState],
) {
    let snapshots = players
        .values()
        .map(|player| PlayerSnapshot {
            player_id: player.id,
            x: player.x,
            y: player.y,
        })
        .collect::<Vec<_>>();

    for (address, player) in players {
        let resource_snapshots = resources
            .iter()
            .map(|resource| ResourceSnapshot {
                resource_id: resource.id,
                kind: resource.kind,
                x: resource.x,
                y: resource.y,
                remaining: resource.remaining,
            })
            .collect::<Vec<_>>();
        let inventory = inventory_stacks(player.inventory);
        let enemy_snapshots = enemies
            .iter()
            .map(|enemy| EnemySnapshot {
                enemy_id: enemy.id,
                x: enemy.x,
                y: enemy.y,
                health: enemy.health,
            })
            .collect::<Vec<_>>();
        let packet = encode_server_state_with_crafting_and_vitals(
            player.sequence,
            player.id,
            &snapshots,
            &resource_snapshots,
            &inventory,
            player.crafted_kits,
            PlayerVitals {
                health: player.health,
                stamina: player.stamina,
                hunger: player.hunger,
            },
            &enemy_snapshots,
        );
        let _ = socket.send_to(&packet, address);
    }
}

fn initial_resources() -> Vec<ResourceNode> {
    vec![
        ResourceNode {
            id: 1,
            kind: ResourceKind::Wood,
            x: 240.0,
            y: 230.0,
            remaining: 8,
        },
        ResourceNode {
            id: 2,
            kind: ResourceKind::Wood,
            x: 575.0,
            y: 260.0,
            remaining: 8,
        },
        ResourceNode {
            id: 3,
            kind: ResourceKind::Stone,
            x: 300.0,
            y: 520.0,
            remaining: 8,
        },
        ResourceNode {
            id: 4,
            kind: ResourceKind::Stone,
            x: 650.0,
            y: 540.0,
            remaining: 8,
        },
        ResourceNode {
            id: 5,
            kind: ResourceKind::Berries,
            x: 500.0,
            y: 470.0,
            remaining: 8,
        },
    ]
}

fn gather_resource(player: &mut PlayerState, resource_id: u32, resources: &mut [ResourceNode]) {
    let Some(resource) = resources
        .iter_mut()
        .find(|resource| resource.id == resource_id)
    else {
        return;
    };
    if resource.remaining == 0
        || (player.x - resource.x).hypot(player.y - resource.y) > RESOURCE_INTERACTION_DISTANCE
    {
        return;
    }
    let slot = resource.kind as usize - 1;
    if player.inventory[slot] >= MAX_STACK {
        return;
    }
    resource.remaining -= RESOURCE_GATHER_AMOUNT;
    player.inventory[slot] += RESOURCE_GATHER_AMOUNT;
}

fn inventory_stacks(inventory: [u16; 3]) -> Vec<InventoryStack> {
    [
        ResourceKind::Wood,
        ResourceKind::Stone,
        ResourceKind::Berries,
    ]
    .into_iter()
    .zip(inventory)
    .filter(|(_, quantity)| *quantity > 0)
    .map(|(kind, quantity)| InventoryStack { kind, quantity })
    .collect()
}

fn craft_recipe(player: &mut PlayerState, recipe: CraftRecipe) {
    match recipe {
        CraftRecipe::CampKit
            if player.inventory[0] >= CAMP_KIT_WOOD_COST
                && player.inventory[1] >= CAMP_KIT_STONE_COST
                && player.crafted_kits < MAX_STACK =>
        {
            player.inventory[0] -= CAMP_KIT_WOOD_COST;
            player.inventory[1] -= CAMP_KIT_STONE_COST;
            player.crafted_kits += 1;
        }
        CraftRecipe::CampKit => {}
    }
}

fn consume_item(player: &mut PlayerState, kind: ResourceKind) {
    if kind != ResourceKind::Berries || player.inventory[2] == 0 {
        return;
    }
    player.inventory[2] -= 1;
    player.hunger = (player.hunger + 25.0).min(100.0);
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
                            player.hunger,
                        )
                        .await
                    {
                        error!(character_id = player.character_id, %error, "could not save disconnected player");
                    }
                    if let Err(error) = database
                        .save_inventory(player.character_id, player.inventory, player.crafted_kits)
                        .await
                    {
                        error!(character_id = player.character_id, %error, "could not save disconnected player inventory");
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
