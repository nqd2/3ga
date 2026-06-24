# SuperSplat Edit Pipeline Notes

Source reference: `vendor/supersplat` from PlayCanvas.

Clean-room rule: vendor code is MIT, but product implementation must stay clean-room. Do not copy files, classes, shaders, or large functions. Reimplement behavior from documented invariants and parity tests. If a small formula is reused, cite the local vendor path and explain why.

## Scope

Use this document as the implementation reference for `ga3d` browser editing before GLB export. Product frontend will be React Vite, not a fork of SuperSplat. The goal is to learn the edit model and pipeline, then implement our own minimal AR-focused editor.

Primary files reviewed:

- `vendor/supersplat/src/io/read/loader.ts`
- `vendor/supersplat/src/splat.ts`
- `vendor/supersplat/src/splat-state.ts`
- `vendor/supersplat/src/selection.ts`
- `vendor/supersplat/src/edit-ops.ts`
- `vendor/supersplat/src/edit-history.ts`
- `vendor/supersplat/src/command-queue.ts`
- `vendor/supersplat/src/splats-transform-handler.ts`
- `vendor/supersplat/src/tools/tool-manager.ts`
- `vendor/supersplat/src/tools/brush-selection.ts`
- `vendor/supersplat/src/tools/transform-tool.ts`
- `vendor/supersplat/src/data-processor/index.ts`
- `vendor/supersplat/src/data-processor/intersect.ts`
- `vendor/supersplat/src/splat-serialize.ts`
- `vendor/supersplat/src/doc.ts`

## Load And Preview Flow

Reference: `src/io/read/loader.ts`.

Behavior:

- SuperSplat uses `@playcanvas/splat-transform` for all splat file parsing.
- `getInputFormat()` detects format.
- `.sog` bundle is mounted as a zip and inner `meta.json` is read.
- `DataTable` is converted to PlayCanvas `GSplatData`.
- Missing `scale_2` for 2D splats is patched with near-zero log scale.
- Non-SOG and non-compressed PLY files are reordered by Morton order for render performance.
- The loader validates required Gaussian properties before adding a scene object.

Product mapping:

- Browser preview should use the same format support surface as backend: `.ply`, `.splat`, `.sog`, `meta.json`.
- Preview may use PlayCanvas if we want `GSplatData` compatibility; Three.js can be used for GLB output preview, but native splat edit preview is easier with PlayCanvas.
- Large scenes should be preview-downsampled client-side while backend keeps full-resolution data.
- Do not require frontend preview decode to be the source of truth. Backend always rereads original upload and bakes edit recipe.

## Splat Scene Object

Reference: `src/splat.ts`.

SuperSplat wraps one splat asset as a `Splat` object with:

- PlayCanvas `Entity` containing a `gsplat` component.
- Original `GSplatData`.
- Per-splat state property named `state`.
- Per-splat transform index property named `transform`.
- GPU `stateTexture` in R8 format.
- GPU `transformTexture` in R16U format.
- `TransformPalette` containing matrices for per-splat transform groups.
- Cached local, world, and selection bounds.
- Color adjustment fields: tint, temperature, saturation, brightness, black/white point, transparency.

State bits:

```text
selected = bit 0
deleted = bit 1
locked = bit 2
```

Important behavior:

- State has CPU mirror plus GPU texture.
- `updateState()` flushes dirty state, updates counts, updates sorting/bounds, and fires events.
- Deleted splats are hidden by sorter mapping.
- Transformed selected splats update sorter centers and bounds.

Product mapping:

- Product edit store should mirror state bits conceptually, but not depend on SuperSplat classes.
- Store edit intent as recipe operations, not as the only copy of mutated GPU buffers.
- Frontend preview can maintain a compact per-splat state texture or CPU typed array for selection/deletion display.
- Backend must bake final recipe into canonical columns before geometry export.

## Selection Model

References:

- `src/selection.ts`
- `src/edit-ops.ts`
- `src/data-processor/intersect.ts`
- `src/tools/brush-selection.ts`

Selection behavior:

- Exactly one `Splat` is active selection target at scene level.
- Per-splat selection is stored in state bits.
- Selection operations use three modes: `add`, `remove`, `set`.
- Selection ignores locked/deleted splats.
- `set` toggles only rows where current selected state differs from hit mask.

Selection tools found:

```text
select all
select none
select invert
brush mask
rectangle
polygon
lasso
sphere
box
flood
eyedropper
range/histogram
```

GPU selection path:

- Tools produce either screen mask, rect, sphere, box, or histogram range.
- `DataProcessor.intersect()` renders a packed mask to a small render target.
- Readback returns per-splat hit bytes.
- `SelectOp` converts hit mask to indexed ranges and mutates selected bit.

Product v1 selection scope:

```text
selectAll
selectNone
selectBox
deleteSelected
transformSelected
filterOpacity
```

Optional later:

```text
brush mask
lasso
sphere
histogram/range
flood
```

Required product recipe shape:

```json
{
  "version": 1,
  "operations": [
    { "type": "selectAll" },
    { "type": "selectNone" },
    { "type": "selectBox", "mode": "set", "center": [0, 0, 0], "size": [1, 1, 1] },
    { "type": "deleteSelected" },
    { "type": "transformSelected", "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] },
    { "type": "filterOpacity", "min": 0.05 }
  ]
}
```

## Edit Operations

Reference: `src/edit-ops.ts`.

SuperSplat edit operations implement:

```text
do()
undo()
destroy?()
name
```

Core ops:

- `StateOp`: set, clear, or toggle selected/deleted/locked bits on `IndexRanges`.
- `SelectAllOp`
- `SelectNoneOp`
- `SelectInvertOp`
- `SelectOp`
- `HideSelectionOp`
- `UnhideAllOp`
- `DeleteSelectionOp`
- `ResetOp`
- `EntityTransformOp`
- `SplatsTransformOp`
- `PlacePivotOp`
- `SetSplatColorAdjustmentOp`
- `MultiOp`
- `AddSplatOp`
- `SplatRenameOp`

Important invariants:

- Delete is reversible by setting/clearing deleted bit, not by immediately removing data.
- Transform selected splats by assigning new transform palette indices to selected rows.
- Transform op stores a palette map so undo can restore old transform indices.
- `MultiOp` groups logically atomic changes, such as splat transform plus pivot placement.

Product mapping:

- Frontend undo/redo can operate on edit recipe stack and preview state.
- Backend does not need undo. It only receives final recipe.
- Recipe baking should implement deterministic replay:

```text
start with all state bits clear
for each operation:
  update selection/deletion state or transform accumulator
after replay:
  remove deleted rows
  bake per-row transform into position, rotation, scale, and SH when available
  apply opacity filter
```

## Edit History And Command Queue

References:

- `src/edit-history.ts`
- `src/command-queue.ts`

Behavior:

- History has `history[]` and `cursor`.
- Adding an op drops redo history after cursor.
- Undo/redo advance cursor only after operation succeeds.
- `CommandQueue` serializes async mutations so GPU readbacks, sort updates, and history changes do not race.
- Removing a splat removes history ops referencing that splat.

Product mapping:

- React edit store must serialize mutations.
- UI should disable export while an edit operation or preview readback is in flight.
- Undo/redo should not mutate committed backend jobs. Each job captures a snapshot of the current recipe.

Suggested frontend model:

```ts
type EditHistoryState = {
  operations: EditOperation[];
  cursor: number;
  pending: boolean;
};
```

Rules:

- `dispatch(op)` truncates `operations` at cursor, appends op, applies to preview, increments cursor.
- `undo()` decrements cursor and rebuilds preview state from operations `[0,cursor)`.
- `redo()` increments cursor and rebuilds preview state from operations `[0,cursor)`.
- `recipe` sent to backend is `{ version: 1, operations: operations.slice(0, cursor) }`.

## Transform Editing

References:

- `src/tools/transform-tool.ts`
- `src/splats-transform-handler.ts`
- `src/transform-palette.ts`
- `src/splat-serialize.ts`

Behavior:

- Transform gizmo attaches to a pivot.
- Pivot starts from selection center or bound center.
- On drag start, selected splats get new transform palette entries.
- On drag update, palette matrices update live for preview.
- On drag end, a `SplatsTransformOp` is recorded.
- Export bakes transform palette into splat data.

Product v1:

- Use one transform matrix per `transformSelected` recipe op.
- Apply matrix to currently selected rows during backend replay.
- For frontend preview, a transform gizmo may show selected rows transformed through shader or CPU downsampled preview.
- Preserve world scale and coordinate system in manifest.

Backend bake requirements:

- Position: multiply point by matrix.
- Rotation: compose operation rotation with existing quaternion.
- Scale: multiply linear scale by matrix scale, then convert back to log scale.
- SH: rotate SH coefficients when SH bands exist.
- Opacity and DC color are unchanged unless explicit filter/color op exists.

## Serialization And Export

Reference: `src/splat-serialize.ts`.

SuperSplat export behavior:

- `GaussianFilter` removes deleted splats.
- Optional selected-only export.
- Optional minimum opacity filter.
- Optional invalid value removal.
- Internal editor props `state` and `transform` are removed unless document save needs state.
- Common properties across splats are exported.
- `SingleSplat` reads source properties, applies entity transform, applies transform palette, rotates SH, applies color grading/transparency, writes final values.
- `extractDataTable()` builds a `splat-transform DataTable` for SOG/viewer writers.
- `serializePly`, `serializeSog`, and viewer export share the same bake/extract logic.

Product mapping:

- `ga3d` backend should own export truth.
- Frontend recipe is not trusted for geometry data. It is only edit instructions.
- Backend reads original input, replays recipe, then generates:

```text
scene.glb
occlusion.glb
navmesh.glb
navmesh.json
manifest.json
```

- Keep internal edit state out of exported GLB unless saved as `extras.ga3d`.
- Manifest must include original count, kept count, deleted count, opacity-filtered count, transform count, and bounds.

## Document Save Lessons

Reference: `src/doc.ts`.

SuperSplat document format:

- `.ssproj` is a zip.
- `document.json` contains camera, view, pose sets, timeline, and splat settings.
- Each splat is stored as `splat_N.ply`.
- Save can keep world transform and color tint for editor project persistence.

Product v1 does not need `.ssproj`, but should support a lightweight project file later:

```json
{
  "version": 1,
  "source": "uploaded-file-id-or-path",
  "config": {},
  "editRecipe": {},
  "camera": {}
}
```

## Frontend UI Plan Impact

Build actual tool screen, not landing page:

- Left: source upload, file info, export profile.
- Center: splat preview/edit viewport.
- Top or side toolbar: select, box crop, move, rotate, scale, delete, undo, redo.
- Right: processing config for voxel, occlusion, navmesh, output.
- Bottom: job status, logs, artifacts.

Avoid copying SuperSplat UI assets. Use our own icons/components.

Minimum edit actions for first usable AR pipeline:

```text
load preview
select all
box select/crop
delete selected
move/rotate/scale selected
opacity filter
undo/redo
submit edits to backend
download GLB artifacts
```

## Parity And Tests

Frontend unit tests:

- Dispatch appends operation and updates cursor.
- Undo/redo rebuilds recipe slice correctly.
- Delete selected records reversible preview state.
- Export snapshot freezes current recipe.

Backend bake tests:

- `selectAll -> deleteSelected` returns zero rows.
- `selectBox -> deleteSelected` removes only rows inside box.
- `transformSelected(identity)` is no-op.
- Translation matrix changes selected positions only.
- Uniform scale matrix updates log scale by `log(factor)`.
- Opacity filter removes rows with `sigmoid(opacity) < min`.

Integration tests:

- Upload fixture, apply edit recipe, process job, verify manifest counts.
- Edited bounds differ from unedited bounds when translation is applied.
- Deleted rows do not contribute to occlusion mesh.

## Implementation Decisions For ga3d

- Do not fork SuperSplat. Use React Vite with own state model.
- Use PlayCanvas for native splat preview if feasible; use Three.js for output GLB preview if simpler.
- Store edits as declarative JSON operations.
- Backend is source of truth for final artifacts.
- Bake edits before filtering, voxelization, occlusion mesh, and navmesh.
- Keep vendor fixtures for parity, but never import vendor source into production modules.
