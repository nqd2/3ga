#pragma once

#include <string>
#include <vector>
#include <array>

namespace ga3d {

struct RustEditOp {
    std::string op_type;
    int box_mode;
    std::array<float, 3> center;
    std::array<float, 3> size;
    std::array<float, 16> matrix;
    float opacity_min;
};

struct RustVoxelOptions {
    float voxel_size;
    float opacity_cutoff;
    float sigma;
    bool align_to_blocks;
};

struct RustNavConfig {
    bool enabled;
    std::array<float, 3> seed;
    float agent_height;
    float agent_radius;
    float max_slope_degrees;
    float cell_size;
    float cell_height;
};

std::string run_pipeline_rust(
    const std::string& input_path,
    const std::string& output_dir,
    const std::vector<RustEditOp>& recipe_ops,
    const RustVoxelOptions& voxel_opts,
    const RustNavConfig& nav_cfg,
    const std::string& mesh_mode
);

} // namespace ga3d
