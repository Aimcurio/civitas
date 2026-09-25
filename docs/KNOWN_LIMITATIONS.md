# CIVITAS-1M Known Limitations

## 1. Scope Boundaries (V1)
- **Geopolitics & Military**: No sovereign state borders, military units, war, or conquests.
- **Micro-pathfinding**: Agent transit across settlements uses geographic node network routing rather than continuous meter-by-meter tile pathfinding.
- **Financial Architecture**: Bounded commodity economy; no fractional-reserve banking, fiat inflation, or credit derivatives.
- **Rendering LOD**: The visualizer (`civitas_app`) renders aggregated particles, heatmaps, and settlement structures. Individual citizens are inspected via spatial selection and inspector queries, not as 1,000,000 discrete 3D skeletal meshes.
