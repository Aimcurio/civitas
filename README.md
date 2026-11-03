# CIVITAS-1M: Frontier Civilization Simulation Engine

![Status](https://img.shields.io/badge/Status-Qualified_Local_Slice-green?style=flat-square) ![Language](https://img.shields.io/badge/Language-Rust_2021-orange?style=flat-square) ![Ecosystem](https://img.shields.io/badge/Ecosystem-Aimcurio_High--Assurance-blue?style=flat-square) ![License](https://img.shields.io/badge/License-MIT_OR_Apache--2.0-lightgrey?style=flat-square)

**CIVITAS-1M** is a frontier-scale civilization simulation engine engineered in native Rust to model emergent human settlements, macroeconomic price discovery, resource production graphs, trade networks, and demographic lifecycles for up to 1,000,000 individual citizens.

---

## Executive Overview

CIVITAS-1M provides a deterministic, high-assurance simulation substrate for exploring emergent societal dynamics. By modeling individual agents with physiological needs, economic behaviors, and generational lifecycles across spatial environments, CIVITAS-1M enables research into macroeconomic equilibrium, settlement resilience, and institutional stability under controlled computational conditions.

---

## Architectural Boundaries & Key Capabilities

- **Deterministic Simulation Kernel (`crates/engine`):** Tick-based state transitions with complete reproducibility across random seeds.
- **Demographic & Lifecycle Modeling (`crates/demographics`):** Age cohorts, mortality curves, generational turnover, and household formation.
- **Macroeconomic Production & Trade (`crates/economics`):** Double-auction local markets, bilateral trade arbitrage, supply-demand price discovery, and inventory depreciation.
- **Spatial Grid & Settlement Growth (`crates/spatial`):** Spatial clustering, land-use zoning, and transport friction.
- **Snapshot & Replay System:** Canonical state serialization, delta diffing, and deterministic session replay.

---

## Verified Maturity & Current Evidence

| Metric | Verified Status | Evidence Reference |
| :--- | :--- | :--- |
| **Maturity Tier** | **QUALIFIED / VERIFIED LOCAL SLICE** | `E:\projects\civitas` (branch `bubba/civitas-full-remediation`) |
| **Automated Test Suite** | **34 / 34 PASS** (100%) | `crates/engine/tests`, `crates/economics/tests` |
| **Demonstrated Scale** | Up to 100,000 concurrent agents (local workstation); benchmark suite verified | Snapshot regression tests |
| **Active Staging** | Pull Request #3 staged (`bubba/civitas-full-remediation` -> `main`) | Commit `fa359ad` |
| **Planned / In-Progress** | WebAssembly/WebGPU visualizer; distributed cluster execution up to 1,000,000 agents | Milestone Roadmap v0.2 |

---

## Quickstart & Verification

### Prerequisites
- Rust 1.78+ (2021 Edition) & Cargo
- CMake (for optional profiling hooks)

### Build & Test
```powershell
# Clone repository
git clone https://github.com/Aimcurio/civitas.git
cd civitas

# Compile release binary
cargo build --release --workspace

# Run full automated test suite
cargo test --workspace

# Run CLI benchmark
cargo run --release --bin civitas-cli -- --help
```

---

## Governance & Integrity Boundary
CIVITAS-1M operates under the **Aimcurio BUBBA 2.0 High-Assurance Standard**. All simulation state changes require verifiable causal attribution and deterministic replay parity.
