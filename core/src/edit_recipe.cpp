#include "ga3d/edit_recipe.hpp"

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <vector>

namespace ga3d {

namespace {

struct Quaternion {
    float w = 1.0f;
    float x = 0.0f;
    float y = 0.0f;
    float z = 0.0f;
};

struct TransformParts {
    std::array<float, 3> scales{};
    std::array<float, 9> rotation{};
    Quaternion quaternion{};
    bool rotates = false;
    std::vector<float> sh_rotation;
    std::size_t sh_coeffs_per_channel = 0;
};

float sigmoid(float value) {
    return 1.0f / (1.0f + std::exp(-value));
}

bool has_state(std::uint8_t state, SplatState bit) {
    return (state & bit) != 0;
}

void set_state(std::uint8_t& state, SplatState bit) {
    state = static_cast<std::uint8_t>(state | bit);
}

void clear_state(std::uint8_t& state, SplatState bit) {
    state = static_cast<std::uint8_t>(state & ~bit);
}

bool selectable(std::uint8_t state) {
    return !has_state(state, Deleted) && !has_state(state, Locked);
}

bool inside_box(const DataTable& table, std::size_t row, const SelectBox& box) {
    const float hx = box.size[0] * 0.5f;
    const float hy = box.size[1] * 0.5f;
    const float hz = box.size[2] * 0.5f;
    return std::abs(table.x[row] - box.center[0]) <= hx &&
           std::abs(table.y[row] - box.center[1]) <= hy &&
           std::abs(table.z[row] - box.center[2]) <= hz;
}

Quaternion normalize(Quaternion q) {
    const float length = std::sqrt(q.w * q.w + q.x * q.x + q.y * q.y + q.z * q.z);
    if (length == 0.0f) {
        return {};
    }
    q.w /= length;
    q.x /= length;
    q.y /= length;
    q.z /= length;
    return q;
}

Quaternion multiply(const Quaternion& a, const Quaternion& b) {
    return normalize({
        a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
        a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w
    });
}

Quaternion quaternion_from_matrix(const std::array<float, 9>& m) {
    const float trace = m[0] + m[4] + m[8];
    Quaternion q;
    if (trace > 0.0f) {
        const float s = std::sqrt(trace + 1.0f) * 2.0f;
        q.w = 0.25f * s;
        q.x = (m[7] - m[5]) / s;
        q.y = (m[2] - m[6]) / s;
        q.z = (m[3] - m[1]) / s;
    } else if (m[0] > m[4] && m[0] > m[8]) {
        const float s = std::sqrt(1.0f + m[0] - m[4] - m[8]) * 2.0f;
        q.w = (m[7] - m[5]) / s;
        q.x = 0.25f * s;
        q.y = (m[1] + m[3]) / s;
        q.z = (m[2] + m[6]) / s;
    } else if (m[4] > m[8]) {
        const float s = std::sqrt(1.0f + m[4] - m[0] - m[8]) * 2.0f;
        q.w = (m[2] - m[6]) / s;
        q.x = (m[1] + m[3]) / s;
        q.y = 0.25f * s;
        q.z = (m[5] + m[7]) / s;
    } else {
        const float s = std::sqrt(1.0f + m[8] - m[0] - m[4]) * 2.0f;
        q.w = (m[3] - m[1]) / s;
        q.x = (m[2] + m[6]) / s;
        q.y = (m[5] + m[7]) / s;
        q.z = 0.25f * s;
    }
    return normalize(q);
}

bool is_identity_rotation(const std::array<float, 9>& m) {
    constexpr float eps = 1e-5f;
    return std::abs(m[0] - 1.0f) < eps && std::abs(m[4] - 1.0f) < eps && std::abs(m[8] - 1.0f) < eps &&
           std::abs(m[1]) < eps && std::abs(m[2]) < eps && std::abs(m[3]) < eps &&
           std::abs(m[5]) < eps && std::abs(m[6]) < eps && std::abs(m[7]) < eps;
}

std::array<float, 9> normalized_rotation_columns(const std::array<float, 16>& matrix, std::array<float, 3>& scales) {
    scales[0] = std::sqrt(matrix[0] * matrix[0] + matrix[1] * matrix[1] + matrix[2] * matrix[2]);
    scales[1] = std::sqrt(matrix[4] * matrix[4] + matrix[5] * matrix[5] + matrix[6] * matrix[6]);
    scales[2] = std::sqrt(matrix[8] * matrix[8] + matrix[9] * matrix[9] + matrix[10] * matrix[10]);
    for (float scale : scales) {
        if (scale <= 0.0f || !std::isfinite(scale)) {
            throw std::runtime_error("transformSelected matrix has invalid scale");
        }
    }
    return {
        matrix[0] / scales[0], matrix[4] / scales[1], matrix[8] / scales[2],
        matrix[1] / scales[0], matrix[5] / scales[1], matrix[9] / scales[2],
        matrix[2] / scales[0], matrix[6] / scales[1], matrix[10] / scales[2]
    };
}

std::size_t sh_coeffs_per_channel(const DataTable& table) {
    if (table.sh_rest.empty()) {
        return 0;
    }
    if (table.sh_rest.size() % 3 != 0) {
        throw std::runtime_error("sh_rest must store RGB channel-major coefficients");
    }
    const std::size_t coeffs = table.sh_rest.size() / 3;
    if (coeffs != 3 && coeffs != 8 && coeffs != 15) {
        throw std::runtime_error("sh_rest supports degree 1, 2, or 3 coefficients only");
    }
    return coeffs;
}

std::array<float, 3> normalize_vec(std::array<float, 3> value) {
    const float length = std::sqrt(value[0] * value[0] + value[1] * value[1] + value[2] * value[2]);
    if (length == 0.0f) {
        return {0.0f, 0.0f, 1.0f};
    }
    return {value[0] / length, value[1] / length, value[2] / length};
}

std::array<float, 3> rotate_inverse(const std::array<float, 9>& r, const std::array<float, 3>& value) {
    return {
        r[0] * value[0] + r[3] * value[1] + r[6] * value[2],
        r[1] * value[0] + r[4] * value[1] + r[7] * value[2],
        r[2] * value[0] + r[5] * value[1] + r[8] * value[2]
    };
}

std::vector<float> sh_basis(const std::array<float, 3>& direction, std::size_t coeffs) {
    const float x = direction[0];
    const float y = direction[1];
    const float z = direction[2];
    std::vector<float> out(coeffs, 0.0f);

    constexpr float c1 = 0.4886025119029199f;
    out[0] = -c1 * y;
    out[1] = c1 * z;
    out[2] = -c1 * x;

    if (coeffs >= 8) {
        constexpr float c20 = 1.0925484305920792f;
        constexpr float c21 = -1.0925484305920792f;
        constexpr float c22 = 0.31539156525252005f;
        constexpr float c23 = -1.0925484305920792f;
        constexpr float c24 = 0.5462742152960396f;
        out[3] = c20 * x * y;
        out[4] = c21 * y * z;
        out[5] = c22 * (2.0f * z * z - x * x - y * y);
        out[6] = c23 * x * z;
        out[7] = c24 * (x * x - y * y);
    }

    if (coeffs >= 15) {
        constexpr float c30 = -0.5900435899266435f;
        constexpr float c31 = 2.890611442640554f;
        constexpr float c32 = -0.4570457994644658f;
        constexpr float c33 = 0.3731763325901154f;
        constexpr float c34 = -0.4570457994644658f;
        constexpr float c35 = 1.445305721320277f;
        constexpr float c36 = -0.5900435899266435f;
        out[8] = c30 * y * (3.0f * x * x - y * y);
        out[9] = c31 * x * y * z;
        out[10] = c32 * y * (4.0f * z * z - x * x - y * y);
        out[11] = c33 * z * (2.0f * z * z - 3.0f * x * x - 3.0f * y * y);
        out[12] = c34 * x * (4.0f * z * z - x * x - y * y);
        out[13] = c35 * z * (x * x - y * y);
        out[14] = c36 * x * (x * x - 3.0f * y * y);
    }

    return out;
}

std::vector<float> solve_square_system(std::vector<float> a, std::vector<float> b, std::size_t n) {
    for (std::size_t col = 0; col < n; ++col) {
        std::size_t pivot = col;
        for (std::size_t row = col + 1; row < n; ++row) {
            if (std::abs(a[row * n + col]) > std::abs(a[pivot * n + col])) {
                pivot = row;
            }
        }
        if (std::abs(a[pivot * n + col]) < 1e-7f) {
            throw std::runtime_error("SH rotation solve became singular");
        }
        if (pivot != col) {
            for (std::size_t k = 0; k < n; ++k) {
                std::swap(a[col * n + k], a[pivot * n + k]);
            }
            std::swap(b[col], b[pivot]);
        }
        const float div = a[col * n + col];
        for (std::size_t k = col; k < n; ++k) {
            a[col * n + k] /= div;
        }
        b[col] /= div;
        for (std::size_t row = 0; row < n; ++row) {
            if (row == col) {
                continue;
            }
            const float factor = a[row * n + col];
            if (factor == 0.0f) {
                continue;
            }
            for (std::size_t k = col; k < n; ++k) {
                a[row * n + k] -= factor * a[col * n + k];
            }
            b[row] -= factor * b[col];
        }
    }
    return b;
}

std::vector<float> build_sh_rotation_matrix(const std::array<float, 9>& rotation, std::size_t coeffs) {
    // Clean-room SH bake: numerically project the rotated real SH basis instead of porting
    // SuperSplat's closed-form rotation helper.
    constexpr std::size_t sample_count = 96;
    constexpr float golden_angle = 2.39996322972865332f;

    std::vector<float> normal(coeffs * coeffs, 0.0f);
    std::vector<float> rhs(coeffs * coeffs, 0.0f);

    for (std::size_t sample = 0; sample < sample_count; ++sample) {
        const float t = (static_cast<float>(sample) + 0.5f) / static_cast<float>(sample_count);
        const float z = 1.0f - 2.0f * t;
        const float radius = std::sqrt(std::max(0.0f, 1.0f - z * z));
        const float theta = golden_angle * static_cast<float>(sample);
        const auto direction = normalize_vec({radius * std::cos(theta), radius * std::sin(theta), z});
        const auto rotated_direction = normalize_vec(rotate_inverse(rotation, direction));
        const auto basis = sh_basis(direction, coeffs);
        const auto rotated_basis = sh_basis(rotated_direction, coeffs);

        for (std::size_t row = 0; row < coeffs; ++row) {
            for (std::size_t col = 0; col < coeffs; ++col) {
                normal[row * coeffs + col] += basis[row] * basis[col];
                rhs[row * coeffs + col] += basis[row] * rotated_basis[col];
            }
        }
    }

    std::vector<float> transform(coeffs * coeffs, 0.0f);
    for (std::size_t col = 0; col < coeffs; ++col) {
        std::vector<float> rhs_col(coeffs, 0.0f);
        for (std::size_t row = 0; row < coeffs; ++row) {
            rhs_col[row] = rhs[row * coeffs + col];
        }
        const auto solved = solve_square_system(normal, rhs_col, coeffs);
        for (std::size_t row = 0; row < coeffs; ++row) {
            transform[row * coeffs + col] = solved[row];
        }
    }
    return transform;
}

void rotate_sh_coefficients(DataTable& table, std::size_t row, const TransformParts& parts) {
    if (!parts.rotates || parts.sh_coeffs_per_channel == 0) {
        return;
    }

    const std::size_t coeffs = parts.sh_coeffs_per_channel;
    std::vector<float> original(coeffs, 0.0f);
    for (std::size_t channel = 0; channel < 3; ++channel) {
        for (std::size_t i = 0; i < coeffs; ++i) {
            original[i] = table.sh_rest[channel * coeffs + i][row];
        }
        for (std::size_t out = 0; out < coeffs; ++out) {
            float value = 0.0f;
            for (std::size_t in = 0; in < coeffs; ++in) {
                value += parts.sh_rotation[out * coeffs + in] * original[in];
            }
            table.sh_rest[channel * coeffs + out][row] = value;
        }
    }
}

TransformParts prepare_transform(const TransformSelected& op, const DataTable& table) {
    TransformParts parts;
    parts.rotation = normalized_rotation_columns(op.matrix, parts.scales);
    parts.rotates = !is_identity_rotation(parts.rotation);
    parts.quaternion = parts.rotates ? quaternion_from_matrix(parts.rotation) : Quaternion{};
    parts.sh_coeffs_per_channel = sh_coeffs_per_channel(table);
    if (parts.rotates && parts.sh_coeffs_per_channel > 0) {
        parts.sh_rotation = build_sh_rotation_matrix(parts.rotation, parts.sh_coeffs_per_channel);
    }
    return parts;
}

void apply_transform(DataTable& table, std::size_t row, const TransformSelected& op, const TransformParts& parts) {
    const float x = table.x[row];
    const float y = table.y[row];
    const float z = table.z[row];
    table.x[row] = op.matrix[0] * x + op.matrix[4] * y + op.matrix[8] * z + op.matrix[12];
    table.y[row] = op.matrix[1] * x + op.matrix[5] * y + op.matrix[9] * z + op.matrix[13];
    table.z[row] = op.matrix[2] * x + op.matrix[6] * y + op.matrix[10] * z + op.matrix[14];

    table.scale0[row] += std::log(parts.scales[0]);
    table.scale1[row] += std::log(parts.scales[1]);
    table.scale2[row] += std::log(parts.scales[2]);

    if (parts.rotates) {
        const Quaternion current = normalize({ table.rot0[row], table.rot1[row], table.rot2[row], table.rot3[row] });
        const Quaternion result = multiply(parts.quaternion, current);
        table.rot0[row] = result.w;
        table.rot1[row] = result.x;
        table.rot2[row] = result.y;
        table.rot3[row] = result.z;
        rotate_sh_coefficients(table, row, parts);
    }
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

} // namespace

DataTable bake_edits(const DataTable& input, const EditRecipe& recipe) {
    input.validate();
    DataTable working = input;
    std::vector<std::uint8_t> state(working.size(), 0);

    for (const auto& operation : recipe.ops) {
        std::visit([&](const auto& op) {
            using Op = std::decay_t<decltype(op)>;
            if constexpr (std::is_same_v<Op, SelectAll>) {
                for (auto& row_state : state) {
                    if (selectable(row_state)) {
                        set_state(row_state, Selected);
                    }
                }
            } else if constexpr (std::is_same_v<Op, SelectNone>) {
                for (auto& row_state : state) {
                    clear_state(row_state, Selected);
                }
            } else if constexpr (std::is_same_v<Op, SelectBox>) {
                for (std::size_t row = 0; row < state.size(); ++row) {
                    if (!selectable(state[row])) {
                        continue;
                    }
                    const bool hit = inside_box(working, row, op);
                    if (op.mode == SelectMode::Set) {
                        if (hit) set_state(state[row], Selected);
                        else clear_state(state[row], Selected);
                    } else if (op.mode == SelectMode::Add) {
                        if (hit) set_state(state[row], Selected);
                    } else if (op.mode == SelectMode::Remove) {
                        if (hit) clear_state(state[row], Selected);
                    }
                }
            } else if constexpr (std::is_same_v<Op, DeleteSelected>) {
                for (auto& row_state : state) {
                    if (has_state(row_state, Selected) && !has_state(row_state, Locked)) {
                        set_state(row_state, Deleted);
                        clear_state(row_state, Selected);
                    }
                }
            } else if constexpr (std::is_same_v<Op, TransformSelected>) {
                const auto parts = prepare_transform(op, working);
                for (std::size_t row = 0; row < state.size(); ++row) {
                    if (has_state(state[row], Selected) && !has_state(state[row], Deleted)) {
                        apply_transform(working, row, op, parts);
                    }
                }
            } else if constexpr (std::is_same_v<Op, FilterOpacity>) {
                if (op.min < 0.0f || op.min > 1.0f) {
                    throw std::runtime_error("filterOpacity min must be in [0, 1]");
                }
                for (std::size_t row = 0; row < state.size(); ++row) {
                    if (sigmoid(working.opacity[row]) < op.min) {
                        set_state(state[row], Deleted);
                        clear_state(state[row], Selected);
                    }
                }
            }
        }, operation);
    }

    DataTable output;
    for (std::size_t row = 0; row < working.size(); ++row) {
        if (!has_state(state[row], Deleted)) {
            append_row(output, working, row);
        }
    }
    output.validate();
    return output;
}

} // namespace ga3d
