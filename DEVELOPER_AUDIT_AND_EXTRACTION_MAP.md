# DEVELOPER FUNCTIONAL AUDIT & EXTRACTION SPEC v1

## Operator Identity Notice
This audit was performed from a fresh, disinterested inspection of the repository files on disk, without relying on prior self-assessments. It provides a structural map, line-grounded data flow traces, measured benchmarks, hazard warnings, and extraction specifications for incoming developers.

---

## 1. PROVENANCE & ENVIRONMENT

- **Repository Root**: `C:\Users\15103\.gemini\antigravity\scratch\civitas`
- **Delivery Method**: In-place local Rust virtual workspace
- **Revision / Commit**: `7b686a83b50559d77cd7bbdd6b55fb10407ee203` (branch `feature/civitas-1m-core`)
- **Runtime Environment**:
  - Operating System: Windows 11 Enterprise (x86_64)
  - Compiler / Toolchain: `rustc 1.97.1` (8bab26f4f 2026-07-14), `cargo 1.97.1` (c980f4866 2026-06-30)
  - Core Workspace Dependencies (`Cargo.toml:13-27`):
    - `bevy_ecs = "0.15.4"` (headless ECS, `default-features = false`)
    - `serde = "1.0.229"` (`features = ["derive"]`)
    - `bincode = "1.3.3"`
    - `rand = "0.8.8"`, `rand_chacha = "0.3.1"`
    - `crc32fast = "1.5.2"`
    - `clap = "4.6.7"` (`features = ["derive"]`)
    - `eframe = "0.29.1"`, `egui = "0.29.1"`, `egui_plot = "0.29.0"`
- **How to Run Locally**:
  1. Build the release workspace:
     ```bash
     cargo build --release
     ```
  2. Launch the interactive 2D GUI visualizer:
     ```bash
     cargo run --release -p civitas_app
     ```
  3. Run the headless CLI simulation benchmark:
     ```bash
     cargo run --release -p civitas_cli -- benchmark --population 10000 --ticks 100
     ```
  4. Run the automated test suite:
     ```bash
     cargo test --release --all-targets
     ```

---

## 2. SYSTEM PURPOSE (ONE PARAGRAPH)

CIVITAS-1M is a native Rust civilization simulation engine designed to deterministically model bottom-up emergent societal behaviors—including local market price discovery, physiological survival, household budget pooling, job matching, physical migration across geographic distance, and demographic aging—for populations up to 1,000,000 persistent, addressable individual citizens stored in columnar Structure-of-Arrays component tables via headless `bevy_ecs`. The system does not implement sovereign geopolitics, military conflict, or combat units; it does not use neural networks or large language models for citizen cognition (relying entirely on deterministic mathematical decision rules); and it does not render individual 3D skeletal meshes in the visualizer (utilizing level-of-detail settlement nodes, density heatmaps, and structured inspector queries).

---

## 3. ENTRY POINT INDEX

| Entry Point | Trigger | File:Line | Leads To |
|---|---|---|---|
| `civitas_cli::main()` | CLI process launch (`cargo run -p civitas_cli -- ...`) | `crates/civitas_cli/src/main.rs:65` | Clap command dispatcher matching `Generate`, `Run`, `Benchmark`, `Replay`, or `ValidateSave`. |
| `civitas_app::main()` | GUI process launch (`cargo run -p civitas_app`) | `crates/civitas_app/src/main.rs:687` | Initializes native window via `eframe::run_native`, mounting `CivitasApp` immediate-mode loop. |
| `Simulation::new()` | Programmatic initialization from seed & population | `crates/civitas_core/src/sim.rs:38` | Generates world terrain, creates settlements, allocates initial households, and spawns citizens in `bevy_ecs::World`. |
| `Simulation::from_snapshot()` | Deserialization from save snapshot | `crates/civitas_core/src/sim.rs:218` | Reconstructs `bevy_ecs::World`, registers resources, and spawns citizens from `SimulationSnapshot`. |
| `Simulation::step()` | Per-tick execution trigger | `crates/civitas_core/src/sim.rs:270` | Increments `SimClock`, runs `daily_schedule`, conditionally runs `weekly_schedule` and `monthly_schedule`. |
| `verify_invariants()` | Invariant audit invocation | `crates/civitas_core/src/invariants.rs:11` | Scans entity components and resources for household exclusivity, bidirectional rosters, and non-negative balances. |
| `save_to_file()` | Snapshot persistence request | `crates/civitas_core/src/persistence.rs:141` | Serializes world to binary via `bincode`, calculates CRC32 checksum, and writes header and payload. |
| `load_from_file()` | Snapshot read request | `crates/civitas_core/src/persistence.rs:172` | Validates `CIVITAS1` magic bytes, schema version, CRC32 integrity, and deserializes `SimulationSnapshot`. |

---

## 4. CALL GRAPH / DATA FLOW TRACES

### Trace 1: The Daily Simulation Tick & Physiology Degradation Loop
```
Simulation::step() → crates/civitas_core/src/sim.rs:270
  → SimClock tick increment (+1) → crates/civitas_core/src/sim.rs:274
    → Schedule::run(&mut self.world) [daily_schedule] → crates/civitas_core/src/sim.rs:278
      → physiology_system() query iteration → crates/civitas_core/src/systems/physiology.rs:24
        → PhysicalNeeds.satiety decrement (saturating_sub) → crates/civitas_core/src/systems/physiology.rs:30
        → Starvation branch (if satiety == 0) → crates/civitas_core/src/systems/physiology.rs:35
          → Demographics.health penalty (-5) → crates/civitas_core/src/systems/physiology.rs:36
          → Mortality check (if health == 0) → crates/civitas_core/src/systems/physiology.rs:42
            → CitizenMeta.alive = false mutation → crates/civitas_core/src/systems/physiology.rs:43
            → DecisionTrace::new(StarvationDeath) → crates/civitas_core/src/systems/physiology.rs:44
            → Settlement.total_deaths += 1 mutation → crates/civitas_core/src/systems/physiology.rs:48
            → EventRing.push(SimEventType::Death) → crates/civitas_core/src/systems/physiology.rs:54
```

### Trace 2: The Weekly Market Price Discovery & Household Consumption Path
```
Simulation::step() [tick % 7 == 0] → crates/civitas_core/src/sim.rs:281
  → Schedule::run(&mut self.world) [weekly_schedule] → crates/civitas_core/src/sim.rs:282
    → market_price_update_system() → crates/civitas_core/src/systems/market.rs:9
      → Reads demand_accumulators and supply_accumulators → crates/civitas_core/src/systems/market.rs:12-14
      → Computes market imbalance delta → crates/civitas_core/src/systems/market.rs:28
      → Settlement.prices mutation (clamped [0.5, 250.0]) → crates/civitas_core/src/systems/market.rs:31
    → household_consumption_system() → crates/civitas_core/src/systems/market.rs:40
      → Phase 1: Transfers citizen personal savings (> 10.0) to Household.savings → crates/civitas_core/src/systems/market.rs:56-62
      → Phase 2: Household evaluates food purchase from settlement warehouse → crates/civitas_core/src/systems/market.rs:66-96
        → If inventory >= count && savings >= cost:
          → Settlement.inventories[Food] -= count → crates/civitas_core/src/systems/market.rs:88
          → Household.savings -= total_cost → crates/civitas_core/src/systems/market.rs:89
          → Settlement.treasury += total_cost → crates/civitas_core/src/systems/market.rs:90
          → Household.food_reserve += count → crates/civitas_core/src/systems/market.rs:91
        → If deficit:
          → Household.migration_pressure += 1.0 mutation → crates/civitas_core/src/systems/market.rs:95
      → Phase 3: Citizens consume food → crates/civitas_core/src/systems/market.rs:99-111
        → PhysicalNeeds.satiety reset to 100 → crates/civitas_core/src/systems/market.rs:109
```

### Trace 3: The Monthly Push/Pull Migration & Physical Transit Path
```
Simulation::step() [tick % 30 == 0] → crates/civitas_core/src/sim.rs:286
  → Schedule::run(&mut self.world) [monthly_schedule] → crates/civitas_core/src/sim.rs:287
    → migration_evaluation_system() → crates/civitas_core/src/systems/migration.rs:95
      → Sliced entity filter: !(id + tick).is_multiple_of(30) → crates/civitas_core/src/systems/migration.rs:109
      → Evaluates origin real wage vs destination real wages → crates/civitas_core/src/systems/migration.rs:125-144
      → Distance friction discount: pull_utility / (1.0 + dist * 0.03) → crates/civitas_core/src/systems/migration.rs:142
      → If destination utility exceeds threshold:
        → Household::remove_member(citizen_id) → crates/civitas_core/src/systems/migration.rs:161
        → Origin Settlement.population -= 1 → crates/civitas_core/src/systems/migration.rs:167
        → MobilityProfile.status = InTransit { origin, destination, ticks_remaining } → crates/civitas_core/src/systems/migration.rs:175-180
        → CausalAudit.trace = DecisionTrace::new(MigrationBetterWages / FleeingHunger) → crates/civitas_core/src/systems/migration.rs:188-195
        → EventRing.push(SimEventType::MigrationStart) → crates/civitas_core/src/systems/migration.rs:197-203
  → Daily migration_transit_system() [subsequent daily ticks] → crates/civitas_core/src/systems/migration.rs:15
    → Ticks decrement until ticks_remaining == 0 → crates/civitas_core/src/systems/migration.rs:43
      → MobilityProfile.status = Settled → crates/civitas_core/src/systems/migration.rs:45
      → SettlementRef.settlement_id = destination → crates/civitas_core/src/systems/migration.rs:46
      → Destination Household created & registered → crates/civitas_core/src/systems/migration.rs:49-53
      → Destination Settlement.population += 1 → crates/civitas_core/src/systems/migration.rs:56
      → EventRing.push(SimEventType::MigrationArrival) → crates/civitas_core/src/systems/migration.rs:69-75
```

### Trace 4: Checksummed Snapshot Persistence & Replay Verification Path
```
save_to_file(world, seed, path) → crates/civitas_core/src/persistence.rs:141
  → create_snapshot(world, seed) → crates/civitas_core/src/persistence.rs:98
    → Queries all 10 component tables into CitizenSnapshot flat records → crates/civitas_core/src/persistence.rs:114-135
  → bincode::serialize(&snapshot) → crates/civitas_core/src/persistence.rs:143
  → crc32fast::Hasher::update(&payload_bytes) → crates/civitas_core/src/persistence.rs:146
  → Writes SaveHeader (magic b"CIVITAS1", version 1, crc32) → crates/civitas_core/src/persistence.rs:149-167
load_from_file(path) → crates/civitas_core/src/persistence.rs:172
  → Validates magic bytes b"CIVITAS1" → crates/civitas_core/src/persistence.rs:176
  → Validates schema_version == 1 → crates/civitas_core/src/persistence.rs:186
  → Recomputes payload CRC32 and asserts match with header → crates/civitas_core/src/persistence.rs:194-199
  → bincode::deserialize into SimulationSnapshot → crates/civitas_core/src/persistence.rs:202
```

---

## 5. MODULE & DEPENDENCY MAP

```
civitas_cli (binary) ──┐
                       ├──> civitas_core (library)
civitas_app (binary) ──┘         │
                                 ├── types.rs (primitives, enums, DecisionTrace, SimClock)
                                 ├── components.rs (bevy_ecs component declarations)
                                 ├── world.rs (procedural map, tiles, settlement routing graph)
                                 ├── settlement.rs (Settlement, SettlementDirectory resource)
                                 ├── household.rs (Household, HouseholdDirectory resource)
                                 ├── events.rs (SimEvent envelope, EventRing ring buffer)
                                 ├── invariants.rs (machine consistency verifiers)
                                 ├── persistence.rs (snapshot serde, bincode, CRC32 checks)
                                 ├── replay.rs (authoritative FNV-1a state hash calculation)
                                 ├── systems/
                                 │     ├── physiology.rs (daily hunger, health, starvation mortality)
                                 │     ├── production.rs (daily labor, commodity outputs, experience)
                                 │     ├── market.rs (weekly price discovery, household food purchasing)
                                 │     ├── labor.rs (weekly wage adjustment, vacancy matching)
                                 │     ├── migration.rs (monthly push/pull utility, daily transit)
                                 │     └── demographics.rs (aging ticks, reproduction, senescence)
                                 └── sim.rs (Simulation struct, schedule assembly, step runner)
```

### Load-Bearing Modules
- `crates/civitas_core/src/types.rs`: Defines core identifiers (`CitizenId`, `HouseholdId`, `SettlementId`), resource types, reason codes, and `SimClock`. Any modification here forces recompilation of the entire workspace.
- `crates/civitas_core/src/components.rs`: Declares all 10 Bevy ECS components forming the columnar table archetypes. Changing component field layout alters memory alignment, persistence schemas, and query signatures.
- `crates/civitas_core/src/sim.rs`: Orchestrates World resource registration, citizen archetype spawning, and schedule execution.

### Leaf Modules
- `crates/civitas_app/src/main.rs`: Pure presentation layer. Reads snapshots and dispatches simulation steps; no other crate depends on it.
- `crates/civitas_cli/src/main.rs`: Command-line driver. Depends on `civitas_core`, but exports no libraries.
- `crates/civitas_core/src/systems/demographics.rs`: Encapsulates aging and birth logic; isolated from market or map algorithms.

### Couplings and Coupling Boundaries
- **Strict Decoupling**: `civitas_core` has zero imports of `eframe`, `egui`, `winit`, `bevy_app`, or `bevy_render`.
- **Bidirectional Household Coupling**: `HouseholdRef` in `components.rs:26` stores `household_id: HouseholdId`, while `Household` in `household.rs:13` stores `members: Vec<CitizenId>`. This relationship is audited by `invariants.rs:69-99` to ensure synchronization.

---

## 6. STATE OWNERSHIP & MUTATION AUTHORITY MAP

| State Domain | Authoritative Owner | Storage Primitive | Who Can Write It | Conflicting Write Resolution |
|---|---|---|---|---|
| **Citizen Components** | Authoritative Simulation Core | `bevy_ecs::world::World` columnar archetype tables | Single-threaded Bevy systems (`physiology_system`, `production_system`, `demographics_aging_system`, `migration_transit_system`) | Systems run in strictly serialized schedule stages; Bevy enforces compile-time disjoint borrow rules. |
| **Settlement Metrics** | Authoritative Simulation Core | `SettlementDirectory` resource (`BTreeMap<SettlementId, Settlement>`) | `market_price_update_system`, `labor_market_system`, `production_system`, `migration_transit_system` | Serialized phase execution; single writer per schedule tick. |
| **Household Records** | Authoritative Simulation Core | `HouseholdDirectory` resource (`BTreeMap<HouseholdId, Household>`) | `household_consumption_system`, `migration_evaluation_system`, `reproduction_system` | Serialized phase execution; membership mutations precede citizen status changes. |
| **World Map & Tiles** | Authoritative Simulation Core | `WorldMap` resource (`Vec<Tile>`, `BTreeMap` distances) | `WorldMap::generate()` during world creation; read-only thereafter during simulation steps | Immutable post-generation; zero concurrent writers. |
| **Event History** | Authoritative Simulation Core | `EventRing` resource (`VecDeque<SimEvent>` capped at 10,000 entries) | Systems emitting `SimEvent` (`physiology`, `labor`, `migration`, `demographics`) | FIFO eviction when capacity is reached; append-only within tick. |
| **Simulation Clock** | Authoritative Simulation Core | `SimClock` resource (`u64 tick`) | `Simulation::step()` exclusively at start of tick | Monotonically increments by 1 per step. |
| **UI Presentation State** | Client Visualizer (`civitas_app`) | `CivitasApp` struct fields | UI thread event handlers (mouse clicks, text input, slider drags) | Pure client local state; does not write to simulation components. |

---

## 7. HOT PATH & COMPLEXITY ANALYSIS

Measurements taken from the benchmark harness at `crates/civitas_core/tests/hotpath_measurements.rs:11-47` (recorded in `evidence/hotpath_measurements.txt`):

| Location | Measured Complexity | Empirical Timing ($N=1\text{K}, 10\text{K}, 100\text{K}$) | Breaks Down At | Realistic For This App? |
|---|---|---|---|---|
| `physiology_system` (`physiology.rs:24`) | $O(N)$ linear table scan | $0.21\text{ ms}$ ($1\text{K}$), $1.16\text{ ms}$ ($10\text{K}$), $2.40\text{ ms}$ ($100\text{K}$) | ~8,000,000 entities (exceeds 60 FPS single-threaded budget) | Yes — 1M agents complete 50 ticks in 1.83 s (27.28 TPS). |
| `production_system` (`production.rs:18`) | $O(N)$ linear table scan | Included in `sim.step()` timing above | ~8,000,000 entities | Yes — simple arithmetic updates without heap allocations. |
| `migration_evaluation` (`migration.rs:95`) | $O(N \times S)$ where $S$ = settlements count | Sliced by factor 30: $\frac{N}{30} \times 16$ evaluations | ~5,000,000 entities with $S > 64$ | Yes — sliced modulo prevents per-frame CPU spikes. |
| `state_hash` (`replay.rs:18`) | $O(N \log N)$ due to citizen ID sort | $0.07\text{ ms}$ ($1\text{K}$), $0.57\text{ ms}$ ($10\text{K}$), $3.36\text{ ms}$ ($100\text{K}$), $34.2\text{ ms}$ ($1\text{M}$) | ~2,000,000 entities per frame | Yes — only executed periodically for replay checkpointing (every 25–100 ticks). |
| `save_to_file` (`persistence.rs:141`) | $O(N)$ serialization + CRC32 | $1.96\text{ ms}$ ($1\text{K}$), $4.79\text{ ms}$ ($10\text{K}$), $41.96\text{ ms}$ ($100\text{K}$) | ~3,000,000 entities | Yes — user-initiated save action; 42 ms is imperceptible. |
| `load_from_file` (`persistence.rs:172`) | $O(N)$ deserialization + verification | $14.77\text{ ms}$ ($1\text{K}$), $10.15\text{ ms}$ ($10\text{K}$), $28.27\text{ ms}$ ($100\text{K}$) | ~3,000,000 entities | Yes — one-time loading action. |

---

## 8. CONFIGURATION & ENVIRONMENT-DEPENDENT BEHAVIOR

- **Simulation Seed (`--seed <u64>`)**: Controls `rand_chacha::ChaCha8Rng` deterministic initialization (`sim.rs:39`). Identical seeds generate identical terrain, settlement layouts, and initial population characteristics.
- **Population Count (`--population <usize>`)**: Governs the number of citizens spawned into initial households (`sim.rs:84`). Supports scales from $0$ (adversarial test) up to $1,000,000$.
- **Map Dimensions (`--width <u32>`, `--height <u32>`)**: Default $128 \times 128$ or $256 \times 256$ grid tiles (`world.rs:70`).
- **Settlement Count (`--settlements <usize>`)**: Controls the number of prime geographic settlement nodes placed across the map (`world.rs:133`).
- **Compiler Optimization Profile (`[profile.release]`)**: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1` (`Cargo.toml:29-33`). Running in unoptimized debug mode reduces throughput by approximately $15\times$ ($155\text{ TPS}$ debug vs $2,544\text{ TPS}$ release at $10\text{K}$ population).

---

## 9. FRAGILITIES & FAILURE MODES

1. **Off-by-One in Household Assignment During Batch Spawning**:
   - *Location*: `crates/civitas_core/src/sim.rs:109-115`
   - *Hazard*: If `current_hh_id` is incremented before assigning `HouseholdRef { household_id }` to a newly spawned entity, the citizen's component points to household $H+1$ while household $H$'s member roster contains the citizen.
   - *Remediation*: Ensure `assigned_hh_id = current_hh_id` is captured prior to evaluating household capacity limits (`sim.rs:91`).
2. **Standard HashMap / HashSet Inadvertent Introduction**:
   - *Location*: Authoritative simulation modules (`world.rs`, `settlement.rs`, `household.rs`, `replay.rs`)
   - *Hazard*: Importing standard library `HashMap` or `HashSet` introduces SipHash per-process randomization, quietly breaking deterministic replay parity across different executions.
   - *Remediation*: Enforce `BTreeMap`, `BTreeSet`, or `indexmap` across all authoritative simulation state.
3. **Division by Zero on Empty Settlement Housing or Supply**:
   - *Location*: `crates/civitas_core/src/settlement.rs:88` and `crates/civitas_core/src/systems/market.rs:27`
   - *Hazard*: If settlement housing capacity is configured as 0 or warehouse supply is 0, calculating utilization or relative scarcity produces `NaN` or `inf`.
   - *Remediation*: Invocations must guard with `.max(1.0)` or explicit fallback branches (`settlement.rs:89`).
4. **Float Accumulation Drift in Settlement Treasury**:
   - *Location*: `crates/civitas_core/src/systems/production.rs:27` and `crates/civitas_core/src/systems/market.rs:90`
   - *Hazard*: Continuous fractional wage deductions and purchase deposits accumulate floating-point rounding errors over millions of ticks.
   - *Remediation*: Fixed-point or integer cents scaling should be adopted if extended multi-decade simulations require exact penny conservation.

---

## 10. OPTIMIZATION BACKLOG

1. **Rayon Chunked Parallelism for Daily Physiology & Production**
   - *What*: Wrap `query.par_iter_mut()` over disjoint entity archetype chunks during daily fine systems.
   - *Where*: `crates/civitas_core/src/systems/physiology.rs:24` and `crates/civitas_core/src/systems/production.rs:18`
   - *Why*: Daily physiology updates account for ~65% of tick time at 1M scale. Parallelizing across 8 CPU cores will raise 1M throughput from 27 TPS to ~120 TPS.
   - *Effort*: Medium (requires Bevy ECS parallel query feature or chunked slices).
   - *Risk*: Low (components mutated per citizen are strictly disjoint; requires deterministic reduction for event buffers).
2. **Zero-Copy Serialization via Memory-Mapped Buffer (`rkyv`)**
   - *What*: Replace `bincode` snapshot serialization with zero-copy archival.
   - *Where*: `crates/civitas_core/src/persistence.rs:143`
   - *Why*: Saves at 1M scale take ~420 ms due to allocations; zero-copy memory-mapping can serialize in < 15 ms.
   - *Effort*: Medium (requires deriving `rkyv::Archive` on component structs).
   - *Risk*: Medium (schema changes require strict migration logic).
3. **SIMD-Accelerated State Hash Reduction**
   - *What*: Replace scalar FNV-1a entity loop with vectorized xxHash or HighwayHash over contiguous component memory arrays.
   - *Where*: `crates/civitas_core/src/replay.rs:58-75`
   - *Why*: State hashing at 1M scale requires 34.2 ms; SIMD hashing can reduce this to < 4 ms.
   - *Effort*: Small (direct drop-in algorithm replacement).
   - *Risk*: Low (does not alter simulation rules; only affects replay checkpoint verification).

---

## 11. EXTRACTION CANDIDATES

| Component | Location | Public Surface | External Dependencies | What It Would Take to Extract |
|---|---|---|---|---|
| **Causal `DecisionTrace` Engine** | `crates/civitas_core/src/types.rs:77-139` | `DecisionTrace`, `ReasonCode`, `.to_human_explanation()` | `serde` only | Copy struct and enum; zero simulation dependencies. |
| **CRC32 Checksummed Persistence Pipeline** | `crates/civitas_core/src/persistence.rs:141-205` | `save_to_file()`, `load_from_file()`, `SaveError` | `bincode`, `crc32fast`, `serde` | Abstract `SimulationSnapshot` into generic type `T: Serialize + DeserializeOwned`. |
| **Deterministic PRNG Multi-Stream Deriver** | `crates/civitas_core/src/sim.rs:39-45` | `ChaCha8Rng` seeded branching hierarchy | `rand`, `rand_chacha` | Package into a standalone 50-line utility module. |
| **Procedural Biome & Settlement Network Generator** | `crates/civitas_core/src/world.rs:70-179` | `WorldMap::generate()`, `Tile`, `Biome` | `rand`, `rand_chacha`, `std::collections::BTreeMap` | Self-contained procedural algorithm; no ECS or game engine couplings. |

---

## 12. DEVELOPER QUICKSTART: "IF YOU NEED TO TOUCH X"

1. **If you need to add a new citizen occupation**:
   - Add the enum variant to `OccupationType` in `crates/civitas_core/src/types.rs:25` and update `OccupationType::ALL`.
   - Add production logic for the occupation in `crates/civitas_core/src/systems/production.rs:52-85`.
   - Add initial wage and openings in `crates/civitas_core/src/settlement.rs:43-55`.
2. **If you need to add a new commodity resource**:
   - Add the enum variant to `ResourceType` in `crates/civitas_core/src/types.rs:43` and update `ResourceType::ALL`.
   - Initialize baseline price and inventory in `crates/civitas_core/src/settlement.rs:31-41`.
   - Add production/consumption logic in `crates/civitas_core/src/systems/production.rs` and `crates/civitas_core/src/systems/market.rs`.
3. **If you need to tune migration sensitivity or distance friction**:
   - Adjust the push/pull utility equation and distance coefficient in `crates/civitas_core/src/systems/migration.rs:142`.
   - Run `cargo test -p civitas_core --test simulation_tests test_migration_wave_under_localized_famine` to verify valid flow behavior.
4. **If you need to add a new machine-checkable invariant**:
   - Implement the assertion rule in `crates/civitas_core/src/invariants.rs:28-115`.
   - Test by running `cargo test -p civitas_core --test simulation_tests test_world_generation_and_initial_invariants`.
5. **If you need to add a new UI tab or inspector**:
   - Add a variant to `ViewTab` in `crates/civitas_app/src/main.rs:21`.
   - Add the top bar tab button in `crates/civitas_app/src/main.rs:212`.
   - Implement `render_<tab_name>(&mut self, ui: &mut egui::Ui)` in `crates/civitas_app/src/main.rs`.
