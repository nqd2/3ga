#pragma once

#include "ga3d/voxel.hpp"

#include <cstdint>
#include <filesystem>
#include <string>
#include <vector>

namespace ga3d {

struct NavConfig {
    bool enabled = true;
    Vec3 seed{0.0f, 0.0f, 0.0f};
    float agent_height = 1.6f;
    float agent_radius = 0.2f;
    float max_slope_degrees = 45.0f;
    float cell_size = 0.1f;
    float cell_height = 0.05f;
};

struct NavMesh {
    NavConfig config{};
    Bounds bounds{};
    Vec3 origin{};
    float cell_size = 0.1f;
    int width = 0;
    int depth = 0;
    std::vector<std::uint8_t> walkable;
    std::vector<float> heights;
    std::vector<float> vertices;
    std::vector<std::uint32_t> indices;

    bool in_bounds(int x, int z) const;
    std::size_t index(int x, int z) const;
    bool is_walkable(int x, int z) const;
    bool contains_walkable(Vec3 point) const;
};

NavMesh build_navmesh(const VoxelGrid& grid, const NavConfig& config = {});
Mesh navmesh_to_mesh(const NavMesh& navmesh);
std::string navmesh_to_json(const NavMesh& navmesh);
void write_navmesh_json(const NavMesh& navmesh, const std::filesystem::path& path);
void write_navmesh_glb(const NavMesh& navmesh, const std::filesystem::path& path);

} // namespace ga3d
