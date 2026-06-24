#include "src-tauri/src/bridge_wrapper.h"
#include "core/include/ga3d/rust_bindings.hpp"
#include "app/src/bridge.rs.h"

#include <vector>

namespace ga3d_ffi {

std::unique_ptr<std::string> run_pipeline_rust(
    const std::string& input_path,
    const std::string& output_dir,
    rust::Slice<const ga3d_ffi::RustEditOp> recipe_ops,
    const RustVoxelOptions& voxel_opts,
    const RustNavConfig& nav_cfg,
    const std::string& mesh_mode
) {
    // 1. Convert cxx Slice<const ga3d_ffi::RustEditOp> to std::vector<ga3d::RustEditOp> (core type)
    std::vector<ga3d::RustEditOp> core_ops;
    core_ops.reserve(recipe_ops.size());
    for (const auto& op : recipe_ops) {
        ga3d::RustEditOp core_op;
        core_op.op_type = std::string(op.op_type);
        core_op.box_mode = op.box_mode;
        core_op.center = op.center;
        core_op.size = op.size;
        core_op.matrix = op.matrix;
        core_op.opacity_min = op.opacity_min;
        core_ops.push_back(core_op);
    }

    // 2. Convert ga3d_ffi::RustVoxelOptions to core ga3d::RustVoxelOptions
    ga3d::RustVoxelOptions core_voxel_opts;
    core_voxel_opts.voxel_size = voxel_opts.voxel_size;
    core_voxel_opts.opacity_cutoff = voxel_opts.opacity_cutoff;
    core_voxel_opts.sigma = voxel_opts.sigma;
    core_voxel_opts.align_to_blocks = voxel_opts.align_to_blocks;

    // 3. Convert ga3d_ffi::RustNavConfig to core ga3d::RustNavConfig
    ga3d::RustNavConfig core_nav_cfg;
    core_nav_cfg.enabled = nav_cfg.enabled;
    core_nav_cfg.seed = nav_cfg.seed;
    core_nav_cfg.agent_height = nav_cfg.agent_height;
    core_nav_cfg.agent_radius = nav_cfg.agent_radius;
    core_nav_cfg.max_slope_degrees = nav_cfg.max_slope_degrees;
    core_nav_cfg.cell_size = nav_cfg.cell_size;
    core_nav_cfg.cell_height = nav_cfg.cell_height;

    // 4. Call the core implementation in namespace ga3d
    std::string result = ga3d::run_pipeline_rust(
        input_path,
        output_dir,
        core_ops,
        core_voxel_opts,
        core_nav_cfg,
        mesh_mode
    );

    // 5. Return std::unique_ptr<std::string> mapped to UniquePtr<CxxString>
    return std::make_unique<std::string>(result);
}

} // namespace ga3d_ffi
