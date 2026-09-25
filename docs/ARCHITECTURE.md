# CIVITAS-1M Architecture Specification

## 1. System Decomposition & Boundaries

The architecture maintains a strict, unidirectional dependency hierarchy:

```
+-------------------------------------------------------------+
|                        civitas_app                          |
|  (Interactive Visualizer: egui/eframe, LOD Map, Inspectors) |
+------------------------------+------------------------------+
                               | Read-only Snapshots & Commands
                               v
+-------------------------------------------------------------+
|                        civitas_cli                          |
|   (Headless Driver: Run, Benchmark, Replay, Invariant Audit) |
+------------------------------+------------------------------+
                               | Direct Dependency
                               v
+-------------------------------------------------------------+
|                       civitas_core                          |
|  +-------------------------------------------------------+  |
|  | bevy_ecs (Headless ECS: SoA Component Tables)         |  |
|  +-------------------------------------------------------+  |
|  | Multirate Deterministic Scheduler                     |  |
|  +-------------------------------------------------------+  |
|  | Simulation Domains: World, Demographics, Economy,     |  |
|  |                     Households, Migration, Causality  |  |
|  +-------------------------------------------------------+  |
|  | Persistence & Replay: serde, bincode, CRC32 Checksums |  |
+-------------------------------------------------------------+
```

### Absolute Invariant: Decoupled Simulation
The authoritative simulation core (`civitas_core`) has **zero knowledge** of UI, rendering, or windowing libraries. It can be compiled, benchmarked, and executed headlessly across any target without graphical dependencies.

The visualizer (`civitas_app`) never mutates simulation state directly. It dispatches player commands (e.g. `Pause`, `Resume`, `SetSpeed`, `Step`, `SelectEntity`) through a bounded thread-safe command channel and receives immutable world snapshots or event rings.

---

## 2. Core ECS Primitive (`bevy_ecs`)

To satisfy the non-negotiable requirement of representing up to 1,000,000 persistent, addressable citizens without falling into the "Hand-Rolled ECS Trap" or object-per-agent heap fragmentation, `civitas_core` utilizes `bevy_ecs`:

- **Citizens as Pure Entity IDs**: A citizen is represented by a 64-bit `Entity` identifier (index + generation).
- **Structure-of-Arrays (SoA) Storage**: Citizen properties are partitioned into tightly packed, contiguous component tables:
  - `Demographics`: `(u32 age_ticks, u8 health, Gender gender, bool alive)`
  - `HouseholdMembership`: `(u32 household_id, HouseholdRole role)`
  - `SettlementResidence`: `(u16 settlement_id, u8 district_id)`
  - `OccupationProfile`: `(OccupationType occupation, u8 skill, u16 experience, f32 productivity)`
  - `PersonalFinances`: `(f64 savings, f32 last_wage, f32 expenses)`
  - `PhysicalNeeds`: `(u8 satiety, u8 shelter, u8 clothing, u8 comfort)`
  - `MobilityProfile`: `(MigrationStatus status, u16 origin, u16 destination, u16 ticks_remaining)`
  - `Kinship`: `(Option<u32> spouse, [Option<u32>; 2] parents, u16 offspring_count)`
  - `CausalAudit`: `(ReasonCode last_reason_code, u32 decision_tick, u32 context_payload)`

- **Systems Execution**: Logic is divided into deterministic systems scheduled via Bevy's `Schedule` and run criteria, or invoked deterministically by the multirate tick driver.

---

## 3. Determinism & Collection Rules

1. **Banned Collections**:
   - `std::collections::HashMap` and `std::collections::HashSet` are **strictly forbidden** in authoritative simulation state, because their default SipHash hasher introduces iteration non-determinism across platforms and runs.
   - Authoritative state must exclusively use `std::collections::BTreeMap`, `std::collections::BTreeSet`, or `indexmap::IndexMap` / `indexmap::IndexSet`.
2. **Deterministic Pseudo-Randomness**:
   - All randomness is driven by `rand_chacha::ChaCha8Rng`.
   - RNG streams are explicitly partitioned or derived: every simulation tick derives a sub-stream using `(master_seed, tick, domain_id)`, guaranteeing that parallel chunk evaluations never suffer from thread-scheduling race conditions.
3. **Floating Point Discipline**:
   - Calculations avoid non-deterministic platform-dependent math operations (e.g. `f32::sin` without software fallback or uncontrolled fast-math flags).
   - Numerical accumulators maintain order-deterministic reductions.

---

## 4. Concurrency & Parallelism Model

- Parallelism is utilized via `rayon` solely for **deterministic staged parallel passes**:
  - **Read/Evaluate Phase**: Citizens evaluate local choices (e.g. utility calculation for job open slots or migration push factors) concurrently into pre-allocated command buffers.
  - **Conflict-Resolution / Commit Phase**: Deterministic single-threaded or partitioned merge applies mutations in order of sorted Entity ID or deterministic priority queue.
- No shared mutable state or raw locks across threads during simulation ticks.

---

## 5. Event & Causality Architecture

- **Typed Event Ring**:
  High-level transitions emit structured events to a bounded event ring:
  ```rust
  pub struct SimEvent {
      pub tick: u64,
      pub event_type: SimEventType,
      pub entity: Option<u64>,
      pub settlement: Option<u16>,
      pub reason: DecisionTrace,
  }
  ```
- **The "Why Did This Happen?" Subsystem**:
  Every critical decision attaches a compact `DecisionTrace`. If Citizen #74321 migrates, inspection reveals:
  ```rust
  pub struct MigrationReason {
      pub wage_ratio: f32,       // e.g. 1.85 (destination paid 85% more)
      pub local_hunger_rate: f32,// local food starvation pressure
      pub housing_pressure: f32, // lack of housing in origin settlement
      pub transit_cost: f32,     // evaluated travel cost
  }
  ```
  Explanations are directly decompressed from numeric state parameters, completely eliminating hallucinated or disconnected descriptions.

---

## 6. Persistence & Replay Architecture

1. **Snapshot Serialization**:
   - Dense binary snapshots utilizing `bincode` and `serde`.
   - Header contains schema version, engine build version, master seed, tick, entity counts, and a CRC32/SHA-256 payload checksum.
2. **Deterministic Replay**:
   - Replay logs store initial seed, configuration, player command inputs with timestamped tick markers, and periodic authoritative state checksums.
   - Replay execution compares state checksums every $N$ ticks, detecting divergences immediately.
