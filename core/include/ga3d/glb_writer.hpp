#pragma once

#include "ga3d/voxel.hpp"

#include <filesystem>
#include <string>

namespace ga3d {

struct GlbWriteOptions {
    std::string name = "GA3D_MESH";
    std::string role = "scene";
    bool visible = true;
};

void write_mesh_glb(const Mesh& mesh, const std::filesystem::path& path, const GlbWriteOptions& options = {});
void write_scene_glb(const Mesh& mesh, const std::filesystem::path& path);
void write_occlusion_glb(const Mesh& mesh, const std::filesystem::path& path);
void write_navmesh_glb(const Mesh& mesh, const std::filesystem::path& path);

} // namespace ga3d
