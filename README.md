# CIVITAS-1M

**Frontier-Scale Rust Civilization Simulation Engine**

CIVITAS-1M is an interactive, native Rust simulation engine engineered to model emergent human settlements, macroeconomic price discovery, demographic lifecycles, and migration waves at a scale of up to **1,000,000 persistent individual citizens**.

---

## 1. Core Architectural Pillars

- **Strict Identity Persistence**: Every citizen is represented by a persistent, individual Entity in columnar Structure-of-Arrays (SoA) component tables via headless `bevy_ecs`. No anonymous aggregate counters or scalar shortcuts are used.
- **Multirate Deterministic Scheduler**: Daily fine physiological updates, weekly sliced economic market clearing, and monthly demographic/migration evaluations.
- **Strict Determinism**: Zero reliance on non-deterministic collections (standard `HashMap` and `HashSet` are strictly banned in authoritative state; `BTreeMap` and `indexmap` are used). Cryptographic-grade deterministic PRNG powered by `rand_chacha::ChaCha8Rng`.
- **Explainable Causality ("Why Did This Happen?")**: Structured `DecisionTrace` records capture exact mathematical push/pull factors, price disparities, and wage ratios behind citizen migrations, job changes, and mortality.
- **Decoupled Presentation**: Simulation core compiles as a headless library (`civitas_core`). The interactive viewer (`civitas_app`) is built with `egui` via `eframe` and only reads immutable state snapshots.
- **Checksummed Snapshot Persistence**: Native binary serialization via `bincode` with CRC32 integrity verification and bit-for-bit deterministic replay validation.

---

## 2. Workspace Organization

```
civitas/
├── Cargo.toml                  # Virtual workspace configuration & release profile
├── crates/
│   ├── civitas_core/           # Authoritative simulation core (bevy_ecs, rules, world, persistence)
│   ├── civitas_cli/            # Headless command-line runner, benchmarker, and validator
│   └── civitas_app/            # Decoupled interactive viewer (egui/eframe)
├── docs/
│   ├── PRODUCT_CONTRACT.md     # Non-negotiable scope, 1M agent contract, and acceptance criteria
│   ├── ARCHITECTURE.md         # Component design, ECS primitives, and concurrency
│   ├── SIMULATION_MODEL.md     # Multirate scheduling cadence and behavioral rules
│   ├── MEMORY_MODEL.md         # Per-citizen byte budget and RAM scaling projections
│   ├── DETERMINISM.md          # PRNG streams, collection sanitization, and state hashes
│   ├── TESTING.md              # Test matrix, invariants, and adversarial scenarios
│   ├── PERFORMANCE.md          # Benchmark methodology and telemetry reporting
│   ├── KNOWN_LIMITATIONS.md    # Bounded V1 scope boundaries
│   ├── LESSONS_LEARNED.md      # Engineering insights and reusable primitives
│   └── CAPABILITY_GAPS.md      # Future architectural primitives and tooling
└── CANDIDATE_REPORT.md         # Comprehensive verification evidence and benchmark results
```

---

## 3. Getting Started

### Prerequisites
- **Rust Toolchain**: 1.95+ (Tested on Rust 1.97.1)
- **Supported Platforms**: Windows, Linux, macOS (x86_64 and aarch64)

### Building the Project

```bash
# Build in debug mode
cargo build

# Build optimized release binaries
cargo build --release
```

---

## 4. Running the Simulator

### Interactive GUI Viewer (`civitas_app`)

Launch the visualizer to explore the procedural world map, inspect settlements, follow citizen lifecycles, and view real-time macroeconomic charts:

```bash
cargo run --release -p civitas_app
```

#### Viewer Features:
- **World Map**: Pan and zoom across procedural terrain, settlement nodes, travel networks, and population density indicators.
- **Settlement Inspector**: Demographics, housing capacity, warehouse inventories, market clearing prices, and wage levels.
- **Citizen Inspector**: Examine individual citizens, family lineage, physiological needs, wealth, occupation, and structured causal explanations ("Why did this citizen migrate?").
- **Household Inspector**: View pooled family savings, food reserves, and migration pressure.
- **Analytics & Trends**: Live plots of living population trends, commodity prices, and wage levels.
- **Event Log**: Real-time audit stream of births, deaths, migrations, and market shocks.

---

### Headless CLI (`civitas_cli`)

The simulator runs completely headlessly without graphical dependencies:

#### 1. Generate a Deterministic World
```bash
cargo run --release -p civitas_cli -- generate --seed 42 --settlements 8
```

#### 2. Run Headless Simulation
```bash
cargo run --release -p civitas_cli -- run --seed 42 --population 50000 --ticks 100 --save-out snapshot.civ
```

#### 3. Execute Deterministic Replay Check
```bash
cargo run --release -p civitas_cli -- replay --seed 42 --population 10000 --ticks 100
```

#### 4. Validate Save File Integrity
```bash
cargo run --release -p civitas_cli -- validate-save --file snapshot.civ
```

#### 5. Run Performance Benchmark
```bash
# Benchmark 10,000 citizens
cargo run --release -p civitas_cli -- benchmark --population 10000 --ticks 100

# Benchmark 100,000 citizens
cargo run --release -p civitas_cli -- benchmark --population 100000 --ticks 100

# Benchmark 1,000,000 persistent citizens
cargo run --release -p civitas_cli -- benchmark --population 1000000 --ticks 50
```

---

## 5. Running the Test Suite

```bash
# Run unit, integration, invariant, determinism, and persistence tests
cargo test --release --all-targets

# Run strict clippy linting
cargo clippy --all-targets -- -D warnings

# Verify formatting
cargo fmt --check
```
