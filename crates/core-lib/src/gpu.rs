use crate::error::{AgError, AgResult};
use crate::splat_table::SplatTable;
use crate::voxel::{VoxelGrid, VoxelParams, voxel_grid_for_table};
use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;
use std::sync::mpsc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuSplat {
    position: [f32; 4],
    sigma_alpha: [f32; 4],
    rotation: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct VoxelUniforms {
    grid_min: [f32; 4],
    dims: [u32; 4],
    voxel_size: f32,
    opacity_threshold: f32,
    splat_count: u32,
    voxel_count: u32,
}

#[derive(Clone)]
pub struct WgpuContext {
    pub device: std::sync::Arc<wgpu::Device>,
    pub queue: std::sync::Arc<wgpu::Queue>,
    pub pipeline: std::sync::Arc<wgpu::ComputePipeline>,
    pub bind_group_layout: std::sync::Arc<wgpu::BindGroupLayout>,
}

pub fn init_wgpu() -> Option<WgpuContext> {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("augmented-gaussian voxel device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                },
                None,
            )
            .await
            .ok()?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(VOXEL_SHADER)),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("voxel bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxel pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("voxel pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        });

        Some(WgpuContext {
            device: std::sync::Arc::new(device),
            queue: std::sync::Arc::new(queue),
            pipeline: std::sync::Arc::new(pipeline),
            bind_group_layout: std::sync::Arc::new(bind_group_layout),
        })
    })
}

pub fn voxelize_gpu_blocking(
    table: &SplatTable,
    params: VoxelParams,
    wgpu_ctx: Option<&WgpuContext>,
) -> AgResult<VoxelGrid> {
    pollster::block_on(voxelize_gpu(table, params, wgpu_ctx))
}

pub async fn voxelize_gpu(
    table: &SplatTable,
    params: VoxelParams,
    wgpu_ctx: Option<&WgpuContext>,
) -> AgResult<VoxelGrid> {
    let mut grid = voxel_grid_for_table(table, params)?;
    if table.is_empty() {
        return Ok(grid);
    }

    let (device, queue, pipeline, bind_group_layout) = match wgpu_ctx {
        Some(ctx) => (
            ctx.device.clone(),
            ctx.queue.clone(),
            ctx.pipeline.clone(),
            ctx.bind_group_layout.clone(),
        ),
        None => {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .ok_or_else(|| AgError::InvalidInput("no wgpu adapter available".to_string()))?;
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("augmented-gaussian voxel device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::downlevel_defaults(),
                    },
                    None,
                )
                .await
                .map_err(|err| AgError::InvalidInput(format!("failed to request wgpu device: {err}")))?;

            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("voxel shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(VOXEL_SHADER)),
            });
            let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("voxel bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("voxel pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("voxel pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            });
            (
                std::sync::Arc::new(device),
                std::sync::Arc::new(queue),
                std::sync::Arc::new(pipeline),
                std::sync::Arc::new(bind_group_layout),
            )
        }
    };

    let output_bytes = (grid.cell_count() * std::mem::size_of::<u32>()) as u64;
    let zero_output = vec![0u32; grid.cell_count()];
    let output_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("voxel output"),
        contents: bytemuck::cast_slice(&zero_output),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("voxel readback"),
        size: output_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let max_binding = wgpu::Limits::downlevel_defaults().max_storage_buffer_binding_size as usize;
    let max_splats_per_chunk = (max_binding / std::mem::size_of::<GpuSplat>()).clamp(1, 1_000_000);
    for start in (0..table.len()).step_by(max_splats_per_chunk) {
        let end = (start + max_splats_per_chunk).min(table.len());
        let splats = splats_for_gpu_range(table, start, end);
        let splat_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("voxel splats"),
            contents: bytemuck::cast_slice(&splats),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let uniforms = VoxelUniforms {
            grid_min: [grid.min.x, grid.min.y, grid.min.z, 0.0],
            dims: [
                grid.dims[0] as u32,
                grid.dims[1] as u32,
                grid.dims[2] as u32,
                0,
            ],
            voxel_size: grid.size,
            opacity_threshold: params.opacity_threshold,
            splat_count: splats.len() as u32,
            voxel_count: grid.cell_count() as u32,
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("voxel uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("voxel bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: splat_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("voxel encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("voxel pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = uniforms.splat_count.div_ceil(64);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }
        queue.submit(Some(encoder.finish()));
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("voxel readback encoder"),
    });
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback_buffer, 0, output_bytes);
    queue.submit(Some(encoder.finish()));

    let slice = readback_buffer.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .map_err(|err| AgError::InvalidInput(format!("wgpu readback failed: {err}")))?
        .map_err(|err| AgError::InvalidInput(format!("wgpu map failed: {err}")))?;
    {
        let data = slice.get_mapped_range();
        let values: &[u32] = bytemuck::cast_slice(&data);
        for (index, value) in values.iter().copied().enumerate() {
            grid.set_flat(index, value != 0);
        }
    }
    readback_buffer.unmap();
    Ok(grid)
}

fn splats_for_gpu_range(table: &SplatTable, start: usize, end: usize) -> Vec<GpuSplat> {
    (start..end)
        .map(|i| GpuSplat {
            position: [table.x[i], table.y[i], table.z[i], 0.0],
            sigma_alpha: [
                table.scale_0[i].exp(),
                table.scale_1[i].exp(),
                table.scale_2[i].exp(),
                table.linear_alpha(i),
            ],
            rotation: [
                table.rot_0[i],
                table.rot_1[i],
                table.rot_2[i],
                table.rot_3[i],
            ],
        })
        .collect()
}

const VOXEL_SHADER: &str = r#"
struct Splat {
    position: vec4<f32>,
    sigma_alpha: vec4<f32>,
    rotation: vec4<f32>,
};

struct Uniforms {
    grid_min: vec4<f32>,
    dims: vec4<u32>,
    voxel_size: f32,
    opacity_threshold: f32,
    splat_count: u32,
    voxel_count: u32,
};

@group(0) @binding(0)
var<storage, read> splats: array<Splat>;
@group(0) @binding(1)
var<uniform> uniforms: Uniforms;
@group(0) @binding(2)
var<storage, read_write> output: array<atomic<u32>>;

fn rotate_by_quat(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    let qv = vec3<f32>(q.y, q.z, q.w);
    let t = cross(qv, v) * 2.0;
    return v + t * q.x + cross(qv, t);
}

fn inverse_rotate_by_quat(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    return rotate_by_quat(v, vec4<f32>(q.x, -q.y, -q.z, -q.w));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let splat_index = global_id.x;
    if (splat_index >= uniforms.splat_count) {
        return;
    }

    let splat = splats[splat_index];
    if (splat.sigma_alpha.w < uniforms.opacity_threshold) {
        return;
    }
    let sigma = max(splat.sigma_alpha.xyz, vec3<f32>(0.000001));
    let q = normalize(splat.rotation);
    let axis_x = rotate_by_quat(vec3<f32>(sigma.x * 3.0, 0.0, 0.0), q);
    let axis_y = rotate_by_quat(vec3<f32>(0.0, sigma.y * 3.0, 0.0), q);
    let axis_z = rotate_by_quat(vec3<f32>(0.0, 0.0, sigma.z * 3.0), q);
    let half_world = abs(axis_x) + abs(axis_y) + abs(axis_z);
    let min_local = (splat.position.xyz - half_world - uniforms.grid_min.xyz) / uniforms.voxel_size;
    let max_local = (splat.position.xyz + half_world - uniforms.grid_min.xyz) / uniforms.voxel_size;
    let min_cell = vec3<i32>(
        max(0, i32(floor(min_local.x))),
        max(0, i32(floor(min_local.y))),
        max(0, i32(floor(min_local.z)))
    );
    let max_cell = vec3<i32>(
        min(i32(uniforms.dims.x) - 1, i32(floor(max_local.x))),
        min(i32(uniforms.dims.y) - 1, i32(floor(max_local.y))),
        min(i32(uniforms.dims.z) - 1, i32(floor(max_local.z)))
    );
    if (min_cell.x > max_cell.x || min_cell.y > max_cell.y || min_cell.z > max_cell.z) {
        return;
    }

    var z = min_cell.z;
    loop {
        if (z > max_cell.z) { break; }
        var y = min_cell.y;
        loop {
            if (y > max_cell.y) { break; }
            var x = min_cell.x;
            loop {
                if (x > max_cell.x) { break; }
                let center = uniforms.grid_min.xyz + (vec3<f32>(f32(x), f32(y), f32(z)) + vec3<f32>(0.5)) * uniforms.voxel_size;
                let local = inverse_rotate_by_quat(center - splat.position.xyz, q);
                let d = local / sigma;
                let n2 = dot(d, d);
                let contribution = splat.sigma_alpha.w * exp(-0.5 * n2);
                if (contribution >= uniforms.opacity_threshold) {
                    let index = u32(x) + u32(y) * uniforms.dims.x + u32(z) * uniforms.dims.x * uniforms.dims.y;
                    atomicStore(&output[index], 1u);
                }
                x = x + 1;
            }
            y = y + 1;
        }
        z = z + 1;
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Quat, Vec3, QuatExt};

    #[test]
    fn gpu_matches_cpu_when_adapter_available() {
        let mut table = SplatTable::default();
        let s = 0.5f32.sqrt();
        table.push_standard(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.4f32.ln(), 0.1f32.ln(), 0.1f32.ln()),
            4.0,
            Vec3::ZERO,
            Quat::from_wxyz(s, 0.0, 0.0, s),
        );
        let params = VoxelParams {
            size: 0.5,
            opacity_threshold: 0.2,
        };
        let cpu = crate::voxel::voxelize_cpu(&table, params).unwrap();
        let Ok(gpu) = voxelize_gpu_blocking(&table, params, None) else {
            return;
        };
        assert_eq!(cpu.mismatch_count(&gpu), 0);
    }
}