//! Types shared by the Frontier Echoes client and server.

use serde::Deserialize;

/// Stable identifier for a connected player.
pub type PlayerId = u64;

const BUILDING_SECTION_MAGIC: [u8; 2] = *b"BL";

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

pub const PROTOCOL_VERSION: u8 = 2;
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildingSnapshot {
    pub building_id: u32,
    pub kind: BuildingKind,
    pub owner_id: PlayerId,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BuildingKind {
    Wall = 1,
    Floor = 2,
    Storage = 3,
    Campfire = 4,
}

impl TryFrom<u8> for BuildingKind {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Wall),
            2 => Ok(Self::Floor),
            3 => Ok(Self::Storage),
            4 => Ok(Self::Campfire),
            _ => Err(DecodeError::InvalidPayload),
        }
    }
}

impl BuildingSnapshot {
    pub const BYTE_LEN: usize = 21;

    fn encode(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.building_id.to_le_bytes());
        bytes.push(self.kind as u8);
        bytes.extend_from_slice(&self.owner_id.to_le_bytes());
        bytes.extend_from_slice(&self.x.to_le_bytes());
        bytes.extend_from_slice(&self.y.to_le_bytes());
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::BYTE_LEN {
            return None;
        }
        Some(Self {
            building_id: u32::from_le_bytes(bytes[0..4].try_into().ok()?),
            kind: BuildingKind::try_from(bytes[4]).ok()?,
            owner_id: u64::from_le_bytes(bytes[5..13].try_into().ok()?),
            x: f32::from_le_bytes(bytes[13..17].try_into().ok()?),
            y: f32::from_le_bytes(bytes[17..21].try_into().ok()?),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerVitals {
    pub health: f32,
    pub stamina: f32,
    pub hunger: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemySnapshot {
    pub enemy_id: u32,
    pub x: f32,
    pub y: f32,
    pub health: f32,
}

impl EnemySnapshot {
    pub const BYTE_LEN: usize = 16;

    fn encode(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.enemy_id.to_le_bytes());
        bytes.extend_from_slice(&self.x.to_le_bytes());
        bytes.extend_from_slice(&self.y.to_le_bytes());
        bytes.extend_from_slice(&self.health.to_le_bytes());
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::BYTE_LEN {
            return None;
        }
        Some(Self {
            enemy_id: u32::from_le_bytes(bytes[0..4].try_into().ok()?),
            x: f32::from_le_bytes(bytes[4..8].try_into().ok()?),
            y: f32::from_le_bytes(bytes[8..12].try_into().ok()?),
            health: f32::from_le_bytes(bytes[12..16].try_into().ok()?),
        })
    }
}

impl Default for PlayerVitals {
    fn default() -> Self {
        Self {
            health: 100.0,
            stamina: 100.0,
            hunger: 100.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResourceKind {
    Wood = 1,
    Stone = 2,
    Berries = 3,
}

impl TryFrom<u8> for ResourceKind {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Wood),
            2 => Ok(Self::Stone),
            3 => Ok(Self::Berries),
            _ => Err(DecodeError::InvalidPayload),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CraftRecipe {
    CampKit = 1,
}

impl TryFrom<u8> for CraftRecipe {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::CampKit),
            _ => Err(DecodeError::InvalidPayload),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceSnapshot {
    pub resource_id: u32,
    pub kind: ResourceKind,
    pub x: f32,
    pub y: f32,
    pub remaining: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryStack {
    pub kind: ResourceKind,
    pub quantity: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    ClientInput = 1,
    ServerSnapshot = 2,
    ClientAuthenticate = 3,
    ClientInteract = 4,
    ClientCraft = 5,
    ClientConsume = 6,
    ClientAttack = 7,
    ClientBuild = 8,
}

impl TryFrom<u8> for MessageType {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::ClientInput),
            2 => Ok(Self::ServerSnapshot),
            3 => Ok(Self::ClientAuthenticate),
            4 => Ok(Self::ClientInteract),
            5 => Ok(Self::ClientCraft),
            6 => Ok(Self::ClientConsume),
            7 => Ok(Self::ClientAttack),
            8 => Ok(Self::ClientBuild),
            _ => Err(DecodeError::UnknownMessageType(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClientMessage {
    Input {
        sequence: u32,
        input: PlayerInput,
    },
    Authenticate {
        sequence: u32,
        token: String,
    },
    Interact {
        sequence: u32,
        resource_id: u32,
    },
    Craft {
        sequence: u32,
        recipe: CraftRecipe,
    },
    Consume {
        sequence: u32,
        kind: ResourceKind,
    },
    Attack {
        sequence: u32,
    },
    Build {
        sequence: u32,
        kind: BuildingKind,
        x: f32,
        y: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServerMessage {
    pub sequence: u32,
    pub player_id: PlayerId,
    pub snapshots: Vec<PlayerSnapshot>,
    pub resources: Vec<ResourceSnapshot>,
    pub inventory: Vec<InventoryStack>,
    pub crafted_kits: u16,
    pub vitals: PlayerVitals,
    pub enemies: Vec<EnemySnapshot>,
    pub buildings: Vec<BuildingSnapshot>,
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

    pub fn is_valid(self) -> bool {
        (-1..=1).contains(&self.move_x) && (-1..=1).contains(&self.move_y)
    }

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

impl ResourceSnapshot {
    pub const BYTE_LEN: usize = 15;

    fn encode(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.resource_id.to_le_bytes());
        bytes.push(self.kind as u8);
        bytes.extend_from_slice(&self.x.to_le_bytes());
        bytes.extend_from_slice(&self.y.to_le_bytes());
        bytes.extend_from_slice(&self.remaining.to_le_bytes());
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::BYTE_LEN {
            return None;
        }
        Some(Self {
            resource_id: u32::from_le_bytes(bytes[0..4].try_into().ok()?),
            kind: ResourceKind::try_from(bytes[4]).ok()?,
            x: f32::from_le_bytes(bytes[5..9].try_into().ok()?),
            y: f32::from_le_bytes(bytes[9..13].try_into().ok()?),
            remaining: u16::from_le_bytes(bytes[13..15].try_into().ok()?),
        })
    }
}

impl InventoryStack {
    pub const BYTE_LEN: usize = 3;

    fn encode(self, bytes: &mut Vec<u8>) {
        bytes.push(self.kind as u8);
        bytes.extend_from_slice(&self.quantity.to_le_bytes());
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::BYTE_LEN {
            return None;
        }
        Some(Self {
            kind: ResourceKind::try_from(bytes[0]).ok()?,
            quantity: u16::from_le_bytes(bytes[1..3].try_into().ok()?),
        })
    }
}

pub fn encode_client_input(sequence: u32, input: PlayerInput) -> Vec<u8> {
    encode_message(MessageType::ClientInput, sequence, &input.encode())
}

pub fn encode_client_authenticate(sequence: u32, token: &str) -> Vec<u8> {
    encode_message(MessageType::ClientAuthenticate, sequence, token.as_bytes())
}

pub fn encode_client_interact(sequence: u32, resource_id: u32) -> Vec<u8> {
    encode_message(
        MessageType::ClientInteract,
        sequence,
        &resource_id.to_le_bytes(),
    )
}

pub fn encode_client_craft(sequence: u32, recipe: CraftRecipe) -> Vec<u8> {
    encode_message(MessageType::ClientCraft, sequence, &[recipe as u8])
}

pub fn encode_client_consume(sequence: u32, kind: ResourceKind) -> Vec<u8> {
    encode_message(MessageType::ClientConsume, sequence, &[kind as u8])
}

pub fn encode_client_attack(sequence: u32) -> Vec<u8> {
    encode_message(MessageType::ClientAttack, sequence, &[])
}

pub fn encode_client_build(sequence: u32, kind: BuildingKind, x: f32, y: f32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(9);
    payload.push(kind as u8);
    payload.extend_from_slice(&x.to_le_bytes());
    payload.extend_from_slice(&y.to_le_bytes());
    encode_message(MessageType::ClientBuild, sequence, &payload)
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
        MessageType::ClientInteract => {
            if payload.len() != 4 {
                return Err(DecodeError::InvalidPayload);
            }
            Ok(ClientMessage::Interact {
                sequence,
                resource_id: u32::from_le_bytes(
                    payload
                        .try_into()
                        .map_err(|_| DecodeError::InvalidPayload)?,
                ),
            })
        }
        MessageType::ClientCraft => {
            if payload.len() != 1 {
                return Err(DecodeError::InvalidPayload);
            }
            Ok(ClientMessage::Craft {
                sequence,
                recipe: CraftRecipe::try_from(payload[0])?,
            })
        }
        MessageType::ClientConsume => {
            if payload.len() != 1 {
                return Err(DecodeError::InvalidPayload);
            }
            Ok(ClientMessage::Consume {
                sequence,
                kind: ResourceKind::try_from(payload[0])?,
            })
        }
        MessageType::ClientAttack => {
            if !payload.is_empty() {
                return Err(DecodeError::InvalidPayload);
            }
            Ok(ClientMessage::Attack { sequence })
        }
        MessageType::ClientBuild => {
            if payload.len() != 9 {
                return Err(DecodeError::InvalidPayload);
            }
            Ok(ClientMessage::Build {
                sequence,
                kind: BuildingKind::try_from(payload[0])?,
                x: f32::from_le_bytes(
                    payload[1..5]
                        .try_into()
                        .map_err(|_| DecodeError::InvalidPayload)?,
                ),
                y: f32::from_le_bytes(
                    payload[5..9]
                        .try_into()
                        .map_err(|_| DecodeError::InvalidPayload)?,
                ),
            })
        }
        MessageType::ServerSnapshot => Err(DecodeError::UnknownMessageType(message_type as u8)),
    }
}

pub fn encode_server_snapshot(
    sequence: u32,
    player_id: PlayerId,
    snapshots: &[PlayerSnapshot],
) -> Vec<u8> {
    encode_server_state(sequence, player_id, snapshots, &[], &[])
}

pub fn encode_server_state(
    sequence: u32,
    player_id: PlayerId,
    snapshots: &[PlayerSnapshot],
    resources: &[ResourceSnapshot],
    inventory: &[InventoryStack],
) -> Vec<u8> {
    encode_server_state_with_crafting_and_vitals(
        sequence,
        player_id,
        snapshots,
        resources,
        inventory,
        0,
        PlayerVitals::default(),
        &[],
    )
}

pub fn encode_server_state_with_crafting(
    sequence: u32,
    player_id: PlayerId,
    snapshots: &[PlayerSnapshot],
    resources: &[ResourceSnapshot],
    inventory: &[InventoryStack],
    crafted_kits: u16,
) -> Vec<u8> {
    encode_server_state_with_crafting_and_vitals(
        sequence,
        player_id,
        snapshots,
        resources,
        inventory,
        crafted_kits,
        PlayerVitals::default(),
        &[],
    )
}

pub fn encode_server_state_with_crafting_and_vitals(
    sequence: u32,
    player_id: PlayerId,
    snapshots: &[PlayerSnapshot],
    resources: &[ResourceSnapshot],
    inventory: &[InventoryStack],
    crafted_kits: u16,
    vitals: PlayerVitals,
    enemies: &[EnemySnapshot],
) -> Vec<u8> {
    encode_server_state_with_buildings(
        sequence,
        player_id,
        snapshots,
        resources,
        inventory,
        crafted_kits,
        vitals,
        enemies,
        &[],
    )
}

pub fn encode_server_state_with_buildings(
    sequence: u32,
    player_id: PlayerId,
    snapshots: &[PlayerSnapshot],
    resources: &[ResourceSnapshot],
    inventory: &[InventoryStack],
    crafted_kits: u16,
    vitals: PlayerVitals,
    enemies: &[EnemySnapshot],
    buildings: &[BuildingSnapshot],
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(
        12 + snapshots.len() * PlayerSnapshot::BYTE_LEN
            + resources.len() * ResourceSnapshot::BYTE_LEN
            + inventory.len() * InventoryStack::BYTE_LEN
            + enemies.len() * EnemySnapshot::BYTE_LEN
            + buildings.len() * BuildingSnapshot::BYTE_LEN,
    );
    payload.extend_from_slice(&player_id.to_le_bytes());
    payload.extend_from_slice(&(resources.len() as u16).to_le_bytes());
    for resource in resources {
        resource.encode(&mut payload);
    }
    payload.extend_from_slice(&(inventory.len() as u16).to_le_bytes());
    for stack in inventory {
        stack.encode(&mut payload);
    }
    payload.extend_from_slice(&crafted_kits.to_le_bytes());
    payload.extend_from_slice(&vitals.health.to_le_bytes());
    payload.extend_from_slice(&vitals.stamina.to_le_bytes());
    payload.extend_from_slice(&vitals.hunger.to_le_bytes());
    payload.extend_from_slice(&(enemies.len() as u16).to_le_bytes());
    for enemy in enemies {
        enemy.encode(&mut payload);
    }
    payload.extend_from_slice(&BUILDING_SECTION_MAGIC);
    payload.extend_from_slice(&(buildings.len() as u16).to_le_bytes());
    for building in buildings {
        building.encode(&mut payload);
    }
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

    if payload.len() < 12 {
        return Err(DecodeError::InvalidPayload);
    }
    let player_id = u64::from_le_bytes(
        payload[0..8]
            .try_into()
            .map_err(|_| DecodeError::InvalidPayload)?,
    );

    let mut offset = 8;
    let resource_count = usize::from(u16::from_le_bytes(
        payload[offset..offset + 2]
            .try_into()
            .map_err(|_| DecodeError::InvalidPayload)?,
    ));
    offset += 2;

    let resources_end = offset + resource_count * ResourceSnapshot::BYTE_LEN;
    if resources_end > payload.len() {
        return Err(DecodeError::InvalidPayload);
    }
    let resources = payload[offset..resources_end]
        .chunks_exact(ResourceSnapshot::BYTE_LEN)
        .map(|chunk| ResourceSnapshot::decode(chunk).ok_or(DecodeError::InvalidPayload))
        .collect::<Result<Vec<_>, _>>()?;
    offset = resources_end;

    if offset + 2 > payload.len() {
        return Err(DecodeError::InvalidPayload);
    }
    let inventory_count = usize::from(u16::from_le_bytes(
        payload[offset..offset + 2]
            .try_into()
            .map_err(|_| DecodeError::InvalidPayload)?,
    ));
    offset += 2;
    let inventory_end = offset + inventory_count * InventoryStack::BYTE_LEN;
    if inventory_end > payload.len() {
        return Err(DecodeError::InvalidPayload);
    }
    let inventory = payload[offset..inventory_end]
        .chunks_exact(InventoryStack::BYTE_LEN)
        .map(|chunk| InventoryStack::decode(chunk).ok_or(DecodeError::InvalidPayload))
        .collect::<Result<Vec<_>, _>>()?;
    offset = inventory_end;

    if offset + 2 > payload.len() {
        return Err(DecodeError::InvalidPayload);
    }
    let crafted_kits = u16::from_le_bytes(
        payload[offset..offset + 2]
            .try_into()
            .map_err(|_| DecodeError::InvalidPayload)?,
    );
    offset += 2;

    if offset + 12 > payload.len() {
        return Err(DecodeError::InvalidPayload);
    }
    let vitals = PlayerVitals {
        health: f32::from_le_bytes(
            payload[offset..offset + 4]
                .try_into()
                .map_err(|_| DecodeError::InvalidPayload)?,
        ),
        stamina: f32::from_le_bytes(
            payload[offset + 4..offset + 8]
                .try_into()
                .map_err(|_| DecodeError::InvalidPayload)?,
        ),
        hunger: f32::from_le_bytes(
            payload[offset + 8..offset + 12]
                .try_into()
                .map_err(|_| DecodeError::InvalidPayload)?,
        ),
    };
    offset += 12;

    if offset + 2 > payload.len() {
        return Err(DecodeError::InvalidPayload);
    }
    let enemy_count = usize::from(u16::from_le_bytes(
        payload[offset..offset + 2]
            .try_into()
            .map_err(|_| DecodeError::InvalidPayload)?,
    ));
    offset += 2;
    let enemies_end = offset + enemy_count * EnemySnapshot::BYTE_LEN;
    if enemies_end > payload.len() {
        return Err(DecodeError::InvalidPayload);
    }
    let enemies = payload[offset..enemies_end]
        .chunks_exact(EnemySnapshot::BYTE_LEN)
        .map(|chunk| EnemySnapshot::decode(chunk).ok_or(DecodeError::InvalidPayload))
        .collect::<Result<Vec<_>, _>>()?;
    offset = enemies_end;

    let (buildings, snapshots_offset) =
        if payload.len() >= offset + 4 && payload[offset..offset + 2] == BUILDING_SECTION_MAGIC {
            offset += 2;
            if offset + 2 > payload.len() {
                return Err(DecodeError::InvalidPayload);
            }
            let building_count = usize::from(u16::from_le_bytes(
                payload[offset..offset + 2]
                    .try_into()
                    .map_err(|_| DecodeError::InvalidPayload)?,
            ));
            let buildings_start = offset + 2;
            let buildings_end = buildings_start + building_count * BuildingSnapshot::BYTE_LEN;
            if buildings_end > payload.len() {
                return Err(DecodeError::InvalidPayload);
            }
            let buildings = payload[buildings_start..buildings_end]
                .chunks_exact(BuildingSnapshot::BYTE_LEN)
                .map(|chunk| BuildingSnapshot::decode(chunk).ok_or(DecodeError::InvalidPayload))
                .collect::<Result<Vec<_>, _>>()?;
            (buildings, buildings_end)
        } else {
            // Servers predating building replication placed player snapshots here.
            (Vec::new(), offset)
        };

    if (payload.len() - snapshots_offset) % PlayerSnapshot::BYTE_LEN != 0 {
        return Err(DecodeError::InvalidPayload);
    }
    let snapshots = payload[snapshots_offset..]
        .chunks_exact(PlayerSnapshot::BYTE_LEN)
        .map(|chunk| PlayerSnapshot::decode(chunk).ok_or(DecodeError::InvalidPayload))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ServerMessage {
        sequence,
        player_id,
        snapshots,
        resources,
        inventory,
        crafted_kits,
        vitals,
        enemies,
        buildings,
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
    fn player_input_rejects_values_outside_direction_range() {
        assert!(PlayerInput {
            move_x: 1,
            move_y: -1
        }
        .is_valid());
        assert!(!PlayerInput {
            move_x: 2,
            move_y: 0
        }
        .is_valid());
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
    fn client_interaction_round_trips_through_versioned_envelope() {
        let packet = encode_client_interact(44, 9);
        let decoded = decode_client_message(&packet).expect("valid interaction packet");

        assert_eq!(
            decoded,
            ClientMessage::Interact {
                sequence: 44,
                resource_id: 9,
            }
        );
    }

    #[test]
    fn client_crafting_round_trips_through_versioned_envelope() {
        let packet = encode_client_craft(45, CraftRecipe::CampKit);
        let decoded = decode_client_message(&packet).expect("valid crafting packet");

        assert_eq!(
            decoded,
            ClientMessage::Craft {
                sequence: 45,
                recipe: CraftRecipe::CampKit,
            }
        );
    }

    #[test]
    fn client_consume_round_trips_through_versioned_envelope() {
        let packet = encode_client_consume(46, ResourceKind::Berries);
        let decoded = decode_client_message(&packet).expect("valid consume packet");

        assert_eq!(
            decoded,
            ClientMessage::Consume {
                sequence: 46,
                kind: ResourceKind::Berries,
            }
        );
    }

    #[test]
    fn client_build_round_trips_through_versioned_envelope() {
        let packet = encode_client_build(47, BuildingKind::Campfire, 320.0, 448.0);
        let decoded = decode_client_message(&packet).expect("valid build packet");

        assert_eq!(
            decoded,
            ClientMessage::Build {
                sequence: 47,
                kind: BuildingKind::Campfire,
                x: 320.0,
                y: 448.0,
            }
        );
    }

    #[test]
    fn server_state_round_trips_resources_and_inventory() {
        let resource = ResourceSnapshot {
            resource_id: 3,
            kind: ResourceKind::Stone,
            x: 30.0,
            y: 40.0,
            remaining: 5,
        };
        let inventory = InventoryStack {
            kind: ResourceKind::Wood,
            quantity: 2,
        };
        let packet = encode_server_state(12, 4, &[], &[resource], &[inventory]);
        let decoded = decode_server_message(&packet).expect("valid server state packet");

        assert_eq!(decoded.resources, vec![resource]);
        assert_eq!(decoded.inventory, vec![inventory]);
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
    fn server_state_round_trips_buildings() {
        let building = BuildingSnapshot {
            building_id: 4,
            kind: BuildingKind::Storage,
            owner_id: 7,
            x: 320.0,
            y: 448.0,
        };
        let packet = encode_server_state_with_buildings(
            99,
            7,
            &[],
            &[],
            &[],
            0,
            PlayerVitals::default(),
            &[],
            &[building],
        );
        let decoded = decode_server_message(&packet).expect("valid building state packet");

        assert_eq!(decoded.buildings, vec![building]);
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
