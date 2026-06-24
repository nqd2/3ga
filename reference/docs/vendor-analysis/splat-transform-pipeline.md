# splat-transform Pipeline Notes

Source reference: `vendor/splat-transform` from PlayCanvas.

Clean-room rule: vendor code is MIT, but product implementation must stay clean-room. Do not copy files, classes, shaders, or large functions. Reimplement behavior from documented invariants and parity tests. If a small formula is reused, cite the local vendor path and explain why.

## Scope

Use this document as the implementation reference for `ga3d` import, processing, voxel, collision, and export behavior. Product code must live outside `vendor/`.

Primary files reviewed:

- `vendor/splat-transform/src/cli/index.ts`
- `vendor/splat-transform/src/lib/read.ts`
- `vendor/splat-transform/src/lib/readers/read-ply.ts`
- `vendor/splat-transform/src/lib/readers/read-splat.ts`
- `vendor/splat-transform/src/lib/readers/read-sog.ts`
- `vendor/splat-transform/src/lib/readers/read-sog-v1.ts`
- `vendor/splat-transform/src/lib/process.ts`
- `vendor/splat-transform/src/lib/data-table/data-table.ts`
- `vendor/splat-transform/src/lib/data-table/transform.ts`
- `vendor/splat-transform/src/lib/data-table/gaussian-aabb.ts`
- `vendor/splat-transform/src/lib/writers/write-voxel.ts`
- `vendor/splat-transform/src/lib/writers/collision-glb.ts`
- `vendor/splat-transform/src/lib/voxel/voxelize.ts`
- `vendor/splat-transform/src/lib/voxel/filter-floaters.ts`
- `vendor/splat-transform/src/lib/voxel/filter-cluster.ts`
- `vendor/splat-transform/src/lib/voxel/carve.ts`
- `vendor/splat-transform/src/lib/mesh/marching-cubes.ts`
- `vendor/splat-transform/src/lib/mesh/voxel-faces.ts`
- `vendor/splat-transform/src/lib/mesh/coplanar-merge.ts`
- `vendor/splat-transform/src/lib/writers/write-glb.ts`

## CLI Flow

`splat-transform` treats the command as an ordered chain:

```text
splat-transform [GLOBAL] input [ACTIONS] ... output [ACTIONS]
```

Important behavior to mirror:

- Positional args are files. Options after a file attach to that file as ordered actions.
- Input files are read into one or more `DataTable`s.
- Multiple input tables can be combined before output.
- The last file is the output. Actions after it are applied before writing.
- URL inputs are supported by splitting URL into base directory plus leaf name so sibling files for SOG can be fetched.
- Optional-value flags use defaults when bare, especially voxel/filter options.

For `ga3d`, keep a simpler public CLI, but internally preserve ordered stages:

```text
read -> bake edits -> user filters -> voxel/collision/navmesh -> export artifacts
```

## Input Format Detection

Reference: `src/lib/read.ts`.

Supported by the reference:

```text
.ply
.compressed.ply
.splat
.sog
meta.json
.spz
.ksplat
.lcc
.lcc2
.mjs
```

Required by `ga3d` v1:

```text
.ply
.splat
.sog
meta.json
```

Detection rules to keep:

- Strip URL query and hash only from basename before extension checks.
- `.sog` is a bundled zip. Mount zip and read inner `meta.json`.
- `meta.json` is unbundled SOG. Read sibling WebP payloads from same file system.
- Unsupported extension fails early with a clear error.

## Canonical Data Model

Reference: `src/lib/data-table/data-table.ts`.

The core storage is columnar Structure of Arrays. This is critical for 5-10M splats. Product C++ must not use per-splat heap objects.

Required columns:

```text
x,y,z
scale_0,scale_1,scale_2
rot_0,rot_1,rot_2,rot_3
f_dc_0,f_dc_1,f_dc_2
opacity
f_rest_0..f_rest_44 when SH bands exist
```

Transform conventions:

```text
position: raw world/PLY coordinate
scale_*: log scale
opacity: logit; sigmoid(opacity) is alpha
f_dc_*: raw SH DC; color = 0.5 + f_dc * 0.28209479177387814
rotation: rot_0=w, rot_1=x, rot_2=y, rot_3=z
```

Reference behavior:

- `DataTable` validates equal column lengths.
- `clone({ rows, columns })` copies selected rows/columns.
- `permuteRowsInPlace(indices)` reorders every column with buffer reuse.
- `transform` is stored beside raw data and baked only when a consumer needs a specific coordinate space.

Parity target for `ga3d`:

- C++ `DataTable` keeps separate contiguous vectors for each canonical column.
- All row filters return either selected row indices or a compacted `DataTable`.
- Internal transforms are explicit and only baked before export, voxelization, or value filters on spatial columns.

## PLY Reader

Reference: `src/lib/readers/read-ply.ts`.

Behavior:

- Read ASCII header up to `end_header`.
- Support binary little-endian scalar properties.
- Map PLY numeric property types to typed arrays.
- Decode each element into a `DataTable`.
- Use `vertex` element as splat table.
- Detect compressed PLY and decompress before returning canonical data.
- Set returned transform to PLY coordinate space.

Implementation target:

- Start with binary little-endian `vertex` support.
- Preserve all known vertex properties, but validate canonical columns before processing.
- Use chunked reads for large files.
- Fail on missing `vertex`, unsupported property type, invalid header, or inconsistent row size.

Parity tests:

- Minimal PLY loads exact row count and canonical columns.
- Missing `vertex` element fails.
- Non-float mixed numeric properties decode correctly.
- Compressed PLY fixture can be added after base PLY reader lands.

## SPLAT Reader

Reference: `src/lib/readers/read-splat.ts`.

Format:

```text
32 bytes per splat
0..11: position x,y,z as float32 little-endian
12..23: scale x,y,z as float32 little-endian, linear scale
24..27: red, green, blue, opacity as uint8
28..31: quaternion components as uint8
```

Conversion:

- Scale is converted from linear to log scale.
- RGB is converted from 0-255 to SH DC using `SH_C0 = 0.28209479177387814`.
- Opacity is converted from 0-255 alpha to logit, clamped away from 0 and 1.
- Quaternion bytes are mapped from `[0,255]` to `[-1,1]`, normalized, and default to identity if zero length.
- Returned transform is PLY coordinate space.

Small formulas allowed to reuse with attribution:

```text
alpha = clamp(byte / 255, 1e-6, 1 - 1e-6)
opacity = ln(alpha / (1 - alpha))
f_dc = ((byte / 255) - 0.5) / 0.28209479177387814
```

Parity tests:

- File size not divisible by 32 fails.
- Empty file fails.
- Minimal vendor fixture loads finite rotations and equal column lengths.
- Known hand-authored one-splat fixture decodes expected position, scale log, opacity logit, and identity fallback.

## SOG Reader

References:

- `src/lib/readers/read-sog.ts`
- `src/lib/readers/read-sog-v1.ts`
- `src/lib/writers/write-sog.ts`

SOG is PlayCanvas super-compressed splat data built from metadata plus lossless WebP textures. There are two layouts to support.

V2 meta:

```text
version: 2
count
means: mins, maxs, files
scales: codebook, files
quats: files
sh0: codebook, files
shN optional: count, bands, codebook, files
```

V1 legacy meta:

```text
no version field
means.shape, mins, maxs, files
scales.mins, scales.maxs, files
quats.files
sh0.mins, sh0.maxs, files
shN optional with mins, maxs, files
```

Decode invariants:

- Bundled `.sog` is a zip containing `meta.json` and WebP textures.
- Means use `means_l.webp` and `means_u.webp` to reconstruct 16-bit values.
- Means are stored through signed log transform: `sign(x) * ln(abs(x) + 1)`, then inverted.
- V2 scales and colors use 8-bit labels into codebooks.
- V1 scales and `sh0` use per-channel min/max interpolation.
- Quaternions use largest-component packing: tag byte `252..255` marks largest component, other three are scaled by `sqrt(2)` and reconstructed.
- V2 `shN` uses label texture plus centroid texture plus shared codebook.
- V1 `shN` infers band count from centroid texture width: `192 -> band 1`, `512 -> band 2`, `960 -> band 3`.
- Texture dimensions must be validated against `count`; fail fast on truncated payloads.

Parity tests:

- Bundled `.sog` and unbundled `meta.json` produce same row count.
- V2 meta with invalid version fails.
- V1 meta without version routes to V1 decoder.
- Invalid quaternion tag defaults to identity for that splat.
- Too-small WebP payload dimensions fail.

## Processing Actions

Reference: `src/lib/process.ts`.

Actions to map into `ga3d` config or edit recipe:

```text
translate
rotate
scale
filterNaN
filterByValue
filterBands
filterBox
filterSphere
filterFloaters
filterCluster
lod
summary
mortonOrder
decimate
```

Important value-space rules:

- User-facing opacity is linear alpha. Internal opacity is logit.
- User-facing scale is linear. Internal scale is log.
- User-facing DC color is linear 0-1. Internal value is SH DC.
- `_raw` suffix bypasses user-friendly conversion in `filterByValue`.
- Spatial filters must account for pending transform or bake to target space first.
- SH bands are removed by deleting or remapping `f_rest_*` columns.
- Morton order improves spatial locality and should be used before web/export layouts where helpful.
- Decimation uses progressive pairwise Gaussian merging in the reference. `ga3d` may defer equivalent Gaussian decimation until after geometry path exists, but must keep the config surface separate from mesh simplification.

## Gaussian Extents And BVH

Reference: `src/lib/data-table/gaussian-aabb.ts`.

Behavior:

- Each Gaussian is treated as an oriented ellipsoid.
- Scale is `exp(scale_i)`.
- Quaternion is normalized.
- Local half-extents use a sigma cutoff matching rasterizer radius.
- Rotated ellipsoid AABB is computed per splat.
- Scene bounds are the union of all per-splat AABBs.
- Invalid extents are logged and set to zero.

Parity target:

- `ga3d` must compute bounds from Gaussian extents, not just center positions.
- Bounds in `manifest.json` must reflect geometry/voxel input bounds after edits and transforms.

## Voxel And Collision Pipeline

References:

- `src/lib/writers/write-voxel.ts`
- `src/lib/voxel/filter-pipeline.ts`
- `src/lib/voxel/voxelize.ts`
- `src/lib/voxel/filter-floaters.ts`
- `src/lib/voxel/filter-cluster.ts`
- `src/lib/voxel/fill-exterior.ts`
- `src/lib/voxel/fill-floor.ts`
- `src/lib/voxel/carve.ts`
- `src/lib/writers/collision-glb.ts`

Reference stage order:

```text
select voxel columns
bake transform to engine/world space
compute Gaussian extents
build Gaussian BVH
create GPU voxelizer
align grid bounds to 4x4x4 block boundaries
voxelize Gaussian opacity into block masks
filter and fill block masks
load SparseVoxelGrid
optional exterior fill
optional floor fill
optional capsule carve from seed
crop to occupied or navigable region
optional collision mesh extraction
write voxel json/bin and collision glb
```

Grid conventions:

- Voxel grid is grouped into 4x4x4 blocks.
- Bounds are aligned to block boundaries.
- Fill operations need padding around tight Gaussian bounds.
- `filterFloaters` removes Gaussians that do not contribute to any occupied voxel.
- `filterCluster` keeps Gaussians contributing to connected component near seed.
- `carve` dilates blocked cells by agent capsule radius/height, flood-fills reachable free cells from seed, then builds navigable region.

`ga3d` target:

- Implement CPU baseline first, then GPU acceleration can be added behind same interface.
- Use this pipeline for AR occlusion mesh and as source for navmesh.
- Keep voxel params in config:

```yaml
voxel:
  size: 0.05
  opacity_cutoff: 0.1
  exterior_fill_radius: 1.6
  floor_fill: true
  floor_fill_dilation: 1.6
  seed: [0, 0, 0]
  carve:
    height: 1.6
    radius: 0.2
```

## Collision Mesh Extraction

References:

- `src/lib/writers/collision-glb.ts`
- `src/lib/mesh/marching-cubes.ts`
- `src/lib/mesh/voxel-faces.ts`
- `src/lib/mesh/coplanar-merge.ts`

Reference modes:

- `smooth`: binary marching cubes, optional flat-face pre-merge, then lossless coplanar merge.
- `faces`: direct watertight voxel-boundary faces with greedy rectangle merge and T-junction splitting.

Product mapping:

- `occlusion.glb` should use `smooth` by default for visual/physics quality.
- `faces` should remain available for deterministic watertight collision debugging.
- GLB contains positions and indices only. Product can add material/extras metadata for AR role.

Parity tests:

- Solid cube voxel grid produces closed mesh.
- Empty grid produces no mesh and no crash.
- `faces` mode produces axis-aligned watertight mesh.
- `smooth` mode produces triangles and coplanar merge reduces triangle count on flat regions.

## GLB Export

Reference: `src/lib/writers/write-glb.ts`.

Reference GLB is splat GLB with `KHR_gaussian_splatting`:

- `POSITION` accessor
- `COLOR_0` fallback as normalized RGBA
- `KHR_gaussian_splatting:ROTATION`
- `KHR_gaussian_splatting:SCALE`
- `KHR_gaussian_splatting:OPACITY`
- `KHR_gaussian_splatting:SH_DEGREE_*`

Product exports:

- `scene.glb`: splat preview GLB or reconstructed scene mesh depending on config.
- `occlusion.glb`: standard triangle mesh for AR occlusion.
- `navmesh.glb`: standard triangle mesh for navigation.
- `manifest.json`: scale, unit, coordinate system, source count, bounds, artifact roles, metrics.

## Parity Checklist For Task 2+

- Keep SoA column model.
- Preserve log scale and logit opacity internally.
- Decode `.splat` byte layout exactly.
- Decode SOG V1 and V2 into same canonical columns.
- Use Gaussian extents for bounds and voxelization.
- Bake edit recipe before filtering and voxelization.
- Use voxel pipeline for occlusion, not point-cloud Poisson as first option.
- Generate navmesh after fill/carve, not directly from raw splat centers.
- Add vendor fixture comparisons where possible, but do not import vendor source into product runtime.
