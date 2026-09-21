use frontier_shared::{
    decode_server_message, encode_client_input, PlayerInput, PlayerSnapshot, WorldAsset,
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
const WORLD_SIZE: f32 = 800.0;

#[macroquad::main("Frontier Echoes")]
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
    let mut receive_buffer = [0; 256];
    let mut sequence = 0;

    loop {
        if world.is_none() {
            match asset_receiver.try_recv() {
                Ok(Ok(asset)) => {
                    world = Some(asset);
                    asset_error = None;
                    match connect_game_socket() {
                        Ok(new_socket) => socket = Some(new_socket),
                        Err(error) => game_error = Some(error),
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

        let connection_lost = last_server_response
            .map(|time| time.elapsed() > Duration::from_secs(2))
            .unwrap_or(true);
        if is_key_pressed(KeyCode::R) && (game_error.is_some() || connection_lost) {
            match connect_game_socket() {
                Ok(new_socket) => {
                    socket = Some(new_socket);
                    game_error = None;
                    last_server_response = None;
                    sequence = 0;
                }
                Err(error) => game_error = Some(error),
            }
        }

        let input = PlayerInput {
            move_x: i8::from(is_key_down(KeyCode::D)) - i8::from(is_key_down(KeyCode::A)),
            move_y: i8::from(is_key_down(KeyCode::S)) - i8::from(is_key_down(KeyCode::W)),
        };
        if let Some(socket) = socket.as_ref() {
            if socket.send(&encode_client_input(sequence, input)).is_err() {
                game_error = Some("could not send input to game server".to_owned());
            }
            sequence = sequence.wrapping_add(1);

            while let Ok(size) = socket.recv(&mut receive_buffer) {
                if let Ok(message) = decode_server_message(&receive_buffer[..size]) {
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
                    other_players = message.snapshots;
                    last_server_response = Some(Instant::now());
                    game_error = None;
                }
            }
        } else {
            game_error = Some("game server connection is not available".to_owned());
        }

        let world = world.as_ref().expect("world loaded before gameplay loop");
        clear_background(color(world.background));
        draw_world(world);
        for other in &other_players {
            if other.player_id != player.player_id {
                draw_circle(other.x, other.y, 14.0, ORANGE);
                draw_circle_lines(other.x, other.y, 14.0, 2.0, WHITE);
            }
        }
        draw_circle(player.x, player.y, 16.0, SKYBLUE);
        draw_circle_lines(player.x, player.y, 16.0, 2.0, WHITE);

        draw_text("FRONTIER ECHOES", 24.0, 34.0, 28.0, WHITE);
        draw_text("WASD  Move", 24.0, 62.0, 20.0, LIGHTGRAY);
        draw_text(
            &format!("Players online: {}", other_players.len()),
            24.0,
            112.0,
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
        draw_text(connection_text, 24.0, 88.0, 18.0, connection_color);
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
