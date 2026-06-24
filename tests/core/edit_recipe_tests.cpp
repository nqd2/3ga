#include "ga3d/edit_recipe.hpp"

#include <cmath>
#include <cstdlib>
#include <iostream>
#include <string>
#include <vector>

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

ga3d::DataTable make_two_splat_table() {
    ga3d::DataTable table;
    table.x = {0.0f, 2.0f};
    table.y = {0.0f, 0.0f};
    table.z = {0.0f, 0.0f};
    table.scale0 = {0.0f, 0.0f};
    table.scale1 = {0.0f, 0.0f};
    table.scale2 = {0.0f, 0.0f};
    table.rot0 = {1.0f, 1.0f};
    table.rot1 = {0.0f, 0.0f};
    table.rot2 = {0.0f, 0.0f};
    table.rot3 = {0.0f, 0.0f};
    table.fdc0 = {0.0f, 0.0f};
    table.fdc1 = {0.0f, 0.0f};
    table.fdc2 = {0.0f, 0.0f};
    table.opacity = {0.0f, -5.0f};
    return table;
}

ga3d::TransformSelected translation(float x, float y, float z) {
    ga3d::TransformSelected op;
    op.matrix[12] = x;
    op.matrix[13] = y;
    op.matrix[14] = z;
    return op;
}

ga3d::TransformSelected uniform_scale(float scale) {
    ga3d::TransformSelected op;
    op.matrix[0] = scale;
    op.matrix[5] = scale;
    op.matrix[10] = scale;
    return op;
}

ga3d::TransformSelected rotate_z_90() {
    ga3d::TransformSelected op;
    op.matrix = {
        0.0f, 1.0f, 0.0f, 0.0f,
        -1.0f, 0.0f, 0.0f, 0.0f,
        0.0f, 0.0f, 1.0f, 0.0f,
        0.0f, 0.0f, 0.0f, 1.0f
    };
    return op;
}

void delete_selected_removes_rows() {
    auto table = make_two_splat_table();
    ga3d::EditRecipe recipe;
    recipe.ops.push_back(ga3d::SelectAll{});
    recipe.ops.push_back(ga3d::DeleteSelected{});
    const auto baked = ga3d::bake_edits(table, recipe);
    require(baked.size() == 0, "selectAll + deleteSelected should remove all rows");
}

void box_delete_removes_only_hits() {
    auto table = make_two_splat_table();
    ga3d::EditRecipe recipe;
    recipe.ops.push_back(ga3d::SelectBox{ga3d::SelectMode::Set, {0.0f, 0.0f, 0.0f}, {1.0f, 1.0f, 1.0f}});
    recipe.ops.push_back(ga3d::DeleteSelected{});
    const auto baked = ga3d::bake_edits(table, recipe);
    require(baked.size() == 1, "selectBox + deleteSelected should keep one row");
    require_close(baked.x[0], 2.0f, 1e-6f, "remaining row x mismatch");
}

void identity_transform_is_noop() {
    auto table = make_two_splat_table();
    ga3d::EditRecipe recipe;
    recipe.ops.push_back(ga3d::SelectAll{});
    recipe.ops.push_back(ga3d::TransformSelected{});
    const auto baked = ga3d::bake_edits(table, recipe);
    require(baked.size() == table.size(), "identity transform should keep row count");
    require_close(baked.x[1], table.x[1], 1e-6f, "identity transform changed x");
    require_close(baked.scale0[0], table.scale0[0], 1e-6f, "identity transform changed scale");
}

void translation_changes_selected_only() {
    auto table = make_two_splat_table();
    ga3d::EditRecipe recipe;
    recipe.ops.push_back(ga3d::SelectBox{ga3d::SelectMode::Set, {0.0f, 0.0f, 0.0f}, {1.0f, 1.0f, 1.0f}});
    recipe.ops.push_back(translation(5.0f, 0.0f, 0.0f));
    const auto baked = ga3d::bake_edits(table, recipe);
    require_close(baked.x[0], 5.0f, 1e-6f, "translation did not move selected row");
    require_close(baked.x[1], 2.0f, 1e-6f, "translation moved unselected row");
}

void uniform_scale_updates_log_scale() {
    auto table = make_two_splat_table();
    ga3d::EditRecipe recipe;
    recipe.ops.push_back(ga3d::SelectAll{});
    recipe.ops.push_back(uniform_scale(2.0f));
    const auto baked = ga3d::bake_edits(table, recipe);
    require_close(baked.scale0[0], std::log(2.0f), 1e-6f, "uniform scale did not update scale_0");
    require_close(baked.scale1[1], std::log(2.0f), 1e-6f, "uniform scale did not update scale_1");
}

void opacity_filter_removes_low_alpha() {
    auto table = make_two_splat_table();
    ga3d::EditRecipe recipe;
    recipe.ops.push_back(ga3d::FilterOpacity{0.1f});
    const auto baked = ga3d::bake_edits(table, recipe);
    require(baked.size() == 1, "filterOpacity should remove low-alpha row");
    require_close(baked.x[0], 0.0f, 1e-6f, "filterOpacity kept wrong row");
}

void rotation_updates_quaternion_and_sh_coefficients() {
    auto table = make_two_splat_table();
    table.sh_rest.assign(9, std::vector<float>(table.size(), 0.0f));
    table.sh_rest[2][0] = 1.0f;

    ga3d::EditRecipe recipe;
    recipe.ops.push_back(ga3d::SelectBox{ga3d::SelectMode::Set, {0.0f, 0.0f, 0.0f}, {1.0f, 1.0f, 1.0f}});
    recipe.ops.push_back(rotate_z_90());
    const auto baked = ga3d::bake_edits(table, recipe);

    require_close(baked.x[0], 0.0f, 1e-6f, "rotation moved origin incorrectly");
    require_close(baked.rot0[0], std::sqrt(0.5f), 1e-5f, "rotation did not update quaternion w");
    require_close(baked.rot3[0], std::sqrt(0.5f), 1e-5f, "rotation did not update quaternion z");
    require_close(baked.sh_rest[0][0], 1.0f, 0.04f, "rotation did not move red SH x basis into y basis");
    require_close(baked.sh_rest[2][0], 0.0f, 0.04f, "rotation left too much red SH x basis");
    require_close(baked.sh_rest[3][0], 0.0f, 1e-6f, "rotation changed green SH channel");
    require_close(baked.sh_rest[0][1], 0.0f, 1e-6f, "rotation changed unselected row SH");
}

} // namespace

int main() {
    delete_selected_removes_rows();
    box_delete_removes_only_hits();
    identity_transform_is_noop();
    translation_changes_selected_only();
    uniform_scale_updates_log_scale();
    opacity_filter_removes_low_alpha();
    rotation_updates_quaternion_and_sh_coefficients();
    return 0;
}
