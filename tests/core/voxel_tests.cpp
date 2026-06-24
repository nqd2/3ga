#include "ga3d/voxel.hpp"

#include <cmath>
#include <cstdlib>
#include <iostream>
#include <limits>
#include <string>

namespace {

void require(bool condition, const std::string& message) {
    if (!condition) {
        std::cerr << message << '\n';
        std::exit(1);
    }
}

void require_close(float actual, float expected, float tolerance, const std::string& message) {
    if (std::fabs(actual - expected) > tolerance) {
        std::cerr << message << ": expected " << expected << ", got " << actual << '\n';
        std::exit(1);
    }
}

void add_splat(ga3d::DataTable& table, float x, float y, float z, float sx, float sy, float sz, float opacity = 5.0f) {
    table.x.push_back(x);
    table.y.push_back(y);
    table.z.push_back(z);
    table.scale0.push_back(std::log(sx));
    table.scale1.push_back(std::log(sy));
    table.scale2.push_back(std::log(sz));
    table.rot0.push_back(1.0f);
    table.rot1.push_back(0.0f);
    table.rot2.push_back(0.0f);
    table.rot3.push_back(0.0f);
    table.fdc0.push_back(0.0f);
    table.fdc1.push_back(0.0f);
    table.fdc2.push_back(0.0f);
    table.opacity.push_back(opacity);
}

ga3d::DataTable make_floor_with_box_splats() {
    ga3d::DataTable table;
    for (float x : {-0.3f, 0.0f, 0.3f}) {
        for (float z : {-0.3f, 0.0f, 0.3f}) {
            add_splat(table, x, 0.0f, z, 0.08f, 0.025f, 0.08f);
        }
    }
    add_splat(table, 0.0f, 0.25f, 0.0f, 0.12f, 0.12f, 0.12f);
    return table;
}

void gaussian_extent_uses_three_sigma_axes() {
    ga3d::DataTable table;
    add_splat(table, 1.0f, 2.0f, 3.0f, 0.5f, 0.25f, 0.125f);
    const auto extent = ga3d::gaussian_extent(table, 0, 3.0f);
    require_close(extent.bounds.min.x, -0.5f, 1e-5f, "x min extent mismatch");
    require_close(extent.bounds.max.y, 2.75f, 1e-5f, "y max extent mismatch");
    require_close(extent.bounds.max.z, 3.375f, 1e-5f, "z max extent mismatch");
}

void filters_remove_invalid_and_low_alpha_rows() {
    ga3d::DataTable table;
    add_splat(table, 0.0f, 0.0f, 0.0f, 0.1f, 0.1f, 0.1f, 5.0f);
    add_splat(table, std::numeric_limits<float>::quiet_NaN(), 0.0f, 0.0f, 0.1f, 0.1f, 0.1f, 5.0f);
    add_splat(table, 3.0f, 0.0f, 0.0f, 0.1f, 0.1f, 0.1f, -8.0f);

    const auto finite = ga3d::filter_nan(table);
    require(finite.size() == 2, "filter_nan should remove NaN row");
    const auto opaque = ga3d::filter_opacity_min(finite, 0.1f);
    require(opaque.size() == 1, "filter_opacity_min should remove low alpha row");
    const auto boxed = ga3d::filter_box(opaque, {{-0.1f, -0.1f, -0.1f}, {0.1f, 0.1f, 0.1f}});
    require(boxed.size() == 1, "filter_box should keep origin row");
    const auto sphere = ga3d::filter_sphere(opaque, {2.0f, 0.0f, 0.0f}, 0.5f);
    require(sphere.size() == 0, "filter_sphere should reject distant row");
}

void floor_and_cube_produce_collision_triangles() {
    const auto table = make_floor_with_box_splats();
    const auto grid = ga3d::voxelize(table, {.voxel_size = 0.1f, .opacity_cutoff = 0.1f});
    require(grid.occupied_count() > 0, "voxelize should produce occupied cells");

    const auto mesh = ga3d::extract_occlusion_mesh(grid, ga3d::MeshMode::Faces);
    require(!mesh.indices.empty(), "occlusion mesh should contain triangles");
    require(mesh.bounds.contains({0.0f, 0.0f, 0.0f}), "occlusion mesh bounds should contain origin");

    const auto smooth = ga3d::extract_occlusion_mesh(grid, ga3d::MeshMode::Smooth);
    require(!smooth.indices.empty(), "smooth occlusion mesh should contain triangles");
}

void block_fill_and_carve_update_grid() {
    ga3d::VoxelGrid grid;
    grid.voxel_size = 0.1f;
    grid.nx = 4;
    grid.ny = 4;
    grid.nz = 4;
    grid.solid.assign(64, 0);
    grid.set_solid(1, 1, 1);
    grid.set_solid(2, 1, 1);
    grid.set_solid(3, 3, 3);

    const auto filtered = ga3d::filter_floaters_by_voxel_contribution(grid, 2);
    require(filtered.occupied_count() == 2, "floater filter should remove only isolated voxel");

    const auto cluster = ga3d::filter_cluster_from_seed(grid, grid.cell_center(1, 1, 1));
    require(cluster.occupied_count() == 2, "cluster filter should keep seed-connected solid cells");

    const auto filled = ga3d::fill_mixed_blocks(grid, 1);
    require(filled.occupied_count() == 64, "fill_mixed_blocks should fill touched block");

    const auto carved = ga3d::capsule_carve(filled, {0.0f, 0.0f, 0.0f}, {0.4f, 0.4f, 0.4f}, 0.12f);
    require(carved.occupied_count() < filled.occupied_count(), "capsule_carve should remove cells near path");

    ga3d::VoxelGrid isolated = grid;
    isolated.solid.assign(64, 0);
    isolated.set_solid(1, 1, 1);
    const auto cropped = ga3d::crop_to_occupied(isolated);
    require(cropped.nx == 1 && cropped.ny == 1 && cropped.nz == 1, "crop_to_occupied should shrink isolated cell");
}

} // namespace

int main() {
    gaussian_extent_uses_three_sigma_axes();
    filters_remove_invalid_and_low_alpha_rows();
    floor_and_cube_produce_collision_triangles();
    block_fill_and_carve_update_grid();
    return 0;
}
