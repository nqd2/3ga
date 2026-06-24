#include "ga3d/voxel.hpp"

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <deque>
#include <limits>
#include <stdexcept>
#include <vector>

namespace ga3d {

namespace {

struct Quaternion {
    float w;
    float x;
    float y;
    float z;
};

float sigmoid(float value) {
    return 1.0f / (1.0f + std::exp(-value));
}

Bounds invalid_bounds() {
    const float inf = std::numeric_limits<float>::infinity();
    return {{inf, inf, inf}, {-inf, -inf, -inf}};
}

bool finite(float value) {
    return std::isfinite(value);
}

bool finite_row(const DataTable& table, std::size_t row) {
    bool ok = finite(table.x[row]) && finite(table.y[row]) && finite(table.z[row]) &&
              finite(table.scale0[row]) && finite(table.scale1[row]) && finite(table.scale2[row]) &&
              finite(table.rot0[row]) && finite(table.rot1[row]) && finite(table.rot2[row]) && finite(table.rot3[row]) &&
              finite(table.fdc0[row]) && finite(table.fdc1[row]) && finite(table.fdc2[row]) &&
              finite(table.opacity[row]);
    for (const auto& coeff : table.sh_rest) {
        ok = ok && finite(coeff[row]);
    }
    return ok;
}

void append_row(DataTable& out, const DataTable& input, std::size_t row) {
    out.x.push_back(input.x[row]);
    out.y.push_back(input.y[row]);
    out.z.push_back(input.z[row]);
    out.scale0.push_back(input.scale0[row]);
    out.scale1.push_back(input.scale1[row]);
    out.scale2.push_back(input.scale2[row]);
    out.rot0.push_back(input.rot0[row]);
    out.rot1.push_back(input.rot1[row]);
    out.rot2.push_back(input.rot2[row]);
    out.rot3.push_back(input.rot3[row]);
    out.fdc0.push_back(input.fdc0[row]);
    out.fdc1.push_back(input.fdc1[row]);
    out.fdc2.push_back(input.fdc2[row]);
    out.opacity.push_back(input.opacity[row]);
    if (out.sh_rest.empty() && !input.sh_rest.empty()) {
        out.sh_rest.resize(input.sh_rest.size());
    }
    for (std::size_t i = 0; i < input.sh_rest.size(); ++i) {
        out.sh_rest[i].push_back(input.sh_rest[i][row]);
    }
}

Quaternion normalize(Quaternion q) {
    const float length = std::sqrt(q.w * q.w + q.x * q.x + q.y * q.y + q.z * q.z);
    if (length == 0.0f || !std::isfinite(length)) {
        return {1.0f, 0.0f, 0.0f, 0.0f};
    }
    return {q.w / length, q.x / length, q.y / length, q.z / length};
}

std::array<float, 9> rotation_matrix(Quaternion raw) {
    const auto q = normalize(raw);
    const float xx = q.x * q.x;
    const float yy = q.y * q.y;
    const float zz = q.z * q.z;
    const float xy = q.x * q.y;
    const float xz = q.x * q.z;
    const float yz = q.y * q.z;
    const float wx = q.w * q.x;
    const float wy = q.w * q.y;
    const float wz = q.w * q.z;

    return {
        1.0f - 2.0f * (yy + zz), 2.0f * (xy - wz), 2.0f * (xz + wy),
        2.0f * (xy + wz), 1.0f - 2.0f * (xx + zz), 2.0f * (yz - wx),
        2.0f * (xz - wy), 2.0f * (yz + wx), 1.0f - 2.0f * (xx + yy)
    };
}

std::array<float, 3> inverse_rotate(const std::array<float, 9>& r, Vec3 value) {
    return {
        r[0] * value.x + r[3] * value.y + r[6] * value.z,
        r[1] * value.x + r[4] * value.y + r[7] * value.z,
        r[2] * value.x + r[5] * value.y + r[8] * value.z
    };
}

bool inside_gaussian_cutoff(const DataTable& table, std::size_t row, Vec3 point, float sigma) {
    const auto r = rotation_matrix({table.rot0[row], table.rot1[row], table.rot2[row], table.rot3[row]});
    const Vec3 delta{point.x - table.x[row], point.y - table.y[row], point.z - table.z[row]};
    const auto local = inverse_rotate(r, delta);
    const std::array<float, 3> axes{
        std::exp(table.scale0[row]) * sigma,
        std::exp(table.scale1[row]) * sigma,
        std::exp(table.scale2[row]) * sigma
    };
    if (axes[0] <= 0.0f || axes[1] <= 0.0f || axes[2] <= 0.0f) {
        return false;
    }
    const float d =
        (local[0] * local[0]) / (axes[0] * axes[0]) +
        (local[1] * local[1]) / (axes[1] * axes[1]) +
        (local[2] * local[2]) / (axes[2] * axes[2]);
    return d <= 1.0f;
}

int floor_to_index(float value, float origin, float voxel_size) {
    return static_cast<int>(std::floor((value - origin) / voxel_size));
}

int ceil_dim(float min_value, float max_value, float voxel_size) {
    return std::max(1, static_cast<int>(std::ceil((max_value - min_value) / voxel_size)));
}

int align4(int value) {
    return ((value + 3) / 4) * 4;
}

std::size_t voxel_count(int nx, int ny, int nz) {
    return static_cast<std::size_t>(nx) * static_cast<std::size_t>(ny) * static_cast<std::size_t>(nz);
}

int neighbor_count(const VoxelGrid& grid, int x, int y, int z) {
    int count = 0;
    for (int dz = -1; dz <= 1; ++dz) {
        for (int dy = -1; dy <= 1; ++dy) {
            for (int dx = -1; dx <= 1; ++dx) {
                if (grid.is_solid(x + dx, y + dy, z + dz)) {
                    ++count;
                }
            }
        }
    }
    return count;
}

float sqr(float value) {
    return value * value;
}

std::array<int, 3> nearest_solid_cell(const VoxelGrid& grid, Vec3 seed) {
    std::array<int, 3> best{-1, -1, -1};
    float best_distance = std::numeric_limits<float>::infinity();
    for (int z = 0; z < grid.nz; ++z) {
        for (int y = 0; y < grid.ny; ++y) {
            for (int x = 0; x < grid.nx; ++x) {
                if (!grid.is_solid(x, y, z)) {
                    continue;
                }
                const auto center = grid.cell_center(x, y, z);
                const float d2 = sqr(center.x - seed.x) + sqr(center.y - seed.y) + sqr(center.z - seed.z);
                if (d2 < best_distance) {
                    best_distance = d2;
                    best = {x, y, z};
                }
            }
        }
    }
    return best;
}

float distance_to_segment(Vec3 point, Vec3 a, Vec3 b) {
    const Vec3 ab{b.x - a.x, b.y - a.y, b.z - a.z};
    const Vec3 ap{point.x - a.x, point.y - a.y, point.z - a.z};
    const float len2 = sqr(ab.x) + sqr(ab.y) + sqr(ab.z);
    const float t = len2 == 0.0f
        ? 0.0f
        : std::clamp((ap.x * ab.x + ap.y * ab.y + ap.z * ab.z) / len2, 0.0f, 1.0f);
    const Vec3 closest{a.x + ab.x * t, a.y + ab.y * t, a.z + ab.z * t};
    return std::sqrt(sqr(point.x - closest.x) + sqr(point.y - closest.y) + sqr(point.z - closest.z));
}

} // namespace

DataTable filter_nan(const DataTable& table) {
    table.validate();
    DataTable out;
    for (std::size_t row = 0; row < table.size(); ++row) {
        if (finite_row(table, row)) {
            append_row(out, table, row);
        }
    }
    out.validate();
    return out;
}

DataTable filter_opacity_min(const DataTable& table, float alpha_min) {
    table.validate();
    if (alpha_min < 0.0f || alpha_min > 1.0f || !std::isfinite(alpha_min)) {
        throw std::runtime_error("alpha_min must be in [0, 1]");
    }
    DataTable out;
    for (std::size_t row = 0; row < table.size(); ++row) {
        if (sigmoid(table.opacity[row]) >= alpha_min) {
            append_row(out, table, row);
        }
    }
    out.validate();
    return out;
}

DataTable filter_box(const DataTable& table, const Bounds& bounds) {
    table.validate();
    DataTable out;
    for (std::size_t row = 0; row < table.size(); ++row) {
        if (bounds.contains({table.x[row], table.y[row], table.z[row]})) {
            append_row(out, table, row);
        }
    }
    out.validate();
    return out;
}

DataTable filter_sphere(const DataTable& table, Vec3 center, float radius) {
    table.validate();
    if (radius < 0.0f || !std::isfinite(radius)) {
        throw std::runtime_error("filter_sphere radius must be finite and non-negative");
    }
    const float radius2 = radius * radius;
    DataTable out;
    for (std::size_t row = 0; row < table.size(); ++row) {
        const float d2 = sqr(table.x[row] - center.x) + sqr(table.y[row] - center.y) + sqr(table.z[row] - center.z);
        if (d2 <= radius2) {
            append_row(out, table, row);
        }
    }
    out.validate();
    return out;
}

VoxelGrid voxelize(const DataTable& table, const VoxelOptions& options) {
    if (options.voxel_size <= 0.0f || !std::isfinite(options.voxel_size)) {
        throw std::runtime_error("voxel_size must be finite and positive");
    }
    if (options.sigma <= 0.0f || !std::isfinite(options.sigma)) {
        throw std::runtime_error("sigma must be finite and positive");
    }

    const auto filtered = filter_opacity_min(filter_nan(table), options.opacity_cutoff);
    Bounds scene = invalid_bounds();
    std::vector<GaussianExtent> extents;
    extents.reserve(filtered.size());
    for (std::size_t row = 0; row < filtered.size(); ++row) {
        auto extent = gaussian_extent(filtered, row, options.sigma);
        scene.expand(extent.bounds);
        extents.push_back(extent);
    }

    VoxelGrid grid;
    grid.voxel_size = options.voxel_size;
    if (filtered.size() == 0 || !scene.valid()) {
        return grid;
    }

    grid.origin = {
        std::floor(scene.min.x / options.voxel_size) * options.voxel_size,
        std::floor(scene.min.y / options.voxel_size) * options.voxel_size,
        std::floor(scene.min.z / options.voxel_size) * options.voxel_size
    };
    grid.nx = ceil_dim(grid.origin.x, scene.max.x, options.voxel_size);
    grid.ny = ceil_dim(grid.origin.y, scene.max.y, options.voxel_size);
    grid.nz = ceil_dim(grid.origin.z, scene.max.z, options.voxel_size);
    if (options.align_to_blocks) {
        grid.nx = align4(grid.nx);
        grid.ny = align4(grid.ny);
        grid.nz = align4(grid.nz);
    }
    if (voxel_count(grid.nx, grid.ny, grid.nz) > 200'000'000ULL) {
        throw std::runtime_error("voxel grid would exceed safety limit");
    }
    grid.solid.assign(voxel_count(grid.nx, grid.ny, grid.nz), 0);

    for (std::size_t row = 0; row < filtered.size(); ++row) {
        const auto& bounds = extents[row].bounds;
        const int min_x = std::max(0, floor_to_index(bounds.min.x, grid.origin.x, grid.voxel_size));
        const int min_y = std::max(0, floor_to_index(bounds.min.y, grid.origin.y, grid.voxel_size));
        const int min_z = std::max(0, floor_to_index(bounds.min.z, grid.origin.z, grid.voxel_size));
        const int max_x = std::min(grid.nx - 1, floor_to_index(bounds.max.x, grid.origin.x, grid.voxel_size));
        const int max_y = std::min(grid.ny - 1, floor_to_index(bounds.max.y, grid.origin.y, grid.voxel_size));
        const int max_z = std::min(grid.nz - 1, floor_to_index(bounds.max.z, grid.origin.z, grid.voxel_size));
        for (int z = min_z; z <= max_z; ++z) {
            for (int y = min_y; y <= max_y; ++y) {
                for (int x = min_x; x <= max_x; ++x) {
                    if (inside_gaussian_cutoff(filtered, row, grid.cell_center(x, y, z), options.sigma)) {
                        grid.set_solid(x, y, z);
                    }
                }
            }
        }
    }

    return grid;
}

VoxelGrid filter_floaters_by_voxel_contribution(const VoxelGrid& grid, int min_neighbors) {
    VoxelGrid out = grid;
    std::fill(out.solid.begin(), out.solid.end(), 0);
    for (int z = 0; z < grid.nz; ++z) {
        for (int y = 0; y < grid.ny; ++y) {
            for (int x = 0; x < grid.nx; ++x) {
                if (grid.is_solid(x, y, z) && neighbor_count(grid, x, y, z) >= min_neighbors) {
                    out.set_solid(x, y, z);
                }
            }
        }
    }
    return out;
}

VoxelGrid filter_cluster_from_seed(const VoxelGrid& grid, Vec3 seed) {
    VoxelGrid out = grid;
    std::fill(out.solid.begin(), out.solid.end(), 0);
    if (grid.occupied_count() == 0) {
        return out;
    }

    int sx = floor_to_index(seed.x, grid.origin.x, grid.voxel_size);
    int sy = floor_to_index(seed.y, grid.origin.y, grid.voxel_size);
    int sz = floor_to_index(seed.z, grid.origin.z, grid.voxel_size);
    if (!grid.is_solid(sx, sy, sz)) {
        const auto nearest = nearest_solid_cell(grid, seed);
        sx = nearest[0];
        sy = nearest[1];
        sz = nearest[2];
    }
    if (!grid.is_solid(sx, sy, sz)) {
        return out;
    }

    std::vector<std::uint8_t> visited(grid.solid.size(), 0);
    std::deque<std::array<int, 3>> queue;
    queue.push_back({sx, sy, sz});
    visited[grid.index(sx, sy, sz)] = 1;
    constexpr std::array<std::array<int, 3>, 6> dirs{{
        {{1, 0, 0}}, {{-1, 0, 0}}, {{0, 1, 0}}, {{0, -1, 0}}, {{0, 0, 1}}, {{0, 0, -1}}
    }};

    while (!queue.empty()) {
        const auto cell = queue.front();
        queue.pop_front();
        out.set_solid(cell[0], cell[1], cell[2]);
        for (const auto& dir : dirs) {
            const int nx = cell[0] + dir[0];
            const int ny = cell[1] + dir[1];
            const int nz = cell[2] + dir[2];
            if (!grid.is_solid(nx, ny, nz)) {
                continue;
            }
            const auto idx = grid.index(nx, ny, nz);
            if (visited[idx] == 0) {
                visited[idx] = 1;
                queue.push_back({nx, ny, nz});
            }
        }
    }

    return out;
}

VoxelGrid filter_sparse_blocks(const VoxelGrid& grid, int min_occupied_per_block) {
    VoxelGrid out = grid;
    std::fill(out.solid.begin(), out.solid.end(), 0);
    for (int bz = 0; bz < grid.nz; bz += 4) {
        for (int by = 0; by < grid.ny; by += 4) {
            for (int bx = 0; bx < grid.nx; bx += 4) {
                int count = 0;
                for (int z = bz; z < std::min(bz + 4, grid.nz); ++z) {
                    for (int y = by; y < std::min(by + 4, grid.ny); ++y) {
                        for (int x = bx; x < std::min(bx + 4, grid.nx); ++x) {
                            count += grid.is_solid(x, y, z) ? 1 : 0;
                        }
                    }
                }
                if (count >= min_occupied_per_block) {
                    for (int z = bz; z < std::min(bz + 4, grid.nz); ++z) {
                        for (int y = by; y < std::min(by + 4, grid.ny); ++y) {
                            for (int x = bx; x < std::min(bx + 4, grid.nx); ++x) {
                                out.set_solid(x, y, z, grid.is_solid(x, y, z));
                            }
                        }
                    }
                }
            }
        }
    }
    return out;
}

VoxelGrid fill_mixed_blocks(const VoxelGrid& grid, int min_occupied_per_block) {
    VoxelGrid out = grid;
    for (int bz = 0; bz < grid.nz; bz += 4) {
        for (int by = 0; by < grid.ny; by += 4) {
            for (int bx = 0; bx < grid.nx; bx += 4) {
                int count = 0;
                for (int z = bz; z < std::min(bz + 4, grid.nz); ++z) {
                    for (int y = by; y < std::min(by + 4, grid.ny); ++y) {
                        for (int x = bx; x < std::min(bx + 4, grid.nx); ++x) {
                            count += grid.is_solid(x, y, z) ? 1 : 0;
                        }
                    }
                }
                if (count >= min_occupied_per_block) {
                    for (int z = bz; z < std::min(bz + 4, grid.nz); ++z) {
                        for (int y = by; y < std::min(by + 4, grid.ny); ++y) {
                            for (int x = bx; x < std::min(bx + 4, grid.nx); ++x) {
                                out.set_solid(x, y, z);
                            }
                        }
                    }
                }
            }
        }
    }
    return out;
}

VoxelGrid floor_fill(const VoxelGrid& grid, int floor_layers) {
    VoxelGrid out = grid;
    const int layers = std::max(1, floor_layers);
    for (int z = 0; z < grid.nz; ++z) {
        for (int x = 0; x < grid.nx; ++x) {
            int lowest = -1;
            for (int y = 0; y < grid.ny; ++y) {
                if (grid.is_solid(x, y, z)) {
                    lowest = y;
                    break;
                }
            }
            if (lowest >= 0) {
                const int limit = std::min(grid.ny - 1, lowest + layers - 1);
                for (int y = 0; y <= limit; ++y) {
                    out.set_solid(x, y, z);
                }
            }
        }
    }
    return out;
}

VoxelGrid exterior_fill(const VoxelGrid& grid) {
    VoxelGrid out = grid;
    std::vector<std::uint8_t> exterior(grid.solid.size(), 0);
    std::deque<std::array<int, 3>> queue;

    auto push_empty = [&](int x, int y, int z) {
        if (!grid.in_bounds(x, y, z) || grid.is_solid(x, y, z)) {
            return;
        }
        const auto idx = grid.index(x, y, z);
        if (exterior[idx] != 0) {
            return;
        }
        exterior[idx] = 1;
        queue.push_back({x, y, z});
    };

    for (int z = 0; z < grid.nz; ++z) {
        for (int y = 0; y < grid.ny; ++y) {
            push_empty(0, y, z);
            push_empty(grid.nx - 1, y, z);
        }
    }
    for (int z = 0; z < grid.nz; ++z) {
        for (int x = 0; x < grid.nx; ++x) {
            push_empty(x, 0, z);
            push_empty(x, grid.ny - 1, z);
        }
    }
    for (int y = 0; y < grid.ny; ++y) {
        for (int x = 0; x < grid.nx; ++x) {
            push_empty(x, y, 0);
            push_empty(x, y, grid.nz - 1);
        }
    }

    constexpr std::array<std::array<int, 3>, 6> dirs{{
        {{1, 0, 0}}, {{-1, 0, 0}}, {{0, 1, 0}}, {{0, -1, 0}}, {{0, 0, 1}}, {{0, 0, -1}}
    }};
    while (!queue.empty()) {
        const auto cell = queue.front();
        queue.pop_front();
        for (const auto& dir : dirs) {
            push_empty(cell[0] + dir[0], cell[1] + dir[1], cell[2] + dir[2]);
        }
    }

    for (int z = 0; z < grid.nz; ++z) {
        for (int y = 0; y < grid.ny; ++y) {
            for (int x = 0; x < grid.nx; ++x) {
                const auto idx = grid.index(x, y, z);
                if (!grid.is_solid(x, y, z) && exterior[idx] == 0) {
                    out.set_solid(x, y, z);
                }
            }
        }
    }
    return out;
}

VoxelGrid capsule_carve(const VoxelGrid& grid, Vec3 start, Vec3 end, float radius) {
    VoxelGrid out = grid;
    if (radius < 0.0f || !std::isfinite(radius)) {
        throw std::runtime_error("capsule radius must be finite and non-negative");
    }
    for (int z = 0; z < grid.nz; ++z) {
        for (int y = 0; y < grid.ny; ++y) {
            for (int x = 0; x < grid.nx; ++x) {
                if (grid.is_solid(x, y, z) && distance_to_segment(grid.cell_center(x, y, z), start, end) <= radius) {
                    out.set_solid(x, y, z, false);
                }
            }
        }
    }
    return out;
}

VoxelGrid crop_to_occupied(const VoxelGrid& grid) {
    int min_x = grid.nx;
    int min_y = grid.ny;
    int min_z = grid.nz;
    int max_x = -1;
    int max_y = -1;
    int max_z = -1;
    for (int z = 0; z < grid.nz; ++z) {
        for (int y = 0; y < grid.ny; ++y) {
            for (int x = 0; x < grid.nx; ++x) {
                if (grid.is_solid(x, y, z)) {
                    min_x = std::min(min_x, x);
                    min_y = std::min(min_y, y);
                    min_z = std::min(min_z, z);
                    max_x = std::max(max_x, x);
                    max_y = std::max(max_y, y);
                    max_z = std::max(max_z, z);
                }
            }
        }
    }
    VoxelGrid out;
    out.voxel_size = grid.voxel_size;
    if (max_x < min_x) {
        return out;
    }
    out.origin = {
        grid.origin.x + static_cast<float>(min_x) * grid.voxel_size,
        grid.origin.y + static_cast<float>(min_y) * grid.voxel_size,
        grid.origin.z + static_cast<float>(min_z) * grid.voxel_size
    };
    out.nx = max_x - min_x + 1;
    out.ny = max_y - min_y + 1;
    out.nz = max_z - min_z + 1;
    out.solid.assign(voxel_count(out.nx, out.ny, out.nz), 0);
    for (int z = min_z; z <= max_z; ++z) {
        for (int y = min_y; y <= max_y; ++y) {
            for (int x = min_x; x <= max_x; ++x) {
                out.set_solid(x - min_x, y - min_y, z - min_z, grid.is_solid(x, y, z));
            }
        }
    }
    return out;
}

} // namespace ga3d
