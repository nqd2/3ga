pub mod context;
pub mod voxelizer;

pub use context::{GpuSplat, VoxelUniforms, init_wgpu};
pub use voxelizer::{voxelize_gpu, voxelize_gpu_blocking};

#[derive(Clone)]
pub struct WgpuContext {
    pub device: std::sync::Arc<wgpu::Device>,
    pub queue: std::sync::Arc<wgpu::Queue>,
    pub pipeline: std::sync::Arc<wgpu::ComputePipeline>,
    pub bind_group_layout: std::sync::Arc<wgpu::BindGroupLayout>,
}

pub const VOXEL_SHADER: &str = include_str!("shader.wgsl");
