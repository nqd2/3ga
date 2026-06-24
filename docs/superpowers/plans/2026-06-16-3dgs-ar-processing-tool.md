# 3DGS AR Processing Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a product-grade tool that edits 3D Gaussian Splatting data, converts it into AR-ready geometry, and exports WebXR/Unity-friendly GLB artifacts.

**Architecture:** Use `vendor/splat-transform` as the clean-room reference for import/processing/voxel/collision pipeline and `vendor/supersplat` as the clean-room reference for browser edit workflow. Product code stays separate: C++20 core for heavy processing, Python pybind11/FastAPI/Typer bridge, React Vite frontend for upload/edit/process/export. Vendor code is not copied wholesale; only small formulas or structure notes may be adapted with attribution in comments when unavoidable.

**Tech Stack:** C++20, CMake, pybind11, scikit-build-core, Python 3.11+, FastAPI, Typer, React, Vite, TypeScript, PlayCanvas or Three.js for splat preview, Recast Navigation for navmesh, glTF/GLB writer, Docker.

---

## Reference Findings Locked Into Plan

- `splat-transform` pipeline: CLI parses ordered `input [actions] output [actions]`; `readFile()` loads `.ply`, `.splat`, `.sog`, `.spz`, `.ksplat`, `.lcc`, `.lcc2`; `processDataTable()` applies transform/filter/summary/decimate; `writeFile()` writes `.glb`, `.sog`, `.voxel.json`, `.collision.glb`, etc.
- Core data model: `DataTable` is columnar SoA typed-array storage with lazy spatial transform. C++ core must use columnar arrays, not per-splat heap objects.
- Important format behavior:
  - `.ply`: binary little-endian, chunked read, compressed PLY detection, `vertex` element as table.
  - `.splat`: 32 bytes per splat; position float32, scale linear float32 converted to log scale, RGBA uint8 converted to SH DC + logit opacity, quaternion uint8 normalized.
  - `.sog`: bundled zip or `meta.json`; V2 has WebP textures and codebooks for means/scales/sh0/shN; V1 legacy has per-channel min/max; both must decode to same internal columns.
- Geometry path from `splat-transform`: compute Gaussian extents using rotated ellipsoid AABBs, build BVH, GPU voxelize to 4x4x4 block masks, filter/fill/carve sparse voxel grid, extract collision GLB with marching cubes or voxel faces, then coplanar merge.
- AR path: reuse voxel/collision concepts for occlusion mesh; generate navmesh after floor/exterior fill and capsule carve; keep scale/bounds metadata in manifest.
- SuperSplat frontend path: load through `splat-transform`, convert to `GSplatData`, reorder Morton for render, store per-splat state bits and transform palette, execute edits through undoable `EditOp`s, serialize by baking selection/deletion/transforms/color/opacity into exported data.
- Product edit model: frontend records edit recipe and preview state; backend bakes recipe into canonical splat columns before filtering/voxelization/mesh export.

## File Structure

- Create `docs/vendor-analysis/splat-transform-pipeline.md`: exact behavior notes and parity targets from local vendor files.
- Create `docs/vendor-analysis/supersplat-edit-pipeline.md`: edit state, tool, serialization, and export behavior notes.
- Create `core/include/ga3d/*.hpp` and `core/src/*.cpp`: C++ data model, readers, processors, voxel grid, mesh extraction, navmesh, GLB export.
- Create `python/ga3d/*.py`: pybind11 package, Typer CLI, FastAPI job API, config/schema, artifact manifest.
- Create `web/src/*`: React Vite UI with splat preview, edit tools, job submission, artifact download.
- Create `tests/fixtures/*`: tiny `.ply`, `.splat`, `.sog` fixtures plus generated synthetic scenes for parity/performance tests.

## Task 1: Vendor Pipeline Research Notes

**Files:**
- Create: `docs/vendor-analysis/splat-transform-pipeline.md`
- Create: `docs/vendor-analysis/supersplat-edit-pipeline.md`

- [x] Read and summarize `vendor/splat-transform/src/lib/read.ts`, `readers/read-ply.ts`, `readers/read-splat.ts`, `readers/read-sog.ts`, `process.ts`, `writers/write-voxel.ts`, `writers/collision-glb.ts`.
- [x] Record exact internal columns:

```text
x,y,z
scale_0,scale_1,scale_2
rot_0,rot_1,rot_2,rot_3
f_dc_0,f_dc_1,f_dc_2
opacity
f_rest_0..f_rest_44 when SH bands exist
```

- [x] Record exact transform conventions:

```text
position: raw world/PLY coordinate
scale_*: log scale
opacity: logit, sigmoid(opacity) is alpha
f_dc_*: raw SH DC, color = 0.5 + f_dc * 0.28209479177387814
rotation: rot_0=w, rot_1=x, rot_2=y, rot_3=z
```

- [x] Read and summarize `vendor/supersplat/src/io/read/loader.ts`, `splat.ts`, `edit-ops.ts`, `edit-history.ts`, `splats-transform-handler.ts`, `splat-serialize.ts`.
- [x] Record edit recipe shape for our product:

```json
{
  "version": 1,
  "operations": [
    { "type": "select", "mode": "set", "selector": { "type": "box", "center": [0, 0, 0], "size": [1, 1, 1] } },
    { "type": "deleteSelected" },
    { "type": "transformSelected", "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] }
  ]
}
```

- [x] Add clean-room rule to both docs:

```text
Vendor code is MIT but product implementation must be clean-room. Do not copy files, classes, shaders, or large functions. Reimplement behavior from documented invariants and parity tests. If a small formula is reused, cite local vendor path and explain why.
```

- [x] Commit:

```bash
git add docs/vendor-analysis/splat-transform-pipeline.md docs/vendor-analysis/supersplat-edit-pipeline.md
git commit -m "docs: capture splat vendor pipeline references"
```

## Task 2: Repository Scaffold

**Files:**
- Create: `CMakeLists.txt`
- Modify: `.gitignore`
- Create: `core/CMakeLists.txt`
- Create: `core/include/ga3d/data_table.hpp`
- Create: `core/src/data_table.cpp`
- Create: `python/pyproject.toml`
- Create: `python/ga3d/__init__.py`
- Create: `web/package.json`
- Create: `web/vite.config.ts`
- Create: `web/src/main.tsx`
- Create: `web/index.html`
- Create: `web/tsconfig.json`
- Create: `tests/core/data_table_tests.cpp`

- [x] Create build layout with C++ library target `ga3d_core`, Python package `ga3d`, and React Vite app `web`.
- [x] Define C++ SoA table API:

```cpp
namespace ga3d {
struct DataTable {
  std::vector<float> x, y, z;
  std::vector<float> scale0, scale1, scale2;
  std::vector<float> rot0, rot1, rot2, rot3;
  std::vector<float> fdc0, fdc1, fdc2;
  std::vector<float> opacity;
  std::vector<std::vector<float>> sh_rest;
  std::size_t size() const { return x.size(); }
  void validate() const;
};
}
```

- [x] Add `validate()` checks equal column lengths and required columns.
- [x] Add first C++ unit test:

```cpp
TEST(DataTableTest, RejectsMismatchedColumns) {
  ga3d::DataTable table;
  table.x = {0.0f};
  table.y = {0.0f, 1.0f};
  EXPECT_THROW(table.validate(), std::runtime_error);
}
```

- [x] Run:

```bash
cmake -S . -B build -DGA3D_BUILD_TESTS=ON
cmake --build build
ctest --test-dir build --output-on-failure
```

- [x] Commit:

```bash
git add .gitignore CMakeLists.txt core python tests web docs/superpowers/plans/2026-06-16-3dgs-ar-processing-tool.md
git commit -m "chore: scaffold ga3d product workspace"
```

## Task 3: Format Readers With Vendor Parity

**Files:**
- Create: `core/include/ga3d/readers.hpp`
- Create: `core/src/read_ply.cpp`
- Create: `core/src/read_splat.cpp`
- Create: `core/src/read_sog.cpp`
- Create: `tests/fixtures/minimal.ply`
- Copy as fixture only: `vendor/splat-transform/test/fixtures/splat/minimal.splat` to `tests/fixtures/minimal.splat`
- Create: `tests/core/reader_tests.cpp`

- [x] Implement `.ply` reader: binary little-endian only, `vertex` element, scalar numeric properties, chunked row decoding.
- [x] Implement `.splat` reader with exact conversion:

```cpp
constexpr float SH_C0 = 0.28209479177387814f;
float logit(float p) {
  p = std::clamp(p, 1e-6f, 1.0f - 1e-6f);
  return std::log(p / (1.0f - p));
}
float dc_from_u8(uint8_t value) {
  return ((float(value) / 255.0f) - 0.5f) / SH_C0;
}
```

- [x] Implement `.sog` reader for PlayCanvas bundled `.sog` and unbundled `meta.json`; support V1 and V2 meta. Use independent WebP decode library through a thin adapter; fail fast when texture dimensions cannot hold `count`.
- [x] Add parity tests:

```cpp
TEST(ReadSplatTest, MinimalFixtureLoadsStandardColumns) {
  auto table = ga3d::read_splat("tests/fixtures/minimal.splat");
  EXPECT_GT(table.size(), 0);
  EXPECT_EQ(table.x.size(), table.opacity.size());
  EXPECT_TRUE(std::all_of(table.rot0.begin(), table.rot0.end(), [](float v) { return std::isfinite(v); }));
}
```

- [x] Run vendor oracle command for fixture count:

```bash
cd vendor/splat-transform
npm test -- formats.test.mjs
```

Expected: PASS.

Verification note: exact `npm test -- formats.test.mjs` was run after `npm ci`, but the vendor package script runs all `test/*.test.mjs` and failed on unrelated `test/render-golden.test.mjs` with pending Promise. Focused format oracle `node --import tsx --test test/formats.test.mjs` passed 23/23, including PLY, SOG, and SPLAT suites.

- [x] Run our tests:

```bash
cmake --build build
ctest --test-dir build -R Reader --output-on-failure
```

- [x] Commit:

```bash
git add core tests docs/superpowers/plans/2026-06-16-3dgs-ar-processing-tool.md
git commit -m "feat: add clean-room splat readers"
```

## Task 4: Edit Recipe And Bake Pipeline

**Files:**
- Create: `core/include/ga3d/edit_recipe.hpp`
- Create: `core/src/edit_recipe.cpp`
- Create: `tests/core/edit_recipe_tests.cpp`
- Create: `python/ga3d/schemas.py`

- [x] Implement edit operations:

```json
[
  { "type": "selectAll" },
  { "type": "selectNone" },
  { "type": "selectBox", "mode": "set|add|remove", "center": [0, 0, 0], "size": [1, 1, 1] },
  { "type": "deleteSelected" },
  { "type": "transformSelected", "matrix": [16 numbers] },
  { "type": "filterOpacity", "min": 0.05 }
]
```

- [x] Represent state bits like SuperSplat:

```cpp
enum SplatState : uint8_t {
  Selected = 1 << 0,
  Deleted = 1 << 1,
  Locked = 1 << 2
};
```

- [x] Bake recipe before export: deleted splats are removed; transform changes update position, quaternion, log scale, and SH rotation when SH bands exist.
- [x] Add tests:

```cpp
TEST(EditRecipeTest, DeleteSelectedRemovesRows) {
  auto table = make_two_splat_table();
  ga3d::EditRecipe recipe;
  recipe.ops.push_back(ga3d::SelectAll{});
  recipe.ops.push_back(ga3d::DeleteSelected{});
  auto baked = ga3d::bake_edits(table, recipe);
  EXPECT_EQ(baked.size(), 0);
}
```

- [x] Add Python Pydantic schema matching C++ JSON shape.
- [x] Run:

```bash
ctest --test-dir build -R EditRecipe --output-on-failure
cd python && pytest tests/test_schemas.py -q
```

Verification note: local run used `python/.venv/bin/pytest python/tests/test_schemas.py -q`.

- [x] Commit:

```bash
git add core python tests
git commit -m "feat: add splat edit recipe baking"
```

## Task 5: Filtering, Voxelization, Occlusion Mesh

**Files:**
- Create: `core/include/ga3d/voxel.hpp`
- Create: `core/src/gaussian_extents.cpp`
- Create: `core/src/voxel_grid.cpp`
- Create: `core/src/voxelize.cpp`
- Create: `core/src/collision_mesh.cpp`
- Create: `tests/core/voxel_tests.cpp`

- [x] Implement Gaussian AABB extents from rotated ellipsoid using 3-sigma cutoff.
- [x] Build CPU baseline voxelizer first; keep GPU optional for later. Use 4x4x4 block masks like reference.
- [x] Implement filters:

```text
filter_nan
filter_opacity_min
filter_box
filter_sphere
filter_floaters_by_voxel_contribution
filter_cluster_from_seed
```

- [x] Implement sparse voxel grid post-processing:

```text
align grid bounds to 4x4x4 blocks
filter/fill solid and mixed blocks
floor fill for exterior scenes
exterior fill for enclosed scenes
capsule carve from seed for navigable space
crop to occupied or navigable region
```

- [x] Extract occlusion mesh:

```text
mode=smooth: binary marching cubes then coplanar merge
mode=faces: watertight greedy voxel-face rectangles
```

Implementation note: `MeshMode::Faces` uses greedy voxel-face rectangles. `MeshMode::Smooth` uses a clean-room binary marching-tetrahedra baseline over padded occupancy instead of importing a marching-cubes table from vendor code.

- [x] Add tests:

```cpp
TEST(VoxelTest, FloorAndCubeProduceCollisionTriangles) {
  auto table = make_floor_with_box_splats();
  auto grid = ga3d::voxelize(table, {.voxel_size = 0.1f, .opacity_cutoff = 0.1f});
  auto mesh = ga3d::extract_occlusion_mesh(grid, ga3d::MeshMode::Faces);
  EXPECT_GT(mesh.indices.size(), 0);
  EXPECT_TRUE(mesh.bounds.contains({0.0f, 0.0f, 0.0f}));
}
```

- [x] Run:

```bash
ctest --test-dir build -R Voxel --output-on-failure
```

- [x] Commit:

```bash
git add core tests
git commit -m "feat: generate occlusion mesh from splat voxels"
```

## Task 6: Navigation Mesh

**Files:**
- Create: `core/include/ga3d/navmesh.hpp`
- Create: `core/src/navmesh.cpp`
- Create: `tests/core/navmesh_tests.cpp`

- [x] Feed carved navigable voxel surface into navmesh builder.

Implementation note: Recast headers/libs were not available in the local toolchain, so this task uses a CPU seed-flood baseline over carved voxel surfaces while preserving the planned config/API shape for later Recast replacement.

- [x] Config fields:

```yaml
nav:
  enabled: true
  seed: [0, 0, 0]
  agent_height: 1.6
  agent_radius: 0.2
  max_slope_degrees: 45
  cell_size: 0.1
  cell_height: 0.05
```

- [x] Export `navmesh.glb` and `navmesh.json` with vertices, indices, agent config, bounds, coordinate system.
- [x] Add tests: flat floor has connected nav polygon; cube obstacle creates non-walkable hole.
- [x] Run:

```bash
ctest --test-dir build -R Navmesh --output-on-failure
```

- [x] Commit:

```bash
git add core tests
git commit -m "feat: generate AR navigation mesh"
```

## Task 7: GLB Export And Manifest

**Files:**
- Create: `core/include/ga3d/glb_writer.hpp`
- Create: `core/src/glb_writer.cpp`
- Create: `python/ga3d/manifest.py`
- Create: `tests/core/glb_writer_tests.cpp`

- [x] Export `scene.glb`, `occlusion.glb`, `navmesh.glb`.
- [x] `scene.glb` can be splat GLB with `KHR_gaussian_splatting` for preview and/or mesh GLB when reconstruction is requested.
- [x] `occlusion.glb` uses standard triangle mesh with material metadata:

```json
{
  "name": "GA3D_OCCLUSION",
  "extras": { "ga3dRole": "occlusion", "visible": false }
}
```

- [x] Manifest shape:

```json
{
  "version": 1,
  "source": { "format": "ply|splat|sog", "splatCount": 0 },
  "coordinateSystem": { "upAxis": "Y", "unit": "meter", "scale": 1.0 },
  "bounds": { "min": [0, 0, 0], "max": [0, 0, 0] },
  "artifacts": {
    "scene": "scene.glb",
    "occlusion": "occlusion.glb",
    "navmesh": "navmesh.glb",
    "navmeshJson": "navmesh.json"
  },
  "metrics": { "durationMs": 0, "peakMemoryMb": 0 }
}
```

- [x] Run:

```bash
ctest --test-dir build -R Glb --output-on-failure
python -m pytest python/tests/test_manifest.py -q
```

Verification note: local run used `python/.venv/bin/pytest python/tests/test_manifest.py -q`.

- [x] Commit:

```bash
git add core python tests
git commit -m "feat: export AR glb artifacts"
```

## Task 8: Python CLI And Backend

**Files:**
- Create: `python/ga3d/bindings.cpp`
- Create: `python/ga3d/cli.py`
- Create: `python/ga3d/api.py`
- Create: `python/ga3d/jobs.py`
- Create: `python/tests/test_cli.py`
- Create: `python/tests/test_api.py`

- [x] Expose core API:

```python
from ga3d import process

process(
    input_path="scene.sog",
    output_dir="dist/job-001",
    config_path="config.yaml",
    edit_recipe_path="edits.json",
)
```

- [x] CLI command:

```bash
ga3d process input.sog --config config.yaml --edits edits.json --out dist/job-001
```

- [x] FastAPI endpoints:

```text
POST /api/jobs
GET /api/jobs/{job_id}
GET /api/jobs/{job_id}/artifacts/{name}
```

- [x] Job states:

```text
queued -> reading -> editing -> filtering -> voxelizing -> meshing -> navmesh -> exporting -> done
queued -> ... -> failed
```

- [x] Tests use tiny fixtures and assert artifact names exist.
- [x] Run:

```bash
cd python
pip install -e .[dev]
pytest -q
```

Verification note: local run used `python/.venv/bin/pip install -e 'python[dev]'` and `python/.venv/bin/pytest python/tests -q`.

- [x] Commit:

```bash
git add python
git commit -m "feat: expose ga3d cli and backend"
```

## Task 9: React Vite Frontend With Splat Editing

**Files:**
- Create: `web/src/app/App.tsx`
- Create: `web/src/splat/SplatViewer.tsx`
- Create: `web/src/edit/EditStore.ts`
- Create: `web/src/edit/EditToolbar.tsx`
- Create: `web/src/jobs/JobPanel.tsx`
- Create: `web/src/api/client.ts`
- Create: `web/src/styles.css`
- Create: `web/tests/edit-store.test.ts`

- [x] Build single work screen: upload, preview/edit, processing config, job progress, artifact downloads.
- [x] Implement edit store independent of vendor:

```ts
export type EditOperation =
  | { type: 'selectAll' }
  | { type: 'selectNone' }
  | { type: 'selectBox'; mode: 'set' | 'add' | 'remove'; center: [number, number, number]; size: [number, number, number] }
  | { type: 'deleteSelected' }
  | { type: 'transformSelected'; matrix: number[] }
  | { type: 'filterOpacity'; min: number };

export type EditRecipe = { version: 1; operations: EditOperation[] };
```

- [x] Learn from SuperSplat UX but implement own controls: select all/none, box select, delete selected, move/rotate/scale gizmo, undo/redo, opacity filter.
- [x] Preview selected/deleted state in browser. Keep preview responsive for large scenes by downsampling client preview while backend processes full input.
- [x] Submit upload + config + edit recipe to backend before export.
- [x] Test edit store:

```ts
it('records undoable delete-selected recipe', () => {
  const store = createEditStore();
  store.dispatch({ type: 'selectAll' });
  store.dispatch({ type: 'deleteSelected' });
  expect(store.recipe.operations).toEqual([
    { type: 'selectAll' },
    { type: 'deleteSelected' }
  ]);
});
```

- [x] Run:

```bash
cd web
npm install
npm test -- --run
npm run build
```

- [x] Commit:

```bash
git add web
git commit -m "feat: add splat edit frontend"
```

## Task 10: End-To-End Product Flow

**Files:**
- Create: `tests/e2e/test_process_fixtures.py`
- Create: `docker/Dockerfile`
- Create: `docker/docker-compose.yml`
- Modify: `README.md`

- [x] Add e2e scenarios:

```text
PLY -> edit recipe -> scene.glb + occlusion.glb + navmesh.glb + manifest.json
SPLAT -> edit recipe -> same artifacts
SOG V1/V2 -> edit recipe -> same artifacts
```

- [x] Add performance benchmark:

```bash
ga3d benchmark --splats 5000000 --out dist/bench-5m
ga3d benchmark --splats 10000000 --out dist/bench-10m
```

Expected report fields:

```json
{
  "splatCount": 10000000,
  "peakMemoryMb": 0,
  "durationMs": 0,
  "sceneGlbMb": 0,
  "occlusionTriangleCount": 0,
  "navmeshTriangleCount": 0
}
```

- [x] Docker serves backend and static frontend.
- [x] README documents:

```text
install
CLI usage
backend API
frontend dev
export profiles: webxr, unity, both
vendor reference policy
known limits for huge scenes and SOG variants
```

- [x] Run full checks:

```bash
cmake --build build
ctest --test-dir build --output-on-failure
cd python && pytest -q
cd ../web && npm run build
python tests/e2e/test_process_fixtures.py
```

Verification note: local run used `python/.venv/bin/pytest python/tests -q` and `python/.venv/bin/python tests/e2e/test_process_fixtures.py`.

- [x] Commit:

```bash
git add README.md docker tests
git commit -m "test: add end-to-end ga3d product flow"
```

## Acceptance Criteria

- Reads `.ply`, `.splat`, `.sog` PlayCanvas V1/V2 into canonical columns.
- User can edit splat before export: select, delete/crop, transform, opacity filter, undo/redo.
- Backend bakes edit recipe before generating geometry.
- Outputs `scene.glb`, `occlusion.glb`, `navmesh.glb`, `navmesh.json`, `manifest.json`.
- Manifest preserves scale, unit, coordinate system, source count, bounds, metrics.
- WebXR profile loads optimized GLB artifacts.
- Unity profile provides import-safe GLB artifacts and metadata; no Unity package in v1.
- 5M and 10M synthetic benchmarks produce memory/runtime/output-size report.
- Vendor code remains untouched except fixture copy. Product source has no wholesale copied vendor files or large copied functions.

## Sources

- Local reference: `vendor/splat-transform` from PlayCanvas.
- Local reference: `vendor/supersplat` from PlayCanvas.
- Upstream `splat-transform`: https://github.com/playcanvas/splat-transform
- Upstream `supersplat`: https://github.com/playcanvas/supersplat
