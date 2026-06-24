#include "ga3d/navmesh.hpp"

#include <cstdint>
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <string>

namespace {

void require(bool condition, const std::string& message) {
    if (!condition) {
        std::cerr << message << '\n';
        std::exit(1);
    }
}

ga3d::VoxelGrid make_floor_grid() {
    ga3d::VoxelGrid grid;
    grid.origin = {-0.3f, 0.0f, -0.3f};
    grid.voxel_size = 0.1f;
    grid.nx = 6;
    grid.ny = 5;
    grid.nz = 6;
    grid.solid.assign(static_cast<std::size_t>(grid.nx * grid.ny * grid.nz), 0);
    for (int z = 0; z < grid.nz; ++z) {
        for (int x = 0; x < grid.nx; ++x) {
            grid.set_solid(x, 0, z);
        }
    }
    return grid;
}

ga3d::NavConfig test_config() {
    ga3d::NavConfig config;
    config.seed = {-0.2f, 0.1f, -0.2f};
    config.agent_height = 0.2f;
    config.agent_radius = 0.0f;
    config.max_slope_degrees = 45.0f;
    config.cell_size = 0.1f;
    config.cell_height = 0.025f;
    return config;
}

void flat_floor_has_connected_nav_polygon() {
    const auto grid = make_floor_grid();
    const auto navmesh = ga3d::build_navmesh(grid, test_config());

    require(!navmesh.indices.empty(), "flat floor navmesh should contain triangles");
    require(navmesh.contains_walkable({0.0f, 0.1f, 0.0f}), "flat floor should be walkable at origin");
    require(navmesh.walkable.size() == 36, "nav walkable grid size mismatch");
    for (const auto value : navmesh.walkable) {
        require(value != 0, "flat floor should be one connected walkable region");
    }

    const auto mesh = ga3d::navmesh_to_mesh(navmesh);
    require(mesh.indices.size() == navmesh.indices.size(), "navmesh_to_mesh should preserve indices");
}

void cube_obstacle_creates_non_walkable_hole() {
    auto grid = make_floor_grid();
    grid.set_solid(3, 1, 3);
    grid.set_solid(3, 2, 3);

    const auto navmesh = ga3d::build_navmesh(grid, test_config());
    require(!navmesh.contains_walkable({0.05f, 0.1f, 0.05f}), "obstacle column should create non-walkable hole");
    require(navmesh.contains_walkable({-0.2f, 0.1f, -0.2f}), "seed-side floor should remain walkable");

    const auto json = ga3d::navmesh_to_json(navmesh);
    require(json.find("\"coordinateSystem\"") != std::string::npos, "navmesh JSON missing coordinate system");
    require(json.find("\"agentHeight\"") != std::string::npos, "navmesh JSON missing config");

    ga3d::write_navmesh_json(navmesh, "/tmp/ga3d_navmesh_test.json");
    ga3d::write_navmesh_glb(navmesh, "/tmp/ga3d_navmesh_test.glb");
    std::ifstream glb("/tmp/ga3d_navmesh_test.glb", std::ios::binary);
    std::uint32_t magic = 0;
    glb.read(reinterpret_cast<char*>(&magic), sizeof(magic));
    require(magic == 0x46546C67U, "navmesh GLB missing glTF magic");
}

void agent_clearance_erodes_edges() {
    const auto grid = make_floor_grid();
    auto config = test_config();
    config.agent_radius = 0.11f;
    const auto navmesh = ga3d::build_navmesh(grid, config);

    require(!navmesh.contains_walkable({-0.25f, 0.1f, -0.25f}), "agent radius should erode exposed floor edge");
    require(navmesh.contains_walkable({0.0f, 0.1f, 0.0f}), "center floor should remain after radius clearance");
}

void slope_limit_blocks_steep_step() {
    auto grid = make_floor_grid();
    grid.set_solid(2, 1, 2);
    grid.set_solid(2, 2, 2);

    auto config = test_config();
    config.seed = {-0.25f, 0.1f, -0.25f};
    config.max_slope_degrees = 10.0f;
    const auto navmesh = ga3d::build_navmesh(grid, config);

    require(!navmesh.contains_walkable({-0.05f, 0.2f, -0.05f}), "steep adjacent support should not be connected");
    require(navmesh.contains_walkable({-0.25f, 0.1f, -0.25f}), "seed floor cell should remain walkable");
}

} // namespace

int main() {
    flat_floor_has_connected_nav_polygon();
    cube_obstacle_creates_non_walkable_hole();
    agent_clearance_erodes_edges();
    slope_limit_blocks_steep_step();
    return 0;
}
