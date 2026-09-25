# CIVITAS-1M Performance Specification & Benchmark Harness

## 1. Instrumentation Requirements
The simulation must instrument and report:
- **Simulation Ticks Per Second (TPS)**
- **Simulated Calendar Days / Years Per Second**
- **Citizens Processed Per Second (Throughput)**
- **Total Authoritative Memory Footprint (RSS / Heap Allocations)**
- **Per-Subsystem Execution Breakdown**:
  - Fine/Daily survival & movement
  - Weekly market clearing & labor matching
  - Monthly migration & household updates
  - Demographic aging & reproduction

## 2. Benchmark Scale Tiers
Benchmarks are executed headlessly via `civitas_cli`:
1. **Tier 1 (10,000 citizens)**: Baseline verification, fast cycle.
2. **Tier 2 (100,000 citizens)**: Medium settlement cluster, macro-dynamics emerging.
3. **Tier 3 (1,000,000 citizens)**: Full frontier-scale evaluation proving 1M persistent individuals run without aggregate collapse.

## 3. Reporting Standards
All reported metrics must specify:
- Host CPU, Core Count, Total RAM, OS
- Rust toolchain and compiler flags (`--release`, opt-level 3, codegen-units)
- Benchmark duration and tick count
- Peak memory footprint measured on hardware
