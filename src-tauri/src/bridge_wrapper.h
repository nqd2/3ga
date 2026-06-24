#pragma once

#include <memory>
#include <string>
#include "rust/cxx.h"

namespace ga3d_ffi {

struct RustEditOp;
struct RustVoxelOptions;
struct RustNavConfig;

std::unique_ptr<std::string> run_pipeline_rust(
    const std::string& input_path,
    const std::string& output_dir,
    rust::Slice<const ga3d_ffi::RustEditOp> recipe_ops,
    const RustVoxelOptions& voxel_opts,
    const RustNavConfig& nav_cfg,
    const std::string& mesh_mode
);

} // namespace ga3d_ffi
