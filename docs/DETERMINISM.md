# CIVITAS-1M Determinism & Verification Specification

## 1. Principles of Deterministic Simulation
CIVITAS-1M must guarantee that for any given triplet $(\text{Seed}, \text{Config}, \text{TickCount})$, execution on any system running the same build produces bit-for-bit identical authoritative simulation state.

## 2. Determinism Guarantees

### 2.1 Pseudo-Random Number Generation
- **Generator**: `rand_chacha::ChaCha8Rng` (cryptographically strong, completely platform-independent, zero dependence on OS entropy during simulation steps).
- **RNG Derivation Hierarchy**:
  ```
  Master Seed (u64)
    ├── World Gen Stream (ChaCha8Rng::seed_from_u64(seed ^ 0x01))
    ├── Per-Tick Master Stream (derived from tick index + seed)
    │     ├── Daily Survival Stream (tick, 0x10)
    │     ├── Economic Market Stream (tick, 0x20)
    │     ├── Migration Decisions Stream (tick, 0x30)
    │     └── Demographics Stream (tick, 0x40)
  ```
- No system time, thread IDs, or unseeded hardware randomness may be accessed within simulation systems.

### 2.2 Strict Prohibition of Non-Deterministic Collections
- `std::collections::HashMap` and `HashSet` are banned.
- All associative maps and sets in authoritative state must be `std::collections::BTreeMap`, `std::collections::BTreeSet`, or `indexmap::IndexMap` / `indexmap::IndexSet`.
- Iteration orders across collections are guaranteed to be identical across runs and architectures.

### 2.3 Floating-Point Reproducibility
- Floating-point arithmetic strictly uses standard IEEE 754 operations without non-associative reordering flags (`-C target-cpu=native` with fast-math is forbidden).
- Reductions (e.g. summing total settlement wealth or market demand) must either use sorted Entity order or deterministic tree reductions.
- Where fixed-point is viable (currency/wealth, price indices, inventory units), integer/fixed-point scaling is preferred.

### 2.4 Parallelism & Order Independence
- Rayon work chunks process disjoint subsets.
- When generating simulation events or applying state changes, mutations are accumulated into thread-local buffers and merged into the authoritative ECS in strictly sorted Entity ID order.

---

## 3. Checksum & Divergence Detection
Every $N$ ticks (default $N = 100$), the simulation computes an authoritative state hash:
- Hashes sorted Entity IDs and their component states.
- Hashes settlement inventories, treasury, and demographic tallies.
- Hashes household rosters and balances.
- Hashes current tick and RNG state counter.
- Replay validation matches this running hash against recorded baseline runs. Any mismatch immediately flags the exact tick of divergence.
