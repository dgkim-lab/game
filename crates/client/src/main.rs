use frontier_shared::{PlayerInput, PlayerSnapshot};
use macroquad::prelude::*;
use std::net::UdpSocket;

const SERVER_ADDRESS: &str = "127.0.0.1:4000";
const WORLD_SIZE: f32 = 800.0;

#[macroquad::main("Frontier Echoes")]
async fn main() {
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
    let mut receive_buffer = [0; PlayerSnapshot::BYTE_LEN];

    loop {
        let input = PlayerInput {
            move_x: i8::from(is_key_down(KeyCode::D)) - i8::from(is_key_down(KeyCode::A)),
            move_y: i8::from(is_key_down(KeyCode::S)) - i8::from(is_key_down(KeyCode::W)),
        };
        let _ = socket.send(&input.encode());

        while let Ok(size) = socket.recv(&mut receive_buffer) {
            if let Some(snapshot) = PlayerSnapshot::decode(&receive_buffer[..size]) {
                player = snapshot;
            }
        }

        clear_background(Color::from_rgba(17, 29, 39, 255));
        draw_world();
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

fn draw_world() {
    draw_rectangle(
        0.0,
        0.0,
        WORLD_SIZE,
        WORLD_SIZE,
        Color::from_rgba(45, 94, 65, 255),
    );

    for x in (48..=752).step_by(64) {
        for y in (48..=752).step_by(64) {
            draw_circle(x as f32, y as f32, 5.0, Color::from_rgba(65, 126, 76, 180));
        }
    }

    draw_rectangle(
        170.0,
        180.0,
        120.0,
        90.0,
        Color::from_rgba(115, 78, 49, 255),
    );
    draw_rectangle(195.0, 205.0, 70.0, 65.0, Color::from_rgba(76, 49, 36, 255));
    draw_rectangle(
        550.0,
        480.0,
        140.0,
        80.0,
        Color::from_rgba(101, 72, 51, 255),
    );
    draw_circle(625.0, 190.0, 34.0, Color::from_rgba(33, 110, 136, 255));
    draw_circle(625.0, 190.0, 25.0, Color::from_rgba(47, 143, 165, 255));
}
