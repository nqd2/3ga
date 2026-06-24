#include "ga3d/navmesh.hpp"

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <deque>
#include <fstream>
#include <iomanip>
#include <limits>
#include <sstream>
#include <stdexcept>
#include <vector>

namespace ga3d {

namespace {

Bounds invalid_bounds() {
    const float inf = std::numeric_limits<float>::infinity();
    return {{inf, inf, inf}, {-inf, -inf, -inf}};
}

float sqr(float value) {
    return value * value;
}

int floor_index(float value, float origin, float cell_size) {
    return static_cast<int>(std::floor((value - origin) / cell_size));
}

bool has_clearance(const VoxelGrid& grid, int x, int support_y, int z, int clearance_cells) {
    for (int y = support_y + 1; y <= support_y + clearance_cells; ++y) {
        if (grid.is_solid(x, y, z)) {
            return false;
        }
    }
    return true;
}

std::array<int, 2> nearest_candidate(const std::vector<std::uint8_t>& candidate, int width, int depth, int seed_x, int seed_z) {
    std::array<int, 2> best{-1, -1};
    int best_d2 = std::numeric_limits<int>::max();
    for (int z = 0; z < depth; ++z) {
        for (int x = 0; x < width; ++x) {
            if (candidate[static_cast<std::size_t>(z * width + x)] == 0) {
                continue;
            }
            const int d2 = (x - seed_x) * (x - seed_x) + (z - seed_z) * (z - seed_z);
            if (d2 < best_d2) {
                best_d2 = d2;
                best = {x, z};
            }
        }
    }
    return best;
}

void append_cell_quad(NavMesh& navmesh, int x, int z, float height) {
    const float x0 = navmesh.origin.x + static_cast<float>(x) * navmesh.cell_size;
    const float x1 = x0 + navmesh.cell_size;
    const float z0 = navmesh.origin.z + static_cast<float>(z) * navmesh.cell_size;
    const float z1 = z0 + navmesh.cell_size;
    const auto base = static_cast<std::uint32_t>(navmesh.vertices.size() / 3);
    const Vec3 verts[4] = {
        {x0, height, z0},
        {x1, height, z0},
        {x1, height, z1},
        {x0, height, z1}
    };
    for (const auto& vertex : verts) {
        navmesh.vertices.push_back(vertex.x);
        navmesh.vertices.push_back(vertex.y);
        navmesh.vertices.push_back(vertex.z);
        navmesh.bounds.expand(vertex);
    }
    navmesh.indices.insert(navmesh.indices.end(), {base, base + 1, base + 2, base, base + 2, base + 3});
}

void append_float_array(std::ostringstream& out, const std::vector<float>& values) {
    out << '[';
    for (std::size_t i = 0; i < values.size(); ++i) {
        if (i != 0) {
            out << ',';
        }
        out << values[i];
    }
    out << ']';
}

void append_index_array(std::ostringstream& out, const std::vector<std::uint32_t>& values) {
    out << '[';
    for (std::size_t i = 0; i < values.size(); ++i) {
        if (i != 0) {
            out << ',';
        }
        out << values[i];
    }
    out << ']';
}

} // namespace

bool NavMesh::in_bounds(int x, int z) const {
    return x >= 0 && z >= 0 && x < width && z < depth;
}

std::size_t NavMesh::index(int x, int z) const {
    return static_cast<std::size_t>(z * width + x);
}

bool NavMesh::is_walkable(int x, int z) const {
    return in_bounds(x, z) && walkable[index(x, z)] != 0;
}

bool NavMesh::contains_walkable(Vec3 point) const {
    const int x = floor_index(point.x, origin.x, cell_size);
    const int z = floor_index(point.z, origin.z, cell_size);
    return is_walkable(x, z);
}

NavMesh build_navmesh(const VoxelGrid& grid, const NavConfig& config) {
    NavMesh navmesh;
    navmesh.config = config;
    navmesh.bounds = invalid_bounds();
    navmesh.origin = grid.origin;
    navmesh.cell_size = grid.voxel_size;
    navmesh.width = grid.nx;
    navmesh.depth = grid.nz;
    navmesh.walkable.assign(static_cast<std::size_t>(grid.nx * grid.nz), 0);
    navmesh.heights.assign(static_cast<std::size_t>(grid.nx * grid.nz), 0.0f);

    if (!config.enabled || grid.nx <= 0 || grid.ny <= 0 || grid.nz <= 0 || grid.solid.empty()) {
        return navmesh;
    }
    if (config.agent_height <= 0.0f || config.agent_radius < 0.0f || config.max_slope_degrees < 0.0f) {
        throw std::runtime_error("invalid nav agent config");
    }

    const int clearance_cells = std::max(1, static_cast<int>(std::ceil(config.agent_height / grid.voxel_size)));
    std::vector<std::uint8_t> candidate(navmesh.walkable.size(), 0);
    for (int z = 0; z < grid.nz; ++z) {
        for (int x = 0; x < grid.nx; ++x) {
            for (int y = grid.ny - 1; y >= 0; --y) {
                if (!grid.is_solid(x, y, z)) {
                    continue;
                }
                if (has_clearance(grid, x, y, z, clearance_cells)) {
                    const auto idx = navmesh.index(x, z);
                    candidate[idx] = 1;
                    navmesh.heights[idx] = grid.origin.y + static_cast<float>(y + 1) * grid.voxel_size;
                }
                break;
            }
        }
    }

    const int radius_cells = static_cast<int>(std::ceil(config.agent_radius / grid.voxel_size));
    std::vector<std::uint8_t> eroded = candidate;
    if (radius_cells > 0) {
        for (int z = 0; z < grid.nz; ++z) {
            for (int x = 0; x < grid.nx; ++x) {
                const auto idx = navmesh.index(x, z);
                if (candidate[idx] == 0) {
                    continue;
                }
                for (int dz = -radius_cells; dz <= radius_cells; ++dz) {
                    for (int dx = -radius_cells; dx <= radius_cells; ++dx) {
                        if (sqr(static_cast<float>(dx)) + sqr(static_cast<float>(dz)) > sqr(static_cast<float>(radius_cells))) {
                            continue;
                        }
                        const int nx = x + dx;
                        const int nz = z + dz;
                        if (nx < 0 || nz < 0 || nx >= grid.nx || nz >= grid.nz ||
                            candidate[static_cast<std::size_t>(nz * grid.nx + nx)] == 0) {
                            eroded[idx] = 0;
                        }
                    }
                }
            }
        }
    }

    int seed_x = floor_index(config.seed.x, grid.origin.x, grid.voxel_size);
    int seed_z = floor_index(config.seed.z, grid.origin.z, grid.voxel_size);
    if (seed_x < 0 || seed_z < 0 || seed_x >= grid.nx || seed_z >= grid.nz ||
        eroded[static_cast<std::size_t>(seed_z * grid.nx + seed_x)] == 0) {
        const auto nearest = nearest_candidate(eroded, grid.nx, grid.nz, seed_x, seed_z);
        seed_x = nearest[0];
        seed_z = nearest[1];
    }
    if (seed_x < 0 || seed_z < 0) {
        return navmesh;
    }

    constexpr float pi = 3.14159265358979323846f;
    const float max_step = std::tan(config.max_slope_degrees * pi / 180.0f) * grid.voxel_size + config.cell_height;
    std::deque<std::array<int, 2>> queue;
    queue.push_back({seed_x, seed_z});
    navmesh.walkable[navmesh.index(seed_x, seed_z)] = 1;
    constexpr std::array<std::array<int, 2>, 4> dirs{{
        {{1, 0}}, {{-1, 0}}, {{0, 1}}, {{0, -1}}
    }};

    while (!queue.empty()) {
        const auto cell = queue.front();
        queue.pop_front();
        const auto base_idx = navmesh.index(cell[0], cell[1]);
        for (const auto& dir : dirs) {
            const int nx = cell[0] + dir[0];
            const int nz = cell[1] + dir[1];
            if (nx < 0 || nz < 0 || nx >= grid.nx || nz >= grid.nz) {
                continue;
            }
            const auto next_idx = navmesh.index(nx, nz);
            if (eroded[next_idx] == 0 || navmesh.walkable[next_idx] != 0) {
                continue;
            }
            if (std::abs(navmesh.heights[next_idx] - navmesh.heights[base_idx]) <= max_step) {
                navmesh.walkable[next_idx] = 1;
                queue.push_back({nx, nz});
            }
        }
    }

    for (int z = 0; z < navmesh.depth; ++z) {
        for (int x = 0; x < navmesh.width; ++x) {
            if (navmesh.is_walkable(x, z)) {
                append_cell_quad(navmesh, x, z, navmesh.heights[navmesh.index(x, z)]);
            }
        }
    }

    return navmesh;
}

Mesh navmesh_to_mesh(const NavMesh& navmesh) {
    Mesh mesh;
    mesh.bounds = navmesh.bounds;
    mesh.positions = navmesh.vertices;
    mesh.indices = navmesh.indices;
    return mesh;
}

std::string navmesh_to_json(const NavMesh& navmesh) {
    std::ostringstream out;
    out << std::setprecision(9);
    out << "{";
    out << "\"version\":1,";
    out << "\"coordinateSystem\":{\"upAxis\":\"Y\",\"unit\":\"meter\"},";
    out << "\"bounds\":{\"min\":[" << navmesh.bounds.min.x << ',' << navmesh.bounds.min.y << ',' << navmesh.bounds.min.z
        << "],\"max\":[" << navmesh.bounds.max.x << ',' << navmesh.bounds.max.y << ',' << navmesh.bounds.max.z << "]},";
    out << "\"config\":{"
        << "\"enabled\":" << (navmesh.config.enabled ? "true" : "false") << ','
        << "\"seed\":[" << navmesh.config.seed.x << ',' << navmesh.config.seed.y << ',' << navmesh.config.seed.z << "],"
        << "\"agentHeight\":" << navmesh.config.agent_height << ','
        << "\"agentRadius\":" << navmesh.config.agent_radius << ','
        << "\"maxSlopeDegrees\":" << navmesh.config.max_slope_degrees << ','
        << "\"cellSize\":" << navmesh.config.cell_size << ','
        << "\"cellHeight\":" << navmesh.config.cell_height
        << "},";
    out << "\"grid\":{\"width\":" << navmesh.width << ",\"depth\":" << navmesh.depth << ",\"cellSize\":" << navmesh.cell_size << "},";
    out << "\"vertices\":";
    append_float_array(out, navmesh.vertices);
    out << ",\"indices\":";
    append_index_array(out, navmesh.indices);
    out << "}";
    return out.str();
}

void write_navmesh_json(const NavMesh& navmesh, const std::filesystem::path& path) {
    std::ofstream out(path);
    if (!out) {
        throw std::runtime_error("failed to open navmesh json output");
    }
    out << navmesh_to_json(navmesh);
}

void write_navmesh_glb(const NavMesh& navmesh, const std::filesystem::path& path) {
    const auto position_bytes = static_cast<std::uint32_t>(navmesh.vertices.size() * sizeof(float));
    const auto index_offset = static_cast<std::uint32_t>((position_bytes + 3U) & ~3U);
    const auto index_bytes = static_cast<std::uint32_t>(navmesh.indices.size() * sizeof(std::uint32_t));
    const auto bin_length = static_cast<std::uint32_t>((index_offset + index_bytes + 3U) & ~3U);

    std::ostringstream json;
    json << std::setprecision(9);
    json
        << "{\"asset\":{\"version\":\"2.0\",\"generator\":\"ga3d\"},"
        << "\"scene\":0,\"scenes\":[{\"nodes\":[0]}],"
        << "\"nodes\":[{\"mesh\":0,\"name\":\"GA3D_NAVMESH\"}],"
        << "\"meshes\":[{\"name\":\"GA3D_NAVMESH\",\"primitives\":[{\"attributes\":{\"POSITION\":0},\"indices\":1,"
        << "\"extras\":{\"ga3dRole\":\"navmesh\"}}]}],"
        << "\"buffers\":[{\"byteLength\":" << bin_length << "}],"
        << "\"bufferViews\":["
        << "{\"buffer\":0,\"byteOffset\":0,\"byteLength\":" << position_bytes << ",\"target\":34962},"
        << "{\"buffer\":0,\"byteOffset\":" << index_offset << ",\"byteLength\":" << index_bytes << ",\"target\":34963}],"
        << "\"accessors\":["
        << "{\"bufferView\":0,\"componentType\":5126,\"count\":" << (navmesh.vertices.size() / 3)
        << ",\"type\":\"VEC3\",\"min\":[" << navmesh.bounds.min.x << ',' << navmesh.bounds.min.y << ',' << navmesh.bounds.min.z
        << "],\"max\":[" << navmesh.bounds.max.x << ',' << navmesh.bounds.max.y << ',' << navmesh.bounds.max.z << "]},"
        << "{\"bufferView\":1,\"componentType\":5125,\"count\":" << navmesh.indices.size() << ",\"type\":\"SCALAR\"}],"
        << "\"extras\":{\"coordinateSystem\":{\"upAxis\":\"Y\",\"unit\":\"meter\"}}}";

    std::string json_chunk = json.str();
    while (json_chunk.size() % 4 != 0) {
        json_chunk.push_back(' ');
    }

    std::vector<char> bin(bin_length, 0);
    if (!navmesh.vertices.empty()) {
        const auto* bytes = reinterpret_cast<const char*>(navmesh.vertices.data());
        std::copy(bytes, bytes + position_bytes, bin.begin());
    }
    if (!navmesh.indices.empty()) {
        const auto* bytes = reinterpret_cast<const char*>(navmesh.indices.data());
        std::copy(bytes, bytes + index_bytes, bin.begin() + index_offset);
    }

    std::ofstream out(path, std::ios::binary);
    if (!out) {
        throw std::runtime_error("failed to open navmesh glb output");
    }

    auto write_u32 = [&](std::uint32_t value) {
        out.write(reinterpret_cast<const char*>(&value), sizeof(value));
    };

    const std::uint32_t json_length = static_cast<std::uint32_t>(json_chunk.size());
    const std::uint32_t total_length = 12U + 8U + json_length + 8U + bin_length;
    write_u32(0x46546C67U);
    write_u32(2U);
    write_u32(total_length);
    write_u32(json_length);
    write_u32(0x4E4F534AU);
    out.write(json_chunk.data(), json_chunk.size());
    write_u32(bin_length);
    write_u32(0x004E4942U);
    out.write(bin.data(), bin.size());
}

} // namespace ga3d
