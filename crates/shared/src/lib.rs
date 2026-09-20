//! Types shared by the Frontier Echoes client and server.

/// Stable identifier for a connected player.
pub type PlayerId = u64;

/// Input accepted by the authoritative simulation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerInput {
    pub move_x: i8,
    pub move_y: i8,
}

/// Minimal state replicated from the server to a client.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlayerSnapshot {
    pub player_id: PlayerId,
    pub x: f32,
    pub y: f32,
}

impl PlayerInput {
    pub const BYTE_LEN: usize = 2;

    pub fn encode(self) -> [u8; Self::BYTE_LEN] {
        [self.move_x as u8, self.move_y as u8]
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::BYTE_LEN {
            return None;
        }

        Some(Self {
            move_x: bytes[0] as i8,
            move_y: bytes[1] as i8,
        })
    }
}

impl PlayerSnapshot {
    pub const BYTE_LEN: usize = 16;

    pub fn encode(self) -> [u8; Self::BYTE_LEN] {
        let mut bytes = [0; Self::BYTE_LEN];
        bytes[0..8].copy_from_slice(&self.player_id.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.x.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.y.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::BYTE_LEN {
            return None;
        }

        Some(Self {
            player_id: u64::from_le_bytes(bytes[0..8].try_into().ok()?),
            x: f32::from_le_bytes(bytes[8..12].try_into().ok()?),
            y: f32::from_le_bytes(bytes[12..16].try_into().ok()?),
        })
    }
}
