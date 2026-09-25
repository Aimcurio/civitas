# CIVITAS-1M Product Contract

## 1. Document Control & Status
- **Current State**: `CONTRACT_FROZEN`
- **Authorized Target State**: `REQUESTER_REVIEW_REQUIRED`
- **Engine Name**: CIVITAS-1M
- **Implementation Language**: Rust 1.97+

## 2. Mission Statement
CIVITAS-1M is an interactive, native Rust civilization simulator engineered to deterministically evolve populations up to **1,000,000 persistent individual citizens**. It models emergent macro-dynamics (settlement formation, economic specialization, price discovery, demographic transitions, and migration waves) strictly through bottom-up individual and household behavioral rules.

## 3. The 1M Agent Contract (Non-Negotiable)
1. **Persistent Identity**: Every represented citizen is a distinct, addressable entity with a stable ID.
2. **Individual State Retention**:
   - Stable Entity ID (`u64` / generational `Entity`)
   - Age (in discrete simulation ticks/calendar years)
   - Sex / Reproductive profile
   - Alive / Dead status
   - Household membership ID
   - Settlement / Location reference (or explicit in-transit state)
   - Occupation and skill profile
   - Health and physical satisfaction
   - Wealth / savings and income records
   - Current behavioral activity
   - Mobility and migration status
   - Causal provenance / Decision trace
   - Social ties (parents, spouse, offspring count)
3. **No Aggregate Substitution**: Aggregates (e.g., settlement demographic counts, price indexes) are strictly *derived acceleration structures*. Under no circumstances may active citizens be collapsed into scalar counters to simulate scale.
4. **Multirate Scheduling vs Identity**: Fidelity tiers and multirate updates govern execution frequency, never identity persistence.

## 4. In-Scope Feature Boundaries (V1)
- **Deterministic Procedural World**: Hex/Grid regional geography, terrain altitude, biomes, localized natural resources (fertile land, timber, stone, ore), travel costs, and environmental carrying capacity.
- **Individual Citizens & Households**:
  - Physiological survival (food, shelter, basic health).
  - Household economy: pooled resources, collective consumption, housing pressure, formation (marriage/leaving home), and dissolution.
  - Labor & Occupations: farming, foraging, extraction, crafting, services, trading.
  - Demographics: deterministic aging, marriage, conception, birth, natural & starvation mortality.
- **Economy**:
  - Labor market: wages driven by local settlement labor supply and production demand.
  - Commodity exchange: local market centers with price discovery driven by inventory, local demand, and production capacity.
  - Bounded currency/credit: currency circulation between households, production sites, and market clearinghouses.
- **Migration & Settlement Dynamics**:
  - Settlement emergence, growth, stagnation, and abandonment.
  - Push/pull migration evaluated via wage differentials, housing availability, starvation pressure, and physical distance travel costs.
  - Physical transit: citizens visibly travel between settlements across geographic routes.
- **Causality & "Why Did This Happen?" System**:
  - Compact `DecisionTrace` attached to significant agent transitions (migration, marriage, job changes, starvation).
  - Structured queries for citizen provenance and historical settlement events.
- **Save/Load & Deterministic Replay**:
  - Complete authoritative binary snapshot (`bincode` + CRC32 verification).
  - Event log and deterministic re-simulation verification.
- **Dual Presentation**:
  - Headless CLI: authoritative for testing, invariant auditing, determinism checks, and high-performance benchmarking.
  - Standalone GUI (`civitas_app` using `egui`): decoupled inspector, camera pan/zoom, demographic and economic analytics, LOD flow visualization.

## 5. Explicitly Out of Scope (V1)
- Geopolitical diplomacy, treaties, warfare, military units, and combat.
- LLM or neural network citizen cognition (simulation must rely entirely on deterministic behavioral rules).
- Complex religion, culture memes, or genetic inheritance simulations.
- Photorealistic 3D rendering or individual humanoid mesh animation.
- Multiplayer, client-server networking, or cloud infrastructure.
- Arbitrary scripting or modding runtime environments.

## 6. Machine-Checkable Invariants
1. **Household Exclusivity**: A living citizen belongs to at most one household at any point in time.
2. **Bidirectional Household Consistency**: If Citizen $C$ has household $H$, household $H$'s member roster must contain $C$.
3. **Settlement / Transit Validity**: A citizen must reside in a valid settlement or possess an explicit, tick-bounded `InTransit` state with valid origin, destination, and path progress.
4. **Conservation of Population**: Active living citizen count + cumulative dead citizens = total historical citizens spawned.
5. **Non-Negative Accounting**: Wealth, resource inventories, and prices must never be negative, NaN, or infinite.
6. **Dead Agent Quiescence**: Dead citizens are marked quiescent; they never consume resources, earn wages, reproduce, or execute behavior systems.
7. **Authoritative Determinism**: Identical Seed + Identical Configuration + Identical Tick Count $\implies$ Bit-identical Authoritative State Hash.
8. **Save/Load Roundtrip Fidelity**: Save $\implies$ Load $\implies$ Identical Hash and valid continuation.

## 7. Acceptance Gate
To transition from `IMPLEMENTATION_ACTIVE` to `CANDIDATE_VERIFIED` and ultimately `REQUESTER_REVIEW_REQUIRED`:
- Headless execution evolves 1,000,000 individually represented citizens without aggregate conversion.
- Full suite of unit, property, invariant, determinism, and regression tests passes.
- Save/load and replay parity verified.
- Memory consumption documented and verified against the 32GB baseline.
- Interactive GUI connects cleanly to authoritative state via read-only snapshots and provides citizen, household, and settlement inspection.
