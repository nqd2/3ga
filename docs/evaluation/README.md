# Evaluation Outputs

`augmented-gaussian-cli benchmark` writes manifest JSON metrics here during report runs.
Use `docs/evaluation/benchmark-config.json` for large-scene voxel benchmarks.

Generate deterministic benchmark scenes:

```bash
cargo run -p augmented-gaussian-cli -- generate-bench-scenes --out target/bench-scenes
```

Run the report benchmarks:

```bash
cargo run -p augmented-gaussian-cli -- benchmark --input target/bench-scenes/bench-500000.splat --out docs/evaluation/results/bench-500k --config docs/evaluation/benchmark-config.json --compare-cpu-gpu
cargo run -p augmented-gaussian-cli -- benchmark --input target/bench-scenes/bench-5000000.splat --out docs/evaluation/results/bench-5m --config docs/evaluation/benchmark-config.json --compare-cpu-gpu
cargo run -p augmented-gaussian-cli -- benchmark --input target/bench-scenes/bench-10000000.splat --out docs/evaluation/results/bench-10m --config docs/evaluation/benchmark-config.json --compare-cpu-gpu
```

Current summary reports:

- `docs/evaluation/results/summary.json`
- `docs/evaluation/results/summary.csv`

Tracked metrics:

- stage timings: decode, alignment, selected voxel backend, CPU voxel, optional GPU voxel, fill, carve, mesh, navmesh, export
- export artifact contract: `scene.sog`, `collision_mesh.json`, `occlusion.glb`, optional `navmesh.glb`/`navmesh.bin`
- CPU/GPU voxel mismatch count when `--compare-cpu-gpu` is enabled
- source byte size, SOG size, optimized GLB size, WebAR ZIP size
- source-to-optimized-GLB and source-to-WebAR-ZIP size ratios
- collision triangle count before and after mesh reduction
- NavMesh triangle count
- geometric error sample count, mean, RMS, P95
