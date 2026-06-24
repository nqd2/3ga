#pragma once

#include "ga3d/data_table.hpp"

#include <array>
#include <cstddef>
#include <cstdint>
#include <vector>

namespace ga3d {

struct Vec3 {
    float x = 0.0f;
    float y = 0.0f;
    float z = 0.0f;
};

struct Bounds {
    Vec3 min{};
    Vec3 max{};

    bool contains(const Vec3& point) const;
    bool valid() const;
    void expand(const Vec3& point);
    void expand(const Bounds& bounds);
};

struct GaussianExtent {
    Bounds bounds{};
    std::array<float, 3> axes{};
};

struct VoxelOptions {
    float voxel_size = 0.1f;
    float opacity_cutoff = 0.1f;
    float sigma = 3.0f;
    bool align_to_blocks = true;
};

struct VoxelGrid {
    Vec3 origin{};
    float voxel_size = 0.1f;
    int nx = 0;
    int ny = 0;
    int nz = 0;
    std::vector<std::uint8_t> solid;

    std::size_t index(int x, int y, int z) const;
    bool in_bounds(int x, int y, int z) const;
    bool is_solid(int x, int y, int z) const;
    void set_solid(int x, int y, int z, bool value = true);
    Vec3 cell_center(int x, int y, int z) const;
    Bounds bounds() const;
    std::size_t occupied_count() const;
};

enum class MeshMode {
    Smooth,
    Faces
};

struct Mesh {
    std::vector<float> positions;
    std::vector<std::uint32_t> indices;
    Bounds bounds{};
};

GaussianExtent gaussian_extent(const DataTable& table, std::size_t row, float sigma = 3.0f);

DataTable filter_nan(const DataTable& table);
DataTable filter_opacity_min(const DataTable& table, float alpha_min);
DataTable filter_box(const DataTable& table, const Bounds& bounds);
DataTable filter_sphere(const DataTable& table, Vec3 center, float radius);

VoxelGrid voxelize(const DataTable& table, const VoxelOptions& options = {});
VoxelGrid filter_floaters_by_voxel_contribution(const VoxelGrid& grid, int min_neighbors = 2);
VoxelGrid filter_cluster_from_seed(const VoxelGrid& grid, Vec3 seed);
VoxelGrid filter_sparse_blocks(const VoxelGrid& grid, int min_occupied_per_block = 1);
VoxelGrid fill_mixed_blocks(const VoxelGrid& grid, int min_occupied_per_block = 1);
VoxelGrid floor_fill(const VoxelGrid& grid, int floor_layers = 1);
VoxelGrid exterior_fill(const VoxelGrid& grid);
VoxelGrid capsule_carve(const VoxelGrid& grid, Vec3 start, Vec3 end, float radius);
VoxelGrid crop_to_occupied(const VoxelGrid& grid);

Mesh extract_occlusion_mesh(const VoxelGrid& grid, MeshMode mode = MeshMode::Faces);

} // namespace ga3d
