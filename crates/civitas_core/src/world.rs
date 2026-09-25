use bevy_ecs::system::Resource;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::types::{Biome, SettlementId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
    pub biome: Biome,
    pub elevation: u8,
    pub moisture: u8,
    pub fertility: u8,
    pub timber_richness: u8,
    pub mineral_richness: u8,
    pub settlement_id: Option<SettlementId>,
}

#[derive(Resource, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldMap {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
    pub settlement_distances: BTreeMap<(SettlementId, SettlementId), u16>,
    pub settlement_positions: BTreeMap<SettlementId, (u32, u32)>,
}

impl WorldMap {
    pub fn get_tile(&self, x: u32, y: u32) -> Option<&Tile> {
        if x < self.width && y < self.height {
            Some(&self.tiles[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    pub fn get_tile_mut(&mut self, x: u32, y: u32) -> Option<&mut Tile> {
        if x < self.width && y < self.height {
            Some(&mut self.tiles[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    pub fn distance(&self, a: SettlementId, b: SettlementId) -> u16 {
        if a == b {
            return 0;
        }
        if let Some(&dist) = self.settlement_distances.get(&(a, b)) {
            dist
        } else if let (Some(&(ax, ay)), Some(&(bx, by))) = (
            self.settlement_positions.get(&a),
            self.settlement_positions.get(&b),
        ) {
            let dx = (ax as i64 - bx as i64).abs();
            let dy = (ay as i64 - by as i64).abs();
            (((dx * dx + dy * dy) as f64).sqrt().round() as u16).max(1)
        } else {
            10
        }
    }

    /// Procedurally generate a deterministic world map
    pub fn generate(
        width: u32,
        height: u32,
        settlement_count: usize,
        rng: &mut ChaCha8Rng,
    ) -> (Self, Vec<(SettlementId, u32, u32)>) {
        let total_tiles = (width * height) as usize;
        let mut tiles = Vec::with_capacity(total_tiles);

        // Simple coherent heightmap via deterministic octave superposition
        let freq_x = 0.05;
        let freq_y = 0.05;
        let offset_x: f64 = rng.gen_range(0.0..1000.0);
        let offset_y: f64 = rng.gen_range(0.0..1000.0);

        for y in 0..height {
            for x in 0..width {
                let nx = (x as f64 + offset_x) * freq_x;
                let ny = (y as f64 + offset_y) * freq_y;

                // Deterministic pseudo-noise
                let s1 = (nx.sin() * ny.cos() + 1.0) * 0.5;
                let s2 = ((nx * 2.0).cos() * (ny * 2.0).sin() + 1.0) * 0.25;
                let raw_val = (s1 + s2).clamp(0.0, 1.0);

                let elevation = (raw_val * 255.0) as u8;
                let moisture =
                    (((nx * 1.5).cos() + (ny * 1.5).sin() + 2.0) * 60.0).clamp(0.0, 255.0) as u8;

                let (biome, fertility, timber, mineral) = if elevation < 60 {
                    (Biome::Water, 0, 0, 0)
                } else if elevation > 200 {
                    (Biome::Mountains, 10, 20, 240)
                } else if elevation > 150 {
                    (Biome::Hills, 60, 80, 180)
                } else if moisture > 130 {
                    (Biome::Forest, 140, 220, 40)
                } else {
                    (Biome::Plains, 210, 70, 50)
                };

                tiles.push(Tile {
                    biome,
                    elevation,
                    moisture,
                    fertility,
                    timber_richness: timber,
                    mineral_richness: mineral,
                    settlement_id: None,
                });
            }
        }

        // Place settlements on prime habitable locations
        let mut settlement_positions = BTreeMap::new();
        let mut settlements = Vec::with_capacity(settlement_count);

        let mut candidate_positions = Vec::new();
        for y in (height / 8)..(height * 7 / 8) {
            for x in (width / 8)..(width * 7 / 8) {
                let idx = (y * width + x) as usize;
                if tiles[idx].biome != Biome::Water && tiles[idx].biome != Biome::Mountains {
                    candidate_positions.push((x, y, tiles[idx].fertility));
                }
            }
        }

        // Sort candidates deterministically by fertility descending, then x, y
        candidate_positions.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| a.1.cmp(&b.1))
        });

        let mut placed = 0;
        let min_dist_sq = ((width.min(height) / (settlement_count as u32).max(1)).max(4)).pow(2);

        for &(x, y, _) in &candidate_positions {
            if placed >= settlement_count {
                break;
            }
            let too_close = settlement_positions.values().any(|&(px, py)| {
                let dx = (px as i64 - x as i64).abs();
                let dy = (py as i64 - y as i64).abs();
                (dx * dx + dy * dy) < (min_dist_sq as i64)
            });

            if !too_close {
                let id = SettlementId(placed as u16);
                settlement_positions.insert(id, (x, y));
                let idx = (y * width + x) as usize;
                tiles[idx].settlement_id = Some(id);
                settlements.push((id, x, y));
                placed += 1;
            }
        }

        // Compute pairwise distances between all settlements
        let mut distances = BTreeMap::new();
        for i in 0..settlements.len() {
            for j in (i + 1)..settlements.len() {
                let (id_a, ax, ay) = settlements[i];
                let (id_b, bx, by) = settlements[j];
                let dx = (ax as i64 - bx as i64).abs();
                let dy = (ay as i64 - by as i64).abs();
                let dist = (((dx * dx + dy * dy) as f64).sqrt().round() as u16).max(1);
                distances.insert((id_a, id_b), dist);
                distances.insert((id_b, id_a), dist);
            }
        }

        (
            WorldMap {
                width,
                height,
                tiles,
                settlement_distances: distances,
                settlement_positions,
            },
            settlements,
        )
    }
}
