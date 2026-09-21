//! Types shared by the Frontier Echoes client and server.

use serde::Deserialize;

/// Stable identifier for a connected player.
pub type PlayerId = u64;

#[derive(Debug, Clone, Deserialize)]
pub struct WorldAsset {
    pub background: [u8; 4],
    pub ground: [u8; 4],
    pub dot: [u8; 4],
    pub dots: Vec<[f32; 2]>,
    pub decorations: Vec<WorldDecoration>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorldDecoration {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [u8; 4],
}

pub const PROTOCOL_VERSION: u8 = 1;
const MAGIC: [u8; 2] = *b"FE";
const HEADER_LEN: usize = 10;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    ClientInput = 1,
    ServerSnapshot = 2,
    ClientAuthenticate = 3,
}

impl TryFrom<u8> for MessageType {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::ClientInput),
            2 => Ok(Self::ServerSnapshot),
            3 => Ok(Self::ClientAuthenticate),
            _ => Err(DecodeError::UnknownMessageType(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientMessage {
    Input { sequence: u32, input: PlayerInput },
    Authenticate { sequence: u32, token: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServerMessage {
    pub sequence: u32,
    pub player_id: PlayerId,
    pub snapshots: Vec<PlayerSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    TooShort,
    InvalidMagic,
    UnsupportedVersion(u8),
    UnknownMessageType(u8),
    InvalidPayloadLength,
    InvalidPayload,
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

pub fn encode_client_input(sequence: u32, input: PlayerInput) -> Vec<u8> {
    encode_message(MessageType::ClientInput, sequence, &input.encode())
}

pub fn encode_client_authenticate(sequence: u32, token: &str) -> Vec<u8> {
    encode_message(MessageType::ClientAuthenticate, sequence, token.as_bytes())
}

pub fn decode_client_message(bytes: &[u8]) -> Result<ClientMessage, DecodeError> {
    let (message_type, sequence, payload) = decode_message(bytes)?;
    match message_type {
        MessageType::ClientInput => Ok(ClientMessage::Input {
            sequence,
            input: PlayerInput::decode(payload).ok_or(DecodeError::InvalidPayload)?,
        }),
        MessageType::ClientAuthenticate => {
            let token = std::str::from_utf8(payload)
                .map_err(|_| DecodeError::InvalidPayload)?
                .to_owned();
            if token.is_empty() {
                return Err(DecodeError::InvalidPayload);
            }
            Ok(ClientMessage::Authenticate { sequence, token })
        }
        MessageType::ServerSnapshot => Err(DecodeError::UnknownMessageType(message_type as u8)),
    }
}

pub fn encode_server_snapshot(
    sequence: u32,
    player_id: PlayerId,
    snapshots: &[PlayerSnapshot],
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8 + snapshots.len() * PlayerSnapshot::BYTE_LEN);
    payload.extend_from_slice(&player_id.to_le_bytes());
    for snapshot in snapshots {
        payload.extend_from_slice(&snapshot.encode());
    }
    encode_message(MessageType::ServerSnapshot, sequence, &payload)
}

pub fn decode_server_message(bytes: &[u8]) -> Result<ServerMessage, DecodeError> {
    let (message_type, sequence, payload) = decode_message(bytes)?;
    if message_type != MessageType::ServerSnapshot {
        return Err(DecodeError::UnknownMessageType(message_type as u8));
    }

    if payload.len() < 8 || (payload.len() - 8) % PlayerSnapshot::BYTE_LEN != 0 {
        return Err(DecodeError::InvalidPayload);
    }
    let player_id = u64::from_le_bytes(
        payload[0..8]
            .try_into()
            .map_err(|_| DecodeError::InvalidPayload)?,
    );

    let snapshots = payload[8..]
        .chunks_exact(PlayerSnapshot::BYTE_LEN)
        .map(|chunk| PlayerSnapshot::decode(chunk).ok_or(DecodeError::InvalidPayload))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ServerMessage {
        sequence,
        player_id,
        snapshots,
    })
}

fn encode_message(message_type: MessageType, sequence: u32, payload: &[u8]) -> Vec<u8> {
    let payload_len = u16::try_from(payload.len()).expect("protocol payload is too large");
    let mut bytes = Vec::with_capacity(HEADER_LEN + payload.len());
    bytes.extend_from_slice(&MAGIC);
    bytes.push(PROTOCOL_VERSION);
    bytes.push(message_type as u8);
    bytes.extend_from_slice(&sequence.to_le_bytes());
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

fn decode_message(bytes: &[u8]) -> Result<(MessageType, u32, &[u8]), DecodeError> {
    if bytes.len() < HEADER_LEN {
        return Err(DecodeError::TooShort);
    }
    if bytes[0..2] != MAGIC {
        return Err(DecodeError::InvalidMagic);
    }
    if bytes[2] != PROTOCOL_VERSION {
        return Err(DecodeError::UnsupportedVersion(bytes[2]));
    }

    let message_type = MessageType::try_from(bytes[3])?;
    let sequence = u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| DecodeError::TooShort)?);
    let payload_len = usize::from(u16::from_le_bytes(
        bytes[8..10].try_into().map_err(|_| DecodeError::TooShort)?,
    ));
    if bytes.len() != HEADER_LEN + payload_len {
        return Err(DecodeError::InvalidPayloadLength);
    }

    Ok((message_type, sequence, &bytes[HEADER_LEN..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_input_round_trips_through_versioned_envelope() {
        let input = PlayerInput {
            move_x: -1,
            move_y: 1,
        };
        let packet = encode_client_input(42, input);
        let decoded = decode_client_message(&packet).expect("valid client packet");

        assert_eq!(
            decoded,
            ClientMessage::Input {
                sequence: 42,
                input,
            }
        );
    }

    #[test]
    fn client_authentication_round_trips_through_versioned_envelope() {
        let packet = encode_client_authenticate(43, "session-token");
        let decoded = decode_client_message(&packet).expect("valid auth packet");

        assert_eq!(
            decoded,
            ClientMessage::Authenticate {
                sequence: 43,
                token: "session-token".to_owned(),
            }
        );
    }

    #[test]
    fn server_snapshot_round_trips_through_versioned_envelope() {
        let snapshot = PlayerSnapshot {
            player_id: 7,
            x: 12.5,
            y: 18.25,
        };
        let packet = encode_server_snapshot(99, 7, &[snapshot]);
        let decoded = decode_server_message(&packet).expect("valid server packet");

        assert_eq!(decoded.sequence, 99);
        assert_eq!(decoded.player_id, 7);
        assert_eq!(decoded.snapshots, vec![snapshot]);
    }

    #[test]
    fn server_snapshot_can_contain_multiple_players() {
        let snapshots = vec![
            PlayerSnapshot {
                player_id: 1,
                x: 12.5,
                y: 18.25,
            },
            PlayerSnapshot {
                player_id: 2,
                x: 40.0,
                y: 50.0,
            },
        ];
        let packet = encode_server_snapshot(99, 1, &snapshots);
        let decoded = decode_server_message(&packet).expect("valid world snapshot");

        assert_eq!(decoded.player_id, 1);
        assert_eq!(decoded.snapshots, snapshots);
    }

    #[test]
    fn decoder_rejects_wrong_version_and_trailing_bytes() {
        let mut packet = encode_client_input(1, PlayerInput::default());
        packet[2] = PROTOCOL_VERSION + 1;
        assert_eq!(
            decode_client_message(&packet),
            Err(DecodeError::UnsupportedVersion(PROTOCOL_VERSION + 1))
        );

        let mut packet = encode_client_input(1, PlayerInput::default());
        packet.push(0);
        assert_eq!(
            decode_client_message(&packet),
            Err(DecodeError::InvalidPayloadLength)
        );
    }
}
