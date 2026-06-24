# ga3d

3D Gaussian Splatting processing tool for AR occlusion and navigation workflows.

## Install

```bash
cmake -S . -B build -DGA3D_BUILD_TESTS=ON
cmake --build build

python3 -m venv python/.venv
python/.venv/bin/pip install -e 'python[dev]'

cd web
npm install
```

## CLI Usage

```bash
python/.venv/bin/ga3d process tests/fixtures/minimal.splat --out dist/job-001
python/.venv/bin/ga3d process scene.sog --config config.yaml --edits edits.json --out dist/job-002
python/.venv/bin/ga3d benchmark --splats 5000000 --out dist/bench-5m
```

`process` writes:

```text
scene.glb
occlusion.glb
navmesh.glb
navmesh.json
manifest.json
```

## Backend API

Run:

```bash
python/.venv/bin/uvicorn ga3d.api:app --app-dir python --reload
```

Endpoints:

```text
POST /api/jobs
GET /api/jobs/{job_id}
GET /api/jobs/{job_id}/artifacts/{name}
```

Job states:

```text
queued -> reading -> editing -> filtering -> voxelizing -> meshing -> navmesh -> exporting -> done
queued -> ... -> failed
```

## Frontend Dev

```bash
cd web
npm run dev
npm test -- --run
npm run build
```

The Vite app provides one operator workspace for server-side input paths, edit recipe controls, processing config, job progress, and artifact downloads. Live browser splat rendering is not available in this build; the UI labels that state clearly and submits recipe JSON to the backend.

## Docker

```bash
docker compose -f docker/docker-compose.yml up --build
```

The container serves the FastAPI backend and static frontend on port `8000`.

## Export Profiles

- `webxr`: GLB artifacts with meter scale, Y-up metadata, hidden occlusion material extras.
- `unity`: same GLB artifacts and manifest metadata; Unity package import is out of scope for v1.
- `both`: write all artifacts once and consume the same manifest in both runtimes.

## Vendor Reference Policy

`vendor/splat-transform` and `vendor/supersplat` are local references only. Product code is implemented in `core`, `python`, and `web`; do not copy vendor files, classes, shaders, or large functions. Small formulas may be reused only with a local source comment and a reason.

## Known Limits

- Python `process` uses the compiled C++ core extension for reading, edit baking, filtering, voxelization, mesh extraction, navmesh generation, and artifact writing.
- Recast is not linked in this workspace; navmesh generation uses the deterministic CPU seed-flood implementation over carved voxels.
- SOG V1/V2 C++ readers support the PlayCanvas metadata paths implemented in `core/src/read_sog.cpp`; automated SOG E2E runs only when real `.sog` fixtures are present.
- Benchmarks run the same synthetic C++ pipeline and report requested splats, processed splats, duration, GLB size, and measured triangle counts.
