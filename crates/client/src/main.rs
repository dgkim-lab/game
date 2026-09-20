use frontier_shared::{
    decode_server_message, encode_client_input, PlayerInput, PlayerSnapshot, WorldAsset,
};
use macroquad::prelude::*;
use std::{
    io::{Read, Write},
    net::{TcpStream, UdpSocket},
};

const SERVER_ADDRESS: &str = "127.0.0.1:4000";
const ASSET_SERVER_ADDRESS: &str = "127.0.0.1:4100";
const WORLD_SIZE: f32 = 800.0;

#[macroquad::main("Frontier Echoes")]
async fn main() {
    let world = download_world_asset().unwrap_or_else(|error| {
        eprintln!("could not download world asset: {error}");
        std::process::exit(1);
    });
    let socket = UdpSocket::bind("0.0.0.0:0").expect("bind client socket");
    socket
        .connect(SERVER_ADDRESS)
        .expect("connect to game server");
    socket
        .set_nonblocking(true)
        .expect("configure non-blocking client socket");

    let mut player = PlayerSnapshot {
        player_id: 0,
        x: 400.0,
        y: 300.0,
    };
    let mut receive_buffer = [0; 256];
    let mut sequence = 0;

    loop {
        let input = PlayerInput {
            move_x: i8::from(is_key_down(KeyCode::D)) - i8::from(is_key_down(KeyCode::A)),
            move_y: i8::from(is_key_down(KeyCode::S)) - i8::from(is_key_down(KeyCode::W)),
        };
        let _ = socket.send(&encode_client_input(sequence, input));
        sequence = sequence.wrapping_add(1);

        while let Ok(size) = socket.recv(&mut receive_buffer) {
            if let Ok(message) = decode_server_message(&receive_buffer[..size]) {
                player = message.snapshot;
            }
        }

        clear_background(color(world.background));
        draw_world(&world);
        draw_circle(player.x, player.y, 16.0, SKYBLUE);
        draw_circle_lines(player.x, player.y, 16.0, 2.0, WHITE);

        draw_text("FRONTIER ECHOES", 24.0, 34.0, 28.0, WHITE);
        draw_text("WASD  Move", 24.0, 62.0, 20.0, LIGHTGRAY);
        draw_text("Connected to authoritative server", 24.0, 88.0, 18.0, GREEN);
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
