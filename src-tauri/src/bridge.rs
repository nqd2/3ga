#[cxx::bridge(namespace = "ga3d_ffi")]
pub mod ffi {
    struct RustEditOp {
        op_type: String,
        box_mode: i32,
        center: [f32; 3],
        size: [f32; 3],
        matrix: [f32; 16],
        opacity_min: f32,
    }

    struct RustVoxelOptions {
        voxel_size: f32,
        opacity_cutoff: f32,
        sigma: f32,
        align_to_blocks: bool,
    }

    struct RustNavConfig {
        enabled: bool,
        seed: [f32; 3],
        agent_height: f32,
        agent_radius: f32,
        max_slope_degrees: f32,
        cell_size: f32,
        cell_height: f32,
    }

    unsafe extern "C++" {
        include!("src-tauri/src/bridge_wrapper.h");

        fn run_pipeline_rust(
            input_path: &CxxString,
            output_dir: &CxxString,
            recipe_ops: &[RustEditOp],
            voxel_opts: &RustVoxelOptions,
            nav_cfg: &RustNavConfig,
            mesh_mode: &CxxString,
        ) -> UniquePtr<CxxString>;
    }
}
