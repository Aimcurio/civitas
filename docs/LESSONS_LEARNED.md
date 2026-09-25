# CIVITAS-1M Lessons Learned & Design Insights

## 1. Architectural Insights
- **Hand-Rolled ECS vs Bevy ECS**: Using `bevy_ecs` provides a battle-tested Archetype/Table Structure-of-Arrays storage model. It completely circumvents heap fragmentation associated with object-oriented citizen models while eliminating the massive boilerplate of hand-rolling unsafe pointer arrays.
- **Strict Collection Sanitization**: Standard Rust `HashMap` hashes with random seeds per process, which quietly destroys deterministic simulation. Mandating `BTreeMap` and `indexmap` from Day 1 guarantees reproducibility.
- **Multirate Deterministic Slicing**: Rather than letting weekly or monthly tasks cause massive tick spikes, dividing tasks into `entity_id % slice` buckets ensures smooth frame times while preserving deterministic causal ordering.

## 2. Reusable Primitives Discovered
- Deterministic multirate tick scheduler.
- Compact `DecisionTrace` reason encoder for explainable simulation agents.
- Checksummed state snapshot persistence pipeline (`bincode` + CRC32).
