# CIVITAS-1M Memory Model & Hardware Budget

## 1. Design Constraints & Hardware Baseline
- **Baseline Target Hardware**: AMD Ryzen 9, 32 GB System RAM, RTX 3060.
- **Budgetary Ceilings**:
  - Maximum Authoritative State RAM (1,000,000 citizens): **< 2.5 GB** (Less than 8% of 32 GB system memory).
  - Component Data Locality: Zero heap pointers inside per-citizen component structs.
  - Storage Paradigm: `bevy_ecs` columnar archetype/table storage (Structure-of-Arrays).

---

## 2. Per-Citizen Component Memory Budget

Each citizen entity is composed of compact, flat ECS components stored contiguously in columnar memory arrays:

| Component Struct | Field Breakdown | Byte Size |
| :--- | :--- | :--- |
| **`CitizenMeta`** | `stable_id: u64`, `gender: u8`, `alive: bool`, `_pad: [u8; 6]` | 16 bytes |
| **`Demographics`** | `age_years: u16`, `age_ticks: u16`, `health: u8`, `fertility_timer: u8`, `_pad: [u8; 2]` | 8 bytes |
| **`HouseholdRef`** | `household_id: u32`, `role: u8`, `_pad: [u8; 3]` | 8 bytes |
| **`SettlementRef`** | `settlement_id: u16`, `district_id: u8`, `in_transit: bool` | 4 bytes |
| **`OccupationProfile`**| `occupation: u8`, `skill_level: u8`, `experience: u16`, `productivity: f32` | 8 bytes |
| **`PersonalFinances`**| `savings: f64`, `last_income: f32`, `last_expense: f32` | 16 bytes |
| **`PhysicalNeeds`** | `satiety: u8`, `shelter: u8`, `clothing: u8`, `morale: u8` | 4 bytes |
| **`MobilityProfile`** | `status: u8`, `origin: u16`, `destination: u16`, `ticks_remaining: u16`, `_pad: u8` | 8 bytes |
| **`Kinship`** | `spouse: Option<u32>`, `parent_a: Option<u32>`, `parent_b: Option<u32>`, `children: u16`, `_pad: u16` | 16 bytes |
| **`CausalAudit`** | `last_reason: u16`, `decision_tick: u32`, `param_a: u16`, `param_b: u32` | 12 bytes |

### Total Raw Component Footprint:
$$\text{Raw Bytes per Citizen} = 16 + 8 + 8 + 4 + 8 + 16 + 4 + 8 + 16 + 12 = 100\text{ bytes}$$

With `bevy_ecs` table padding and generational entity index records:
$$\text{Effective Footprint per Citizen} \approx 128\text{ bytes}$$

---

## 3. Global Structural Memory Budgets

### 3.1 Households
- Assumed household size: 3 to 4 citizens per household $\implies$ 250,000 households at 1M population.
- Per-Household Struct: `id: u32`, `settlement_id: u16`, `savings: f64`, `members: SmallVec<[u32; 4]>` $\approx 48$ bytes.
- Total Household Memory (1M pop): $250,000 \times 48\text{ B} \approx \mathbf{12.0\text{ MB}}$.

### 3.2 Settlements
- World size: 64 to 256 settlement nodes.
- Per-Settlement Struct: Market price indices, commodity inventories, housing capacity, labor statistics, historical rings $\approx 8\text{ KB}$.
- Total Settlement Memory: $256 \times 8\text{ KB} \approx \mathbf{2.0\text{ MB}}$.

### 3.3 Procedural World Grid
- Map dimensions: $128 \times 128 = 16,384$ tiles or $256 \times 256 = 65,536$ tiles.
- Per-Tile Struct: Elevation (`u8`), Moisture (`u8`), Biome (`u8`), ResourceType (`u8`), TravelCost (`u8`), SettlementRef (`Option<u16>`) $\approx 8$ bytes.
- Total World Grid Memory ($256 \times 256$): $65,536 \times 8\text{ B} \approx \mathbf{0.5\text{ MB}}$.

### 3.4 Bounded Event Ring & Causal Logs
- Ring Buffer: 100,000 recent events $\times 32$ bytes $\approx \mathbf{3.2\text{ MB}}$.

---

## 4. Projected Total RAM Footprint by Population Scale

| Population Scale | Active Citizens RAM | Households RAM | World & Settlements | Event Ring & Caches | Total Estimated RAM |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **10,000 (10K)** | ~1.3 MB | ~0.12 MB | ~3.0 MB | ~4.0 MB | **~8.5 MB** |
| **100,000 (100K)** | ~12.8 MB | ~1.2 MB | ~3.0 MB | ~10.0 MB | **~27.0 MB** |
| **1,000,000 (1M)**| ~128.0 MB | ~12.0 MB | ~4.0 MB | ~32.0 MB | **~176.0 MB** |

Even accounting for peak allocation headroom, deserialization buffers, and Rayon parallel thread scratchpads:
$$\text{Peak Heap Allocation at 1M Citizens} \le \mathbf{450\text{ MB}}$$
This fits with over a $70\times$ safety margin on the baseline 32 GB configuration, leaving ample room for OS, browser, and decoupled visualization.
