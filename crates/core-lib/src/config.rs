use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessConfig {
    #[serde(default)]
    pub voxel: VoxelConfig,
    #[serde(default)]
    pub voxel_fill: VoxelFillConfig,
    #[serde(default)]
    pub voxel_carve: VoxelCarveConfig,
    #[serde(default)]
    pub mesh: MeshConfig,
    #[serde(default)]
    pub navmesh: NavmeshConfig,
    #[serde(default)]
    pub export: ExportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelConfig {
    #[serde(default = "default_voxel_backend")]
    pub backend: VoxelBackend,
    #[serde(default)]
    pub compare_cpu_gpu: bool,
    pub size: f32,
    pub opacity_threshold: f32,
}

impl Default for VoxelConfig {
    fn default() -> Self {
        Self {
            backend: VoxelBackend::Cpu,
            compare_cpu_gpu: false,
            size: 0.1,
            opacity_threshold: 0.1,
        }
    }
}

fn default_voxel_backend() -> VoxelBackend {
    VoxelBackend::Cpu
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VoxelBackend {
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelFillConfig {
    pub mode: FillMode,
    pub dilation_size: f32,
}

impl Default for VoxelFillConfig {
    fn default() -> Self {
        Self {
            mode: FillMode::None,
            dilation_size: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FillMode {
    None,
    ExteriorFill,
    FloorFill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelCarveConfig {
    pub enabled: bool,
    pub agent_height: f32,
    pub agent_radius: f32,
    pub seed_pos: [f32; 3],
}

impl Default for VoxelCarveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            agent_height: 1.6,
            agent_radius: 0.2,
            seed_pos: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshConfig {
    pub mode: MeshMode,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            mode: MeshMode::Smooth,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MeshMode {
    Faces,
    Smooth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavmeshConfig {
    pub enabled: bool,
    pub agent_height: f32,
    pub agent_radius: f32,
    pub max_slope_degrees: f32,
    pub cell_size: f32,
    pub cell_height: f32,
    pub walkable_climb: f32,
    pub min_region_size: u16,
    pub merge_region_size: u16,
}

impl Default for NavmeshConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            agent_height: 1.6,
            agent_radius: 0.2,
            max_slope_degrees: 45.0,
            cell_size: 0.1,
            cell_height: 0.05,
            walkable_climb: 0.25,
            min_region_size: 4,
            merge_region_size: 12,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportConfig {
    pub write_webar_zip: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            write_webar_zip: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeBundle {
    #[serde(default)]
    pub alignment_recipe: Option<AlignmentRecipe>,
    #[serde(default)]
    pub edit_recipe: Option<EditRecipe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentRecipe {
    pub floor_points: Option<[[f32; 3]; 3]>,
    #[serde(default)]
    pub floor_fit_points: Option<Vec<[f32; 3]>>,
    #[serde(default)]
    pub up_axis: Option<UpAxis>,
    pub scale_points: Option<[[f32; 3]; 2]>,
    pub scale_distance_meters: Option<f32>,
    pub origin: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UpAxis {
    X,
    Y,
    Z,
    NegX,
    NegY,
    NegZ,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditRecipe {
    #[serde(default)]
    pub operations: Vec<EditOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EditOperation {
    SelectAll,
    SelectNone,
    FilterOpacity {
        min: f32,
    },
    FilterBox {
        min: [f32; 3],
        max: [f32; 3],
    },
    FilterSphere {
        center: [f32; 3],
        radius: f32,
    },
    FilterCluster {
        coarse_voxel_size: f32,
        opacity_threshold: f32,
        seed_pos: [f32; 3],
    },
    FilterFloatersByVoxelContribution,
}
