use bevy_ecs::world::World;
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::components::{
    CausalAudit, CitizenMeta, Demographics, HouseholdRef, Kinship, MobilityProfile,
    OccupationProfile, PersonalFinances, PhysicalNeeds, SettlementRef,
};
use crate::events::EventRing;
use crate::household::HouseholdDirectory;
use crate::settlement::SettlementDirectory;
use crate::systems::demographics::NextCitizenId;
use crate::types::SimClock;
use crate::world::WorldMap;

pub const SAVE_MAGIC: &[u8; 8] = b"CIVITAS1";
pub const SAVE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitizenSnapshot {
    pub meta: CitizenMeta,
    pub demographics: Demographics,
    pub household_ref: HouseholdRef,
    pub settlement_ref: SettlementRef,
    pub occupation: OccupationProfile,
    pub finances: PersonalFinances,
    pub needs: PhysicalNeeds,
    pub mobility: MobilityProfile,
    pub kinship: Kinship,
    pub causal_audit: CausalAudit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationSnapshot {
    pub clock: SimClock,
    pub seed: u64,
    pub next_citizen_id: u64,
    pub world_map: WorldMap,
    pub settlements: SettlementDirectory,
    pub households: HouseholdDirectory,
    pub citizens: Vec<CitizenSnapshot>,
    pub events: EventRing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveHeader {
    pub magic: [u8; 8],
    pub schema_version: u32,
    pub engine_version: String,
    pub seed: u64,
    pub tick: u64,
    pub citizen_count: u64,
    pub payload_crc32: u32,
}

#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    Bincode(bincode::Error),
    InvalidMagic,
    IncompatibleVersion(u32),
    ChecksumMismatch { expected: u32, actual: u32 },
}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        SaveError::Io(e)
    }
}

impl From<bincode::Error> for SaveError {
    fn from(e: bincode::Error) -> Self {
        SaveError::Bincode(e)
    }
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "IO Error: {}", e),
            SaveError::Bincode(e) => write!(f, "Serialization Error: {}", e),
            SaveError::InvalidMagic => write!(f, "Invalid magic bytes in save file"),
            SaveError::IncompatibleVersion(v) => {
                write!(f, "Incompatible save schema version: {}", v)
            }
            SaveError::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "CRC32 Checksum mismatch! Expected: {:08x}, Actual: {:08x}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for SaveError {}

pub fn create_snapshot(world: &mut World, seed: u64) -> SimulationSnapshot {
    let clock = *world.get_resource::<SimClock>().unwrap();
    let next_id = world
        .get_resource::<NextCitizenId>()
        .map(|id| id.0)
        .unwrap_or(1);
    let world_map = world.get_resource::<WorldMap>().unwrap().clone();
    let settlements = world.get_resource::<SettlementDirectory>().unwrap().clone();
    let households = world.get_resource::<HouseholdDirectory>().unwrap().clone();
    let events = world.get_resource::<EventRing>().unwrap().clone();

    let mut query = world.query::<(
        &CitizenMeta,
        &Demographics,
        &HouseholdRef,
        &SettlementRef,
        &OccupationProfile,
        &PersonalFinances,
        &PhysicalNeeds,
        &MobilityProfile,
        &Kinship,
        &CausalAudit,
    )>();

    let mut citizens = Vec::new();
    for (m, d, hr, sr, o, f, n, mob, k, c) in query.iter(world) {
        citizens.push(CitizenSnapshot {
            meta: m.clone(),
            demographics: d.clone(),
            household_ref: hr.clone(),
            settlement_ref: sr.clone(),
            occupation: o.clone(),
            finances: f.clone(),
            needs: n.clone(),
            mobility: mob.clone(),
            kinship: k.clone(),
            causal_audit: c.clone(),
        });
    }

    SimulationSnapshot {
        clock,
        seed,
        next_citizen_id: next_id,
        world_map,
        settlements,
        households,
        citizens,
        events,
    }
}

pub fn save_to_file<P: AsRef<Path>>(
    world: &mut World,
    seed: u64,
    path: P,
) -> Result<(), SaveError> {
    let snapshot = create_snapshot(world, seed);
    let payload_bytes = bincode::serialize(&snapshot)?;

    let mut hasher = Hasher::new();
    hasher.update(&payload_bytes);
    let payload_crc32 = hasher.finalize();

    let header = SaveHeader {
        magic: *SAVE_MAGIC,
        schema_version: SAVE_SCHEMA_VERSION,
        engine_version: crate::core_version().to_string(),
        seed,
        tick: snapshot.clock.tick,
        citizen_count: snapshot.citizens.len() as u64,
        payload_crc32,
    };

    let header_bytes = bincode::serialize(&header)?;
    let header_len = header_bytes.len() as u32;

    let mut file = File::create(path)?;
    file.write_all(SAVE_MAGIC)?;
    file.write_all(&header_len.to_le_bytes())?;
    file.write_all(&header_bytes)?;
    file.write_all(&payload_bytes)?;
    file.flush()?;

    Ok(())
}

pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<SimulationSnapshot, SaveError> {
    let mut file = File::open(path)?;
    let mut magic_buf = [0u8; 8];
    file.read_exact(&mut magic_buf)?;
    if &magic_buf != SAVE_MAGIC {
        return Err(SaveError::InvalidMagic);
    }

    let mut len_buf = [0u8; 4];
    file.read_exact(&mut len_buf)?;
    let header_len = u32::from_le_bytes(len_buf) as usize;

    let mut header_buf = vec![0u8; header_len];
    file.read_exact(&mut header_buf)?;
    let header: SaveHeader = bincode::deserialize(&header_buf)?;

    if header.schema_version != SAVE_SCHEMA_VERSION {
        return Err(SaveError::IncompatibleVersion(header.schema_version));
    }

    let mut payload_bytes = Vec::new();
    file.read_to_end(&mut payload_bytes)?;

    let mut hasher = Hasher::new();
    hasher.update(&payload_bytes);
    let actual_crc32 = hasher.finalize();

    if actual_crc32 != header.payload_crc32 {
        return Err(SaveError::ChecksumMismatch {
            expected: header.payload_crc32,
            actual: actual_crc32,
        });
    }

    let snapshot: SimulationSnapshot = bincode::deserialize(&payload_bytes)?;
    Ok(snapshot)
}
