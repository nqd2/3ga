#include "ga3d/voxel.hpp"

#include <algorithm>
#include <array>
#include <cstdint>
#include <limits>
#include <vector>

namespace ga3d {

namespace {

Bounds invalid_bounds() {
    const float inf = std::numeric_limits<float>::infinity();
    return {{inf, inf, inf}, {-inf, -inf, -inf}};
}

Vec3 point_for_axis(const VoxelGrid& grid, int axis, int plane, int u, int v) {
    const float p = (axis == 0 ? grid.origin.x : axis == 1 ? grid.origin.y : grid.origin.z) +
                    static_cast<float>(plane) * grid.voxel_size;
    const float a = (axis == 0 ? grid.origin.y : grid.origin.x) + static_cast<float>(u) * grid.voxel_size;
    const float b = (axis == 2 ? grid.origin.y : grid.origin.z) + static_cast<float>(v) * grid.voxel_size;

    if (axis == 0) {
        return {p, a, b};
    }
    if (axis == 1) {
        return {a, p, b};
    }
    return {a, b, p};
}

void emit_quad(Mesh& mesh, Vec3 a, Vec3 b, Vec3 c, Vec3 d, bool reverse) {
    const auto base = static_cast<std::uint32_t>(mesh.positions.size() / 3);
    const Vec3 verts[4] = {a, b, c, d};
    for (const auto& vertex : verts) {
        mesh.positions.push_back(vertex.x);
        mesh.positions.push_back(vertex.y);
        mesh.positions.push_back(vertex.z);
        mesh.bounds.expand(vertex);
    }
    if (reverse) {
        mesh.indices.insert(mesh.indices.end(), {base, base + 2, base + 1, base, base + 3, base + 2});
    } else {
        mesh.indices.insert(mesh.indices.end(), {base, base + 1, base + 2, base, base + 2, base + 3});
    }
}

void emit_merged_face(Mesh& mesh, const VoxelGrid& grid, int axis, bool positive, int plane, int u0, int v0, int w, int h) {
    const Vec3 a = point_for_axis(grid, axis, plane, u0, v0);
    const Vec3 b = point_for_axis(grid, axis, plane, u0 + w, v0);
    const Vec3 c = point_for_axis(grid, axis, plane, u0 + w, v0 + h);
    const Vec3 d = point_for_axis(grid, axis, plane, u0, v0 + h);
    emit_quad(mesh, a, b, c, d, !positive);
}

bool face_visible(const VoxelGrid& grid, int axis, bool positive, int x, int y, int z) {
    if (!grid.is_solid(x, y, z)) {
        return false;
    }
    if (axis == 0) {
        return positive ? !grid.is_solid(x + 1, y, z) : !grid.is_solid(x - 1, y, z);
    }
    if (axis == 1) {
        return positive ? !grid.is_solid(x, y + 1, z) : !grid.is_solid(x, y - 1, z);
    }
    return positive ? !grid.is_solid(x, y, z + 1) : !grid.is_solid(x, y, z - 1);
}

void cell_for_uv(int axis, int plane, bool positive, int u, int v, int& x, int& y, int& z) {
    const int cell_plane = positive ? plane - 1 : plane;
    if (axis == 0) {
        x = cell_plane;
        y = u;
        z = v;
    } else if (axis == 1) {
        x = u;
        y = cell_plane;
        z = v;
    } else {
        x = u;
        y = v;
        z = cell_plane;
    }
}

int plane_count(const VoxelGrid& grid, int axis) {
    return axis == 0 ? grid.nx + 1 : axis == 1 ? grid.ny + 1 : grid.nz + 1;
}

int u_count(const VoxelGrid& grid, int axis) {
    return axis == 0 ? grid.ny : grid.nx;
}

int v_count(const VoxelGrid& grid, int axis) {
    return axis == 2 ? grid.ny : grid.nz;
}

void emit_axis_faces(Mesh& mesh, const VoxelGrid& grid, int axis, bool positive) {
    const int planes = plane_count(grid, axis);
    const int nu = u_count(grid, axis);
    const int nv = v_count(grid, axis);
    std::vector<std::uint8_t> mask(static_cast<std::size_t>(nu * nv), 0);
    std::vector<std::uint8_t> used(static_cast<std::size_t>(nu * nv), 0);

    for (int plane = 0; plane < planes; ++plane) {
        std::fill(mask.begin(), mask.end(), 0);
        std::fill(used.begin(), used.end(), 0);

        for (int v = 0; v < nv; ++v) {
            for (int u = 0; u < nu; ++u) {
                int x = 0;
                int y = 0;
                int z = 0;
                cell_for_uv(axis, plane, positive, u, v, x, y, z);
                mask[static_cast<std::size_t>(v * nu + u)] = face_visible(grid, axis, positive, x, y, z) ? 1 : 0;
            }
        }

        for (int v = 0; v < nv; ++v) {
            for (int u = 0; u < nu; ++u) {
                const auto start = static_cast<std::size_t>(v * nu + u);
                if (mask[start] == 0 || used[start] != 0) {
                    continue;
                }

                int width = 1;
                while (u + width < nu) {
                    const auto idx = static_cast<std::size_t>(v * nu + u + width);
                    if (mask[idx] == 0 || used[idx] != 0) {
                        break;
                    }
                    ++width;
                }

                int height = 1;
                bool can_extend = true;
                while (v + height < nv && can_extend) {
                    for (int du = 0; du < width; ++du) {
                        const auto idx = static_cast<std::size_t>((v + height) * nu + u + du);
                        if (mask[idx] == 0 || used[idx] != 0) {
                            can_extend = false;
                            break;
                        }
                    }
                    if (can_extend) {
                        ++height;
                    }
                }

                for (int dv = 0; dv < height; ++dv) {
                    for (int du = 0; du < width; ++du) {
                        used[static_cast<std::size_t>((v + dv) * nu + u + du)] = 1;
                    }
                }
                emit_merged_face(mesh, grid, axis, positive, plane, u, v, width, height);
            }
        }
    }
}

Vec3 scalar_position(const VoxelGrid& grid, int x, int y, int z) {
    return {
        grid.origin.x + (static_cast<float>(x) - 0.5f) * grid.voxel_size,
        grid.origin.y + (static_cast<float>(y) - 0.5f) * grid.voxel_size,
        grid.origin.z + (static_cast<float>(z) - 0.5f) * grid.voxel_size
    };
}

bool scalar_value(const VoxelGrid& grid, int x, int y, int z) {
    return grid.is_solid(x - 1, y - 1, z - 1);
}

Vec3 midpoint(Vec3 a, Vec3 b) {
    return {
        (a.x + b.x) * 0.5f,
        (a.y + b.y) * 0.5f,
        (a.z + b.z) * 0.5f
    };
}

void emit_triangle(Mesh& mesh, Vec3 a, Vec3 b, Vec3 c) {
    const auto base = static_cast<std::uint32_t>(mesh.positions.size() / 3);
    const Vec3 verts[3] = {a, b, c};
    for (const auto& vertex : verts) {
        mesh.positions.push_back(vertex.x);
        mesh.positions.push_back(vertex.y);
        mesh.positions.push_back(vertex.z);
        mesh.bounds.expand(vertex);
    }
    mesh.indices.insert(mesh.indices.end(), {base, base + 1, base + 2});
}

void emit_tetra(Mesh& mesh, const std::array<Vec3, 4>& points, const std::array<bool, 4>& solid) {
    constexpr std::array<std::array<int, 2>, 6> edges{{
        {{0, 1}}, {{0, 2}}, {{0, 3}}, {{1, 2}}, {{1, 3}}, {{2, 3}}
    }};

    std::vector<Vec3> crossings;
    crossings.reserve(4);
    for (const auto& edge : edges) {
        if (solid[edge[0]] != solid[edge[1]]) {
            crossings.push_back(midpoint(points[edge[0]], points[edge[1]]));
        }
    }

    if (crossings.size() == 3) {
        emit_triangle(mesh, crossings[0], crossings[1], crossings[2]);
    } else if (crossings.size() == 4) {
        emit_triangle(mesh, crossings[0], crossings[1], crossings[2]);
        emit_triangle(mesh, crossings[0], crossings[2], crossings[3]);
    }
}

Mesh extract_smooth_mesh(const VoxelGrid& grid) {
    Mesh mesh;
    mesh.bounds = invalid_bounds();
    constexpr std::array<std::array<int, 3>, 8> corners{{
        {{0, 0, 0}}, {{1, 0, 0}}, {{1, 1, 0}}, {{0, 1, 0}},
        {{0, 0, 1}}, {{1, 0, 1}}, {{1, 1, 1}}, {{0, 1, 1}}
    }};
    constexpr std::array<std::array<int, 4>, 6> tetrahedra{{
        {{0, 5, 1, 6}}, {{0, 1, 2, 6}}, {{0, 2, 3, 6}},
        {{0, 3, 7, 6}}, {{0, 7, 4, 6}}, {{0, 4, 5, 6}}
    }};

    for (int z = 0; z <= grid.nz; ++z) {
        for (int y = 0; y <= grid.ny; ++y) {
            for (int x = 0; x <= grid.nx; ++x) {
                std::array<Vec3, 8> cube_points{};
                std::array<bool, 8> cube_solid{};
                for (std::size_t i = 0; i < corners.size(); ++i) {
                    const int cx = x + corners[i][0];
                    const int cy = y + corners[i][1];
                    const int cz = z + corners[i][2];
                    cube_points[i] = scalar_position(grid, cx, cy, cz);
                    cube_solid[i] = scalar_value(grid, cx, cy, cz);
                }
                for (const auto& tetra : tetrahedra) {
                    emit_tetra(
                        mesh,
                        {cube_points[tetra[0]], cube_points[tetra[1]], cube_points[tetra[2]], cube_points[tetra[3]]},
                        {cube_solid[tetra[0]], cube_solid[tetra[1]], cube_solid[tetra[2]], cube_solid[tetra[3]]}
                    );
                }
            }
        }
    }

    return mesh;
}

} // namespace

Mesh extract_occlusion_mesh(const VoxelGrid& grid, MeshMode mode) {
    Mesh mesh;
    mesh.bounds = invalid_bounds();
    if (grid.nx <= 0 || grid.ny <= 0 || grid.nz <= 0 || grid.solid.empty()) {
        return mesh;
    }
    if (mode == MeshMode::Smooth) {
        return extract_smooth_mesh(grid);
    }

    for (int axis = 0; axis < 3; ++axis) {
        emit_axis_faces(mesh, grid, axis, false);
        emit_axis_faces(mesh, grid, axis, true);
    }
    return mesh;
}

} // namespace ga3d
