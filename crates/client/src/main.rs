use frontier_shared::{
    decode_server_message, encode_client_authenticate, encode_client_consume, encode_client_craft,
    encode_client_input, encode_client_interact, CraftRecipe, InventoryStack, PlayerInput,
    PlayerSnapshot, ResourceKind, ResourceSnapshot, WorldAsset,
};
use macroquad::prelude::*;
use std::{
    io::{Read, Write},
    net::{TcpStream, UdpSocket},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

const SERVER_ADDRESS: &str = "127.0.0.1:4000";
const ASSET_SERVER_ADDRESS: &str = "127.0.0.1:4100";
const AUTH_API_ADDRESS: &str = "127.0.0.1:8080";
const WORLD_SIZE: f32 = 800.0;

#[derive(Clone, Copy)]
enum AuthMode {
    Login,
    Signup,
}

#[derive(Clone, Copy)]
enum AuthField {
    Username,
    Password,
}

#[derive(Debug, Clone, Copy)]
struct RemotePlayer {
    player_id: u64,
    x: f32,
    y: f32,
    target_x: f32,
    target_y: f32,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Frontier Echoes".to_owned(),
        window_width: WORLD_SIZE as i32,
        window_height: WORLD_SIZE as i32,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut asset_receiver = spawn_asset_download();
    let mut world = None;
    let mut asset_error = None;
    let mut socket = None;
    let mut game_error = None;
    let mut last_server_response: Option<Instant> = None;
    let mut player = PlayerSnapshot {
        player_id: 0,
        x: 400.0,
        y: 300.0,
    };
    let mut other_players = Vec::new();
    let mut resources: Vec<ResourceSnapshot> = Vec::new();
    let mut inventory: Vec<InventoryStack> = Vec::new();
    let mut crafted_kits = 0;
    let mut crafting_open = false;
    let mut health = 100.0;
    let mut stamina = 100.0;
    let mut hunger = 100.0;
    let mut receive_buffer = [0; 2048];
    let mut sequence = 0;
    let mut last_frame = Instant::now();
    let mut session_token = std::env::var("GAME_SESSION_TOKEN").unwrap_or_default();
    let mut authenticated = false;
    let mut username = String::new();
    let mut password = String::new();
    let mut auth_mode = AuthMode::Login;
    let mut auth_field = AuthField::Username;
    let mut auth_error = None;
    let mut auth_receiver: Option<Receiver<Result<String, String>>> = None;

    loop {
        if world.is_none() {
            match asset_receiver.try_recv() {
                Ok(Ok(asset)) => {
                    world = Some(asset);
                    asset_error = None;
                    if !session_token.is_empty() {
                        match connect_game_socket() {
                            Ok(new_socket) => socket = Some(new_socket),
                            Err(error) => game_error = Some(error),
                        }
                    }
                }
                Ok(Err(error)) => asset_error = Some(error),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    asset_error = Some("asset loader stopped unexpectedly".to_owned())
                }
            }

            if let Some(error) = asset_error.as_deref() {
                draw_asset_error(error);
                if is_key_pressed(KeyCode::R) {
                    asset_error = None;
                    asset_receiver = spawn_asset_download();
                }
            } else {
                draw_loading_screen();
            }
            next_frame().await;
            continue;
        }

        if socket.is_none() && session_token.is_empty() {
            if let Some(receiver) = auth_receiver.as_ref() {
                match receiver.try_recv() {
                    Ok(Ok(token)) => {
                        session_token = token;
                        auth_receiver = None;
                        auth_error = None;
                        match connect_game_socket() {
                            Ok(new_socket) => socket = Some(new_socket),
                            Err(error) => game_error = Some(error),
                        }
                    }
                    Ok(Err(error)) => {
                        auth_error = Some(error);
                        auth_receiver = None;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        auth_error = Some("authentication request stopped unexpectedly".to_owned());
                        auth_receiver = None;
                    }
                }
            }

            if socket.is_none() {
                handle_login_input(
                    &mut username,
                    &mut password,
                    &mut auth_field,
                    &mut auth_mode,
                    &mut auth_receiver,
                    &mut auth_error,
                );
                draw_login_screen(
                    &username,
                    &password,
                    auth_field,
                    auth_mode,
                    auth_error.as_deref(),
                    auth_receiver.is_some(),
                );
                next_frame().await;
                continue;
            }
        }

        let connection_lost = last_server_response
            .map(|time| time.elapsed() > Duration::from_secs(2))
            .unwrap_or(true);
        let frame_delta = last_frame.elapsed().as_secs_f32().min(0.1);
        last_frame = Instant::now();
        if is_key_pressed(KeyCode::R) && (game_error.is_some() || connection_lost) {
            match connect_game_socket() {
                Ok(new_socket) => {
                    socket = Some(new_socket);
                    game_error = None;
                    last_server_response = None;
                    sequence = 0;
                    authenticated = false;
                }
                Err(error) => game_error = Some(error),
            }
        }

        let input = PlayerInput {
            move_x: i8::from(is_key_down(KeyCode::D)) - i8::from(is_key_down(KeyCode::A)),
            move_y: i8::from(is_key_down(KeyCode::S)) - i8::from(is_key_down(KeyCode::W)),
        };
        if is_key_pressed(KeyCode::C) {
            crafting_open = !crafting_open;
        }
        if let Some(socket) = socket.as_ref() {
            let packet = if authenticated && crafting_open && is_key_pressed(KeyCode::Enter) {
                encode_client_craft(sequence, CraftRecipe::CampKit)
            } else if authenticated && is_key_pressed(KeyCode::F) {
                encode_client_consume(sequence, ResourceKind::Berries)
            } else if authenticated && is_key_pressed(KeyCode::E) {
                nearest_resource_id(&resources, player)
                    .map(|resource_id| encode_client_interact(sequence, resource_id))
                    .unwrap_or_else(|| encode_client_input(sequence, input))
            } else if authenticated {
                encode_client_input(sequence, input)
            } else if session_token.is_empty() {
                game_error = Some("set GAME_SESSION_TOKEN after signing in".to_owned());
                Vec::new()
            } else {
                encode_client_authenticate(sequence, &session_token)
            };
            if !packet.is_empty() && socket.send(&packet).is_err() {
                game_error = Some("could not send input to game server".to_owned());
            }
            sequence = sequence.wrapping_add(1);

            while let Ok(size) = socket.recv(&mut receive_buffer) {
                if let Ok(message) = decode_server_message(&receive_buffer[..size]) {
                    authenticated = true;
                    player.player_id = message.player_id;
                    if let Some(own_snapshot) = message
                        .snapshots
                        .iter()
                        .find(|snapshot| snapshot.player_id == player.player_id)
                    {
                        player = *own_snapshot;
                    } else if let Some(first_snapshot) = message.snapshots.first() {
                        player = *first_snapshot;
                    }
                    update_remote_players(&mut other_players, &message.snapshots, player.player_id);
                    resources = message.resources;
                    inventory = message.inventory;
                    crafted_kits = message.crafted_kits;
                    health = message.vitals.health;
                    stamina = message.vitals.stamina;
                    hunger = message.vitals.hunger;
                    last_server_response = Some(Instant::now());
                    game_error = None;
                }
            }
        } else {
            game_error = Some("game server connection is not available".to_owned());
        }

        for other in &mut other_players {
            let blend = (frame_delta * 14.0).min(1.0);
            other.x += (other.target_x - other.x) * blend;
            other.y += (other.target_y - other.y) * blend;
        }

        let world = world.as_ref().expect("world loaded before gameplay loop");
        set_camera(&world_camera(player));
        clear_background(color(world.background));
        draw_world(world);
        draw_resources(&resources, player);
        for other in &other_players {
            if other.player_id != player.player_id {
                draw_circle(other.x, other.y, 14.0, ORANGE);
                draw_circle_lines(other.x, other.y, 14.0, 2.0, WHITE);
            }
        }
        draw_circle(player.x, player.y, 16.0, SKYBLUE);
        draw_circle_lines(player.x, player.y, 16.0, 2.0, WHITE);

        set_default_camera();
        draw_text("FRONTIER ECHOES", 24.0, 34.0, 28.0, WHITE);
        draw_text("WASD  Move", 24.0, 62.0, 20.0, LIGHTGRAY);
        draw_text("E  Gather nearby resource", 24.0, 86.0, 18.0, LIGHTGRAY);
        draw_text("C  Craft    F  Eat berries", 24.0, 110.0, 18.0, LIGHTGRAY);
        draw_text(
            &format!("Players online: {}", other_players.len()),
            24.0,
            136.0,
            18.0,
            LIGHTGRAY,
        );
        let connection_text = if game_error.is_some() || connection_lost {
            "Connection problem — press R to retry"
        } else {
            "Connected to authoritative server"
        };
        let connection_color = if game_error.is_some() || connection_lost {
            ORANGE
        } else {
            GREEN
        };
        draw_text(connection_text, 24.0, 160.0, 18.0, connection_color);
        draw_text(
            &format!(
                "Player {}  ({:.0}, {:.0})",
                player.player_id, player.x, player.y
            ),
            24.0,
            screen_height() - 24.0,
            18.0,
            LIGHTGRAY,
        );
        draw_inventory(&inventory);
        draw_vitals(health, stamina, hunger);
        if crafting_open {
            draw_crafting_panel(&inventory, crafted_kits);
        }

        if game_error.is_some() || connection_lost {
            draw_rectangle(
                120.0,
                210.0,
                screen_width() - 240.0,
                120.0,
                Color::from_rgba(20, 25, 30, 235),
            );
            draw_text("GAME SERVER UNAVAILABLE", 170.0, 255.0, 26.0, ORANGE);
            draw_text("Press R to reconnect", 235.0, 290.0, 20.0, WHITE);
        }

        next_frame().await;
    }
}

fn nearest_resource_id(resources: &[ResourceSnapshot], player: PlayerSnapshot) -> Option<u32> {
    resources
        .iter()
        .filter(|resource| resource.remaining > 0)
        .filter_map(|resource| {
            let distance = (player.x - resource.x).hypot(player.y - resource.y);
            (distance <= 64.0).then_some((distance, resource.resource_id))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, resource_id)| resource_id)
}

fn world_camera(player: PlayerSnapshot) -> Camera2D {
    let width = screen_width().max(1.0);
    let height = screen_height().max(1.0);
    let target_x = if width >= WORLD_SIZE {
        WORLD_SIZE * 0.5
    } else {
        player.x.clamp(width * 0.5, WORLD_SIZE - width * 0.5)
    };
    let target_y = if height >= WORLD_SIZE {
        WORLD_SIZE * 0.5
    } else {
        player.y.clamp(height * 0.5, WORLD_SIZE - height * 0.5)
    };

    Camera2D {
        target: vec2(target_x, target_y),
        // Direct-to-window cameras invert Y internally; use a positive Y zoom
        // so world coordinates keep the same top-left origin as the default
        // Macroquad screen (and the server's movement coordinates).
        zoom: vec2(2.0 / width, 2.0 / height),
        ..Default::default()
    }
}

fn draw_resources(resources: &[ResourceSnapshot], player: PlayerSnapshot) {
    for resource in resources {
        if resource.remaining == 0 {
            continue;
        }
        let fill = match resource.kind {
            ResourceKind::Wood => DARKGREEN,
            ResourceKind::Stone => GRAY,
            ResourceKind::Berries => PURPLE,
        };
        draw_circle(resource.x, resource.y, 12.0, fill);
        draw_circle_lines(resource.x, resource.y, 12.0, 2.0, WHITE);
        draw_text(
            &resource.remaining.to_string(),
            resource.x - 5.0,
            resource.y + 5.0,
            14.0,
            WHITE,
        );
        if (player.x - resource.x).hypot(player.y - resource.y) <= 64.0 {
            draw_text("E", resource.x - 5.0, resource.y - 18.0, 16.0, YELLOW);
        }
    }
}

fn draw_inventory(inventory: &[InventoryStack]) {
    let mut text = String::from("Inventory:");
    for stack in inventory {
        let name = match stack.kind {
            ResourceKind::Wood => "wood",
            ResourceKind::Stone => "stone",
            ResourceKind::Berries => "berries",
        };
        text.push_str(&format!(" {name} {}", stack.quantity));
    }
    draw_text(&text, 24.0, screen_height() - 48.0, 18.0, LIGHTGRAY);
}

fn draw_vitals(health: f32, stamina: f32, hunger: f32) {
    let x = screen_width() - 220.0;
    draw_stat_bar(x, 28.0, "Health", health, RED);
    draw_stat_bar(x, 52.0, "Stamina", stamina, GREEN);
    draw_stat_bar(x, 76.0, "Hunger", hunger, GOLD);
}

fn draw_stat_bar(x: f32, y: f32, label: &str, value: f32, color: Color) {
    let width = 190.0;
    draw_text(label, x, y + 14.0, 16.0, WHITE);
    draw_rectangle(x + 58.0, y, width - 58.0, 14.0, DARKGRAY);
    draw_rectangle(
        x + 58.0,
        y,
        (width - 58.0) * (value / 100.0).clamp(0.0, 1.0),
        14.0,
        color,
    );
}

fn draw_crafting_panel(inventory: &[InventoryStack], crafted_kits: u16) {
    let panel_width = 390.0;
    let panel_height = 180.0;
    let x = (screen_width() - panel_width) * 0.5;
    let y = (screen_height() - panel_height) * 0.5;
    draw_rectangle(
        x,
        y,
        panel_width,
        panel_height,
        Color::from_rgba(20, 25, 30, 240),
    );
    draw_rectangle_lines(x, y, panel_width, panel_height, 2.0, SKYBLUE);
    draw_text("CRAFTING", x + 24.0, y + 38.0, 26.0, WHITE);
    draw_text("Camp kit", x + 24.0, y + 78.0, 21.0, LIGHTGRAY);
    draw_text("3 wood + 2 stone", x + 24.0, y + 106.0, 18.0, LIGHTGRAY);
    draw_text(
        &format!("Crafted: {crafted_kits}   Press Enter to craft"),
        x + 24.0,
        y + 140.0,
        17.0,
        if has_camp_kit_materials(inventory) {
            GREEN
        } else {
            ORANGE
        },
    );
    draw_text("C closes panel", x + 24.0, y + 166.0, 15.0, GRAY);
}

fn has_camp_kit_materials(inventory: &[InventoryStack]) -> bool {
    inventory_quantity(inventory, ResourceKind::Wood) >= 3
        && inventory_quantity(inventory, ResourceKind::Stone) >= 2
}

fn inventory_quantity(inventory: &[InventoryStack], kind: ResourceKind) -> u16 {
    inventory
        .iter()
        .find(|stack| stack.kind == kind)
        .map(|stack| stack.quantity)
        .unwrap_or(0)
}

fn update_remote_players(
    remote_players: &mut Vec<RemotePlayer>,
    snapshots: &[PlayerSnapshot],
    local_player_id: u64,
) {
    remote_players.retain(|player| {
        snapshots.iter().any(|snapshot| {
            snapshot.player_id == player.player_id && snapshot.player_id != local_player_id
        })
    });

    for snapshot in snapshots {
        if snapshot.player_id == local_player_id {
            continue;
        }
        if let Some(player) = remote_players
            .iter_mut()
            .find(|player| player.player_id == snapshot.player_id)
        {
            player.target_x = snapshot.x;
            player.target_y = snapshot.y;
        } else {
            remote_players.push(RemotePlayer {
                player_id: snapshot.player_id,
                x: snapshot.x,
                y: snapshot.y,
                target_x: snapshot.x,
                target_y: snapshot.y,
            });
        }
    }
}

fn draw_world(world: &WorldAsset) {
    draw_rectangle(0.0, 0.0, WORLD_SIZE, WORLD_SIZE, color(world.ground));

    for [x, y] in &world.dots {
        draw_circle(*x, *y, 5.0, color(world.dot));
    }

    for decoration in &world.decorations {
        let fill = color(decoration.color);
        match decoration.kind.as_str() {
            "water" | "water_inner" => {
                draw_circle(decoration.x, decoration.y, decoration.w, fill);
            }
            _ => draw_rectangle(decoration.x, decoration.y, decoration.w, decoration.h, fill),
        }
    }
}

fn color(value: [u8; 4]) -> Color {
    Color::from_rgba(value[0], value[1], value[2], value[3])
}

fn draw_loading_screen() {
    clear_background(Color::from_rgba(17, 29, 39, 255));
    draw_text("FRONTIER ECHOES", 250.0, 220.0, 36.0, WHITE);
    draw_text(
        "Loading assets from server...",
        230.0,
        285.0,
        24.0,
        LIGHTGRAY,
    );
    draw_text("Connecting to asset service", 260.0, 320.0, 18.0, SKYBLUE);
}

fn draw_asset_error(error: &str) {
    clear_background(Color::from_rgba(17, 29, 39, 255));
    draw_text("FRONTIER ECHOES", 250.0, 200.0, 36.0, WHITE);
    draw_text(
        "Unable to load assets from server",
        190.0,
        270.0,
        24.0,
        ORANGE,
    );
    draw_text("Press R to retry", 300.0, 315.0, 20.0, WHITE);
    draw_text(error, 40.0, 370.0, 16.0, LIGHTGRAY);
}

fn handle_login_input(
    username: &mut String,
    password: &mut String,
    field: &mut AuthField,
    mode: &mut AuthMode,
    receiver: &mut Option<Receiver<Result<String, String>>>,
    error: &mut Option<String>,
) {
    if is_key_pressed(KeyCode::Tab) {
        *field = match field {
            AuthField::Username => AuthField::Password,
            AuthField::Password => AuthField::Username,
        };
    }
    if is_key_pressed(KeyCode::F2) {
        *mode = match mode {
            AuthMode::Login => AuthMode::Signup,
            AuthMode::Signup => AuthMode::Login,
        };
        *error = None;
    }
    if is_key_pressed(KeyCode::Backspace) {
        match field {
            AuthField::Username => {
                username.pop();
            }
            AuthField::Password => {
                password.pop();
            }
        }
    }
    while let Some(character) = get_char_pressed() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ' ' | '.') {
            match field {
                AuthField::Username => username.push(character),
                AuthField::Password => password.push(character),
            }
            *error = None;
        }
    }
    if is_key_pressed(KeyCode::Enter) && receiver.is_none() {
        let action = match mode {
            AuthMode::Login => AuthAction::Login,
            AuthMode::Signup => AuthAction::Signup,
        };
        *receiver = Some(spawn_auth_request(
            action,
            username.clone(),
            password.clone(),
        ));
    }
}

fn draw_login_screen(
    username: &str,
    password: &str,
    field: AuthField,
    mode: AuthMode,
    error: Option<&str>,
    busy: bool,
) {
    clear_background(Color::from_rgba(17, 29, 39, 255));
    draw_text("FRONTIER ECHOES", 250.0, 120.0, 36.0, WHITE);
    draw_text(
        match mode {
            AuthMode::Login => "Sign in",
            AuthMode::Signup => "Create account",
        },
        320.0,
        185.0,
        28.0,
        SKYBLUE,
    );
    draw_text("Username", 230.0, 245.0, 20.0, LIGHTGRAY);
    draw_text(username, 230.0, 275.0, 22.0, WHITE);
    draw_line(
        230.0,
        282.0,
        570.0,
        282.0,
        2.0,
        if matches!(field, AuthField::Username) {
            SKYBLUE
        } else {
            GRAY
        },
    );
    draw_text("Password", 230.0, 330.0, 20.0, LIGHTGRAY);
    draw_text(
        &"•".repeat(password.chars().count()),
        230.0,
        360.0,
        22.0,
        WHITE,
    );
    draw_line(
        230.0,
        367.0,
        570.0,
        367.0,
        2.0,
        if matches!(field, AuthField::Password) {
            SKYBLUE
        } else {
            GRAY
        },
    );
    draw_text(
        "Tab: switch field   Enter: submit   F2: switch login/signup",
        180.0,
        430.0,
        16.0,
        LIGHTGRAY,
    );
    if busy {
        draw_text("Contacting account service...", 270.0, 475.0, 18.0, SKYBLUE);
    }
    if let Some(error) = error {
        draw_text(error, 160.0, 510.0, 18.0, ORANGE);
    }
}

#[derive(Clone, Copy)]
enum AuthAction {
    Login,
    Signup,
}

fn spawn_auth_request(
    action: AuthAction,
    username: String,
    password: String,
) -> Receiver<Result<String, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = authenticate(action, &username, &password);
        let _ = sender.send(result);
    });
    receiver
}

fn authenticate(action: AuthAction, username: &str, password: &str) -> Result<String, String> {
    let path = match action {
        AuthAction::Login => "/auth/login",
        AuthAction::Signup => "/auth/signup",
    };
    let body = serde_json::json!({ "username": username, "password": password }).to_string();
    let mut stream = TcpStream::connect(AUTH_API_ADDRESS).map_err(|error| error.to_string())?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: frontier-auth\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| error.to_string())?;
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "invalid authentication response".to_owned())?;
    let headers = std::str::from_utf8(&response[..header_end])
        .map_err(|_| "authentication response was not valid UTF-8".to_owned())?;
    let body = &response[header_end + 4..];
    let json: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| "authentication service returned invalid JSON".to_owned())?;
    if !headers.starts_with("HTTP/1.1 200") && !headers.starts_with("HTTP/1.1 201") {
        return Err(json["error"]
            .as_str()
            .unwrap_or("authentication failed")
            .to_owned());
    }
    json["token"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "authentication response did not include a session token".to_owned())
}

fn spawn_asset_download() -> Receiver<Result<WorldAsset, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = download_world_asset().map_err(|error| error.to_string());
        let _ = sender.send(result);
    });
    receiver
}

fn connect_game_socket() -> Result<UdpSocket, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| error.to_string())?;
    socket
        .connect(SERVER_ADDRESS)
        .map_err(|error| error.to_string())?;
    socket
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    Ok(socket)
}

fn download_world_asset() -> Result<WorldAsset, Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(ASSET_SERVER_ADDRESS)?;
    stream.write_all(
        b"GET /assets/world.json HTTP/1.1\r\nHost: frontier-server\r\nConnection: close\r\n\r\n",
    )?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;

    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("invalid asset response")?;
    let headers = std::str::from_utf8(&response[..header_end])?;
    if !headers.starts_with("HTTP/1.1 200") {
        return Err(format!("asset request failed: {headers}").into());
    }

    Ok(serde_json::from_slice(&response[header_end + 4..])?)
}
