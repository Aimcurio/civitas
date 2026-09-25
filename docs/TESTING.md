# CIVITAS-1M Testing & Verification Strategy

## 1. Test Architecture & Verification Matrix

Testing is structured into strict categorical suites:

### 1. Unit Tests (`civitas_core`)
- **Physiology Rules**: Satiety decay, health penalty under starvation, mortality thresholds.
- **Economic Math**: Supply/demand price discovery clamping, wage derivation, trade balance.
- **Geographic Calculations**: Travel cost calculation across biomes and roads, shortest-path calculation between settlements.
- **Household Mechanics**: Resource pooling, expenditure distribution, inheritance/dissolution.

### 2. Property & Invariant Tests
- **Invariant 1 (Exclusivity)**: No citizen belongs to $> 1$ household.
- **Invariant 2 (Bidirectional Consistency)**: Citizen household ID $H \iff$ Member listed in $H$.
- **Invariant 3 (Spatial Validity)**: Citizen settlement ID is valid, or citizen has valid `InTransit` data.
- **Invariant 4 (Demographic Balance)**: Total living + total dead == total spawned.
- **Invariant 5 (Non-Negative Values)**: Wealth, food, commodities $\ge 0$, never NaN or infinite.
- **Invariant 6 (Dead Quiescence)**: Dead citizens never update or consume.

### 3. Determinism & Replay Tests
- **Multi-Run Bit Parity**: Run simulation for 500 ticks from Seed $S$. Repeat from same Seed $S$. Ensure authoritative hashes match identically across every 50-tick interval.
- **Save/Load Identity**: Run 100 ticks $\to$ Save snapshot $\to$ Load into fresh engine $\to$ Run 100 more ticks $\implies$ State matches continuous 200-tick run without save.

### 4. Adversarial Scenarios
- **Zero Population**: Engine runs gracefully without division-by-zero or crashes.
- **Lone Citizen**: Single citizen handles labor, starvation/survival without panic.
- **Famine Collapse**: Sudden 100% loss of food production triggers starvation deaths and mass out-migration without numerical overflow.
- **Extreme Demographics**: Infant mortality 100% or fertility 100% does not break capacity limits or cause unbounded queue allocations.
- **Corrupted Save File**: Truncated or byte-flipped binary save is safely rejected with descriptive error instead of segfaulting or loading garbage.

### 5. Scale & Stress Benchmarks
- Automated headless benchmarks at 10K, 100K, and 1,000,000 citizens measuring:
  - Ticks per second (TPS)
  - Citizens processed per second
  - Total heap memory allocation (MB)
  - Multirate scheduling distribution
