# CIVITAS-1M Simulation Model

## 1. Multirate Temporal Scheduling

In a 1,000,000 agent simulation, running every cognitive and structural subsystem on every tick is computationally prohibitive and scientifically unnecessary. CIVITAS-1M uses a **deterministic multirate temporal scheduler**.

### Temporal Units:
- **1 Tick = 1 Simulation Day**
- **7 Ticks = 1 Simulation Week**
- **30 Ticks = 1 Simulation Month**
- **360 Ticks = 1 Simulation Year**

### Subsystem Frequency Tiers:

| Frequency Tier | Cadence | Systems Executed | Scheduling Mechanism |
| :--- | :--- | :--- | :--- |
| **Daily (Fine)** | Every Tick ($T$) | Movement along transit paths, physiological hunger update, daily production output, survival checks. | Bulk contiguous SoA vectorized pass over active entities. |
| **Weekly (Economic)** | Every 7 Ticks | Local market clearing, commodity price adjustments, household budget balancing, labor reallocation. | Sliced deterministically: entity slice $S = \text{EntityID} \pmod 7$ updates on $T \pmod 7 == S$. |
| **Monthly (Social/Migration)** | Every 30 Ticks | Migration push/pull evaluation, household formation/dissolution, marriage proposals, reproductive checks. | Sliced deterministically across 30 daily buckets or evaluated at month boundaries. |
| **Yearly (Demographic)** | Every 360 Ticks | Chronological aging, senescence mortality roll, settlement charter & infrastructure tier updates. | Batch pass over active entities and settlements. |

### Deterministic Slicing
To eliminate computational frame-spikes and cache-thrashing, periodic evaluations (such as weekly job searches and monthly migration decisions) are partitioned into deterministic slices based on `EntityID % Modulus`. This maintains identical causal results while leveling CPU utilization.

---

## 2. Core Behavioral Subsystems

### 2.1 Physiology & Survival
Every citizen maintains physical need metrics:
- **Satiety (`u8` 0–100)**: Decreases by 1 unit daily. If satiety reaches 0, the citizen enters **Starvation** state.
- **Health (`u8` 0–100)**: Decays rapidly during starvation (-10/day) or old age, recovers slowly (+2/day) when satiety $> 70$ and shelter is adequate.
- **Mortality**: If health reaches 0, the citizen dies immediately. Death reason (`Starvation`, `OldAge`, `Disease`) is recorded into causal provenance.

### 2.2 Labor & Occupations
Citizens seek employment matching local settlement demands. Occupations include:
1. **Farmer**: Produces food from fertile land tiles.
2. **Forester**: Produces timber from forest tiles.
3. **Miner**: Produces stone and ore from mineral hills.
4. **Artisan / Crafter**: Consumes raw resources to produce finished goods/tools.
5. **Merchant / Trader**: Facilitates inter-settlement goods transit.
6. **Laborer / Unskilled**: Performs general tasks, infrastructure construction, and maintenance.

- **Skill & Productivity**: Every work cycle increases experience. High skill grants productivity multipliers up to $+100\%$.
- **Wages**: Settlements have wage levels determined by production profitability and labor scarcity.

### 2.3 Household Economy
Households are the fundamental social and consumption unit:
- **Resource Pooling**: Wage earners deposit earnings into the shared household treasury.
- **Bulk Purchasing**: Households purchase food and necessities at the settlement market.
- **Shelter & Rent**: Households occupy housing slots. If housing demand exceeds settlement capacity, overcrowding penalties and rent spikes occur.
- **Formation & Marriage**: Unmarried adult citizens (age 18+) with sufficient personal savings seek partners to establish new independent households.

### 2.4 Settlement Markets & Price Discovery
Each settlement acts as a localized market clearinghouse:
- **Supply & Demand Inventory**: Settlements maintain commodity stocks (Food, Timber, Stone, Tools, Luxury Goods).
- **Price Adjustment Algorithm**:
  $$\text{Price}_{t+1} = \text{Price}_t \times \left(1 + \alpha \cdot \frac{\text{Demand} - \text{Supply}}{\max(\text{Supply}, 1)}\right)$$
  bounded by defined minimum and maximum price clamps.
- **Scarcity Signals**: High prices incentivize citizens to switch occupations toward scarce goods or migrate.

### 2.5 Migration Dynamics
Migration is an emergent response to real settlement differentials:
- **Push Factors**:
  - Sustained hunger / food unaffordability.
  - Unemployment / depressed local wages.
  - Housing deficit / exorbitant rent.
- **Pull Factors**:
  - High real wages (nominal wage divided by food price).
  - Plentiful housing.
  - Family ties in destination settlement.
- **Friction of Distance**:
  $$\text{Migration Utility} = \frac{\text{Pull}(\text{Dest}) - \text{Push}(\text{Origin})}{1 + \beta \cdot \text{Distance}(\text{Origin}, \text{Dest})}$$
- **Physical Transit**:
  Migrating citizens are not instantly teleported. They enter `InTransit` status and spend $D / \text{speed}$ ticks physically traveling along transit paths, visibly moving across the geographic landscape.

### 2.6 Demographics & Provenance
- **Aging**: Age is incremented every 360 ticks.
- **Conception & Birth**: Married fertile couples (ages 18–45) evaluate reproduction probability based on household wealth and nutrition. Upon birth, a new citizen entity is created, inheriting parent references.
- **Lineage**: Every citizen preserves `parent_a` and `parent_b` entity IDs, forming a verifiable ancestral graph.
