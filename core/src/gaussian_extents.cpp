#include "ga3d/voxel.hpp"

#include <array>
#include <cmath>
#include <stdexcept>

namespace ga3d {

namespace {

struct Quaternion {
    float w;
    float x;
    float y;
    float z;
};

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

} // namespace

GaussianExtent gaussian_extent(const DataTable& table, std::size_t row, float sigma) {
    table.validate();
    if (row >= table.size()) {
        throw std::out_of_range("gaussian_extent row is out of range");
    }
    if (sigma <= 0.0f || !std::isfinite(sigma)) {
        throw std::runtime_error("gaussian_extent sigma must be finite and positive");
    }

    const std::array<float, 3> axes{
        std::exp(table.scale0[row]) * sigma,
        std::exp(table.scale1[row]) * sigma,
        std::exp(table.scale2[row]) * sigma
    };
    for (const float axis : axes) {
        if (!std::isfinite(axis) || axis <= 0.0f) {
            throw std::runtime_error("gaussian_extent scale produced invalid axis");
        }
    }

    const auto r = rotation_matrix({table.rot0[row], table.rot1[row], table.rot2[row], table.rot3[row]});
    const float ex = std::sqrt(
        r[0] * r[0] * axes[0] * axes[0] +
        r[1] * r[1] * axes[1] * axes[1] +
        r[2] * r[2] * axes[2] * axes[2]
    );
    const float ey = std::sqrt(
        r[3] * r[3] * axes[0] * axes[0] +
        r[4] * r[4] * axes[1] * axes[1] +
        r[5] * r[5] * axes[2] * axes[2]
    );
    const float ez = std::sqrt(
        r[6] * r[6] * axes[0] * axes[0] +
        r[7] * r[7] * axes[1] * axes[1] +
        r[8] * r[8] * axes[2] * axes[2]
    );

    const Vec3 center{table.x[row], table.y[row], table.z[row]};
    return {
        {
            {center.x - ex, center.y - ey, center.z - ez},
            {center.x + ex, center.y + ey, center.z + ez}
        },
        axes
    };
}

} // namespace ga3d
