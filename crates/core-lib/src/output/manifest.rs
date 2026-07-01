use crate::math::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: u32,
    pub source: SourceStats,
    pub alignment: AlignmentManifest,
    pub bounds: Option<Bounds>,
    pub artifacts: ArtifactManifest,
    pub metrics: Metrics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStats {
    pub format: String,
    pub splat_count: usize,
    pub kept_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_splat_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactManifest {
    pub manifest: String,
    pub index_html: String,
    pub scene: String,
    pub collision_mesh_json: String,
    pub occlusion_glb: String,
    pub navmesh_glb: Option<String>,
    pub navmesh_bin: Option<String>,
    pub webar_zip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentManifest {
    pub unit_scale: f32,
    pub origin: [f32; 3],
    pub rotation_quat_wxyz: [f32; 4],
}

impl Default for AlignmentManifest {
    fn default() -> Self {
        Self {
            unit_scale: 1.0,
            origin: [0.0, 0.0, 0.0],
            rotation_quat_wxyz: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub decode_ms: u128,
    pub alignment_ms: u128,
    pub voxel_ms: u128,
    pub cpu_voxel_ms: u128,
    pub gpu_voxel_ms: Option<u128>,
    pub gpu_voxel_speedup: Option<f64>,
    pub voxel_backend: String,
    pub cpu_gpu_voxel_mismatches: Option<usize>,
    pub filter_cluster_input_count: usize,
    pub filter_cluster_output_count: usize,
    pub filter_cluster_removed_count: usize,
    pub filter_cluster_requested_seed: Option<[f32; 3]>,
    pub filter_cluster_resolved_seed: Option<[f32; 3]>,
    pub filter_cluster_seed_resolved: bool,
    pub filter_cluster_occupied_cells: usize,
    pub filter_cluster_cells: usize,
    pub floater_filter_input_count: usize,
    pub floater_filter_output_count: usize,
    pub floater_filter_removed_count: usize,
    pub fill_ms: u128,
    pub carve_ms: u128,
    pub mesh_ms: u128,
    pub navmesh_ms: u128,
    pub export_ms: u128,
    pub voxel_solid_cells: usize,
    pub filled_solid_cells: usize,
    pub carved_solid_cells: usize,
    pub cropped_solid_cells: usize,
    pub voxel_grid_dims: [usize; 3],
    pub filled_grid_dims: [usize; 3],
    pub carved_grid_dims: [usize; 3],
    pub cropped_grid_dims: [usize; 3],
    pub crop_min_cell: [usize; 3],
    pub crop_max_cell: [usize; 3],
    pub carve_reachable_cells: usize,
    pub carve_requested_seed: [f32; 3],
    pub carve_resolved_seed: Option<[f32; 3]>,
    pub collision_warnings: Vec<String>,
    pub source_bytes: u64,
    pub scene_sog_bytes: u64,
    pub optimized_glb_bytes: u64,
    pub source_to_optimized_glb_ratio: Option<f64>,
    pub optimized_glb_to_source_ratio: Option<f64>,
    pub source_to_webar_zip_ratio: Option<f64>,
    pub webar_zip_to_source_ratio: Option<f64>,
    pub collision_triangles_before_merge: usize,
    pub collision_triangles_after_merge: usize,
    pub navmesh_triangles: usize,
    pub webar_zip_bytes: u64,
    pub geometric_error_sample_count: usize,
    pub geometric_error_mean: f32,
    pub geometric_error_rms: f32,
    pub geometric_error_p95: f32,
}
