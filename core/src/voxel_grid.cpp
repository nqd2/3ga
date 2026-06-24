#include "ga3d/voxel.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <stdexcept>

namespace ga3d {

bool Bounds::contains(const Vec3& point) const {
    return point.x >= min.x && point.x <= max.x &&
           point.y >= min.y && point.y <= max.y &&
           point.z >= min.z && point.z <= max.z;
}

bool Bounds::valid() const {
    return min.x <= max.x && min.y <= max.y && min.z <= max.z;
}

void Bounds::expand(const Vec3& point) {
    if (!valid()) {
        min = point;
        max = point;
        return;
    }
    min.x = std::min(min.x, point.x);
    min.y = std::min(min.y, point.y);
    min.z = std::min(min.z, point.z);
    max.x = std::max(max.x, point.x);
    max.y = std::max(max.y, point.y);
    max.z = std::max(max.z, point.z);
}

void Bounds::expand(const Bounds& bounds) {
    if (!bounds.valid()) {
        return;
    }
    expand(bounds.min);
    expand(bounds.max);
}

std::size_t VoxelGrid::index(int x, int y, int z) const {
    return static_cast<std::size_t>((z * ny + y) * nx + x);
}

bool VoxelGrid::in_bounds(int x, int y, int z) const {
    return x >= 0 && y >= 0 && z >= 0 && x < nx && y < ny && z < nz;
}

bool VoxelGrid::is_solid(int x, int y, int z) const {
    return in_bounds(x, y, z) && solid[index(x, y, z)] != 0;
}

void VoxelGrid::set_solid(int x, int y, int z, bool value) {
    if (!in_bounds(x, y, z)) {
        return;
    }
    solid[index(x, y, z)] = value ? 1 : 0;
}

Vec3 VoxelGrid::cell_center(int x, int y, int z) const {
    return {
        origin.x + (static_cast<float>(x) + 0.5f) * voxel_size,
        origin.y + (static_cast<float>(y) + 0.5f) * voxel_size,
        origin.z + (static_cast<float>(z) + 0.5f) * voxel_size
    };
}

Bounds VoxelGrid::bounds() const {
    if (nx <= 0 || ny <= 0 || nz <= 0) {
        const float nan = std::numeric_limits<float>::quiet_NaN();
        return {{nan, nan, nan}, {nan, nan, nan}};
    }
    return {
        origin,
        {
            origin.x + static_cast<float>(nx) * voxel_size,
            origin.y + static_cast<float>(ny) * voxel_size,
            origin.z + static_cast<float>(nz) * voxel_size
        }
    };
}

std::size_t VoxelGrid::occupied_count() const {
    return static_cast<std::size_t>(std::count(solid.begin(), solid.end(), std::uint8_t{1}));
}

} // namespace ga3d
