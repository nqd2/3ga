#pragma once

#include "ga3d/data_table.hpp"

#include <array>
#include <cstdint>
#include <variant>
#include <vector>

namespace ga3d {

enum SplatState : std::uint8_t {
    Selected = 1 << 0,
    Deleted = 1 << 1,
    Locked = 1 << 2
};

enum class SelectMode {
    Set,
    Add,
    Remove
};

struct SelectAll {};
struct SelectNone {};

struct SelectBox {
    SelectMode mode = SelectMode::Set;
    std::array<float, 3> center{ 0.0f, 0.0f, 0.0f };
    std::array<float, 3> size{ 1.0f, 1.0f, 1.0f };
};

struct DeleteSelected {};

struct TransformSelected {
    std::array<float, 16> matrix{
        1.0f, 0.0f, 0.0f, 0.0f,
        0.0f, 1.0f, 0.0f, 0.0f,
        0.0f, 0.0f, 1.0f, 0.0f,
        0.0f, 0.0f, 0.0f, 1.0f
    };
};

struct FilterOpacity {
    float min = 0.0f;
};

using EditOperation = std::variant<SelectAll, SelectNone, SelectBox, DeleteSelected, TransformSelected, FilterOpacity>;

struct EditRecipe {
    std::vector<EditOperation> ops;
};

DataTable bake_edits(const DataTable& input, const EditRecipe& recipe);

} // namespace ga3d
