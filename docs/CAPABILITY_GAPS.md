# CIVITAS-1M Capability Gaps & Future Primitives

## 1. Identified Runtime & Tooling Gaps
- **SIMD Serialization**: Standard `bincode` is fast, but serializing 1,000,000 entities can take several hundred milliseconds. Future versions could leverage memory-mapped direct zero-copy buffers (`rkyv` or flatbuffers) for instant checkpointing.
- **Hierarchical Spatial Indexing**: Current network graph distances work well for settlement-to-settlement routing; fine-grained terrain travel could benefit from a hierarchical BVH / H3 hex grid indexing layer.
- **GPU Compute Offload**: Fine physiological updates (daily hunger/satiety math for 1M agents) are purely embarrassingly parallel and could easily run on GPU compute shaders via wgpu/WebGPU in future iterations.
