#include "ga3d/data_table.hpp"
#include "ga3d/edit_recipe.hpp"
#include "ga3d/glb_writer.hpp"
#include "ga3d/navmesh.hpp"
#include "ga3d/readers.hpp"
#include "ga3d/voxel.hpp"

#include <pybind11/pybind11.h>
#include <pybind11/stl/filesystem.h>
#include <pybind11/stl.h>

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <filesystem>
#include <limits>
#include <stdexcept>
#include <string>

namespace py = pybind11;

namespace {

ga3d::Bounds invalid_bounds() {
    const float inf = std::numeric_limits<float>::infinity();
    return {{inf, inf, inf}, {-inf, -inf, -inf}};
}

py::list vec3_to_list(const ga3d::Vec3& value) {
    py::list out;
    out.append(value.x);
    out.append(value.y);
    out.append(value.z);
    return out;
}

py::dict bounds_to_dict(const ga3d::Bounds& bounds) {
    py::dict out;
    if (bounds.valid()) {
        out["min"] = vec3_to_list(bounds.min);
        out["max"] = vec3_to_list(bounds.max);
    } else {
        out["min"] = py::make_tuple(0.0f, 0.0f, 0.0f);
        out["max"] = py::make_tuple(0.0f, 0.0f, 0.0f);
    }
    return out;
}

ga3d::Vec3 vec3_from_object(const py::handle value, const ga3d::Vec3& default_value) {
    if (value.is_none()) {
        return default_value;
    }
    const auto seq = py::cast<py::sequence>(value);
    if (seq.size() != 3) {
        throw std::runtime_error("expected vec3 sequence");
    }
    return {
        seq[0].cast<float>(),
        seq[1].cast<float>(),
        seq[2].cast<float>(),
    };
}

ga3d::Bounds table_bounds(const ga3d::DataTable& table) {
    ga3d::Bounds bounds = invalid_bounds();
    for (std::size_t i = 0; i < table.size(); ++i) {
        bounds.expand(ga3d::Vec3{table.x[i], table.y[i], table.z[i]});
    }
    return bounds;
}

py::dict table_summary(const ga3d::DataTable& table) {
    table.validate();
    py::dict out;
    out["splatCount"] = table.size();
    out["bounds"] = bounds_to_dict(table_bounds(table));
    out["shRestColumnCount"] = table.sh_rest.size();
    return out;
}

py::dict grid_summary(const ga3d::VoxelGrid& grid) {
    py::dict out;
    out["origin"] = vec3_to_list(grid.origin);
    out["voxelSize"] = grid.voxel_size;
    out["dimensions"] = py::make_tuple(grid.nx, grid.ny, grid.nz);
    out["occupiedVoxelCount"] = grid.occupied_count();
    out["bounds"] = bounds_to_dict(grid.bounds());
    return out;
}

py::dict mesh_summary(const ga3d::Mesh& mesh) {
    py::dict out;
    out["vertexCount"] = mesh.positions.size() / 3;
    out["triangleCount"] = mesh.indices.size() / 3;
    out["bounds"] = bounds_to_dict(mesh.bounds);
    return out;
}

py::dict navmesh_summary(const ga3d::NavMesh& navmesh) {
    py::dict out;
    out["vertexCount"] = navmesh.vertices.size() / 3;
    out["triangleCount"] = navmesh.indices.size() / 3;
    out["bounds"] = bounds_to_dict(navmesh.bounds);
    out["grid"] = py::make_tuple(navmesh.width, navmesh.depth);
    return out;
}

std::string required_type(const py::dict& op) {
    if (!op.contains("type")) {
        throw std::runtime_error("edit operation missing type");
    }
    return op["type"].cast<std::string>();
}

ga3d::SelectMode parse_select_mode(const py::dict& op) {
    const auto mode = op.contains("mode") ? op["mode"].cast<std::string>() : std::string("set");
    if (mode == "set") {
        return ga3d::SelectMode::Set;
    }
    if (mode == "add") {
        return ga3d::SelectMode::Add;
    }
    if (mode == "remove") {
        return ga3d::SelectMode::Remove;
    }
    throw std::runtime_error("unsupported selectBox mode: " + mode);
}

std::array<float, 3> array3_from_object(const py::handle value) {
    const auto seq = py::cast<py::sequence>(value);
    if (seq.size() != 3) {
        throw std::runtime_error("expected 3 numeric values");
    }
    return {seq[0].cast<float>(), seq[1].cast<float>(), seq[2].cast<float>()};
}

std::array<float, 16> matrix_from_object(const py::handle value) {
    const auto seq = py::cast<py::sequence>(value);
    if (seq.size() != 16) {
        throw std::runtime_error("transformSelected matrix must contain 16 values");
    }
    std::array<float, 16> out{};
    for (std::size_t i = 0; i < out.size(); ++i) {
        out[i] = seq[i].cast<float>();
    }
    return out;
}

ga3d::EditRecipe parse_recipe(const py::object& recipe_obj) {
    ga3d::EditRecipe recipe;
    if (recipe_obj.is_none()) {
        return recipe;
    }

    py::object operations_obj;
    if (py::isinstance<py::dict>(recipe_obj)) {
        const auto recipe_dict = recipe_obj.cast<py::dict>();
        if (!recipe_dict.contains("operations")) {
            return recipe;
        }
        operations_obj = recipe_dict["operations"];
    } else {
        operations_obj = recipe_obj;
    }

    const auto operations = py::cast<py::sequence>(operations_obj);
    for (const auto item : operations) {
        const auto op = py::cast<py::dict>(item);
        const auto type = required_type(op);
        if (type == "selectAll") {
            recipe.ops.emplace_back(ga3d::SelectAll{});
        } else if (type == "selectNone") {
            recipe.ops.emplace_back(ga3d::SelectNone{});
        } else if (type == "selectBox") {
            if (!op.contains("center") || !op.contains("size")) {
                throw std::runtime_error("selectBox requires center and size");
            }
            recipe.ops.emplace_back(ga3d::SelectBox{
                parse_select_mode(op),
                array3_from_object(op["center"]),
                array3_from_object(op["size"]),
            });
        } else if (type == "deleteSelected") {
            recipe.ops.emplace_back(ga3d::DeleteSelected{});
        } else if (type == "transformSelected") {
            if (!op.contains("matrix")) {
                throw std::runtime_error("transformSelected requires matrix");
            }
            recipe.ops.emplace_back(ga3d::TransformSelected{matrix_from_object(op["matrix"])});
        } else if (type == "filterOpacity") {
            if (!op.contains("min")) {
                throw std::runtime_error("filterOpacity requires min");
            }
            recipe.ops.emplace_back(ga3d::FilterOpacity{op["min"].cast<float>()});
        } else {
            throw std::runtime_error("unsupported edit operation type: " + type);
        }
    }
    return recipe;
}

ga3d::VoxelOptions parse_voxel_options(const py::dict& config) {
    ga3d::VoxelOptions options;
    if (config.contains("voxelSize")) {
        options.voxel_size = config["voxelSize"].cast<float>();
    }
    if (config.contains("voxel_size")) {
        options.voxel_size = config["voxel_size"].cast<float>();
    }
    if (config.contains("opacityCutoff")) {
        options.opacity_cutoff = config["opacityCutoff"].cast<float>();
    }
    if (config.contains("opacity_cutoff")) {
        options.opacity_cutoff = config["opacity_cutoff"].cast<float>();
    }
    if (config.contains("sigma")) {
        options.sigma = config["sigma"].cast<float>();
    }
    if (config.contains("alignToBlocks")) {
        options.align_to_blocks = config["alignToBlocks"].cast<bool>();
    }
    if (config.contains("align_to_blocks")) {
        options.align_to_blocks = config["align_to_blocks"].cast<bool>();
    }
    return options;
}

ga3d::NavConfig parse_nav_config(const py::dict& config) {
    ga3d::NavConfig nav;
    nav.agent_height = 0.2f;
    nav.agent_radius = 0.0f;
    nav.cell_height = 0.025f;
    py::dict source = config;
    if (config.contains("navmesh")) {
        source = config["navmesh"].cast<py::dict>();
    }
    if (source.contains("enabled")) {
        nav.enabled = source["enabled"].cast<bool>();
    }
    if (source.contains("seed")) {
        nav.seed = vec3_from_object(source["seed"], nav.seed);
    }
    if (source.contains("agentHeight")) {
        nav.agent_height = source["agentHeight"].cast<float>();
    }
    if (source.contains("agent_height")) {
        nav.agent_height = source["agent_height"].cast<float>();
    }
    if (source.contains("agentRadius")) {
        nav.agent_radius = source["agentRadius"].cast<float>();
    }
    if (source.contains("agent_radius")) {
        nav.agent_radius = source["agent_radius"].cast<float>();
    }
    if (source.contains("maxSlopeDegrees")) {
        nav.max_slope_degrees = source["maxSlopeDegrees"].cast<float>();
    }
    if (source.contains("max_slope_degrees")) {
        nav.max_slope_degrees = source["max_slope_degrees"].cast<float>();
    }
    if (source.contains("cellSize")) {
        nav.cell_size = source["cellSize"].cast<float>();
    }
    if (source.contains("cell_size")) {
        nav.cell_size = source["cell_size"].cast<float>();
    }
    if (source.contains("cellHeight")) {
        nav.cell_height = source["cellHeight"].cast<float>();
    }
    if (source.contains("cell_height")) {
        nav.cell_height = source["cell_height"].cast<float>();
    }
    return nav;
}

ga3d::MeshMode parse_mesh_mode(const py::dict& config) {
    const auto mode = config.contains("meshMode")
        ? config["meshMode"].cast<std::string>()
        : config.contains("mesh_mode")
            ? config["mesh_mode"].cast<std::string>()
            : std::string("faces");
    if (mode == "faces") {
        return ga3d::MeshMode::Faces;
    }
    if (mode == "smooth") {
        return ga3d::MeshMode::Smooth;
    }
    throw std::runtime_error("unsupported mesh mode: " + mode);
}

void ensure_mesh(const ga3d::Mesh& mesh, const std::string& label) {
    if (mesh.positions.empty() || mesh.indices.empty()) {
        throw std::runtime_error(label + " mesh is empty after processing");
    }
}

ga3d::DataTable make_synthetic_table(std::size_t count) {
    ga3d::DataTable table;
    table.x.reserve(count);
    table.y.reserve(count);
    table.z.reserve(count);
    table.scale0.reserve(count);
    table.scale1.reserve(count);
    table.scale2.reserve(count);
    table.rot0.reserve(count);
    table.rot1.reserve(count);
    table.rot2.reserve(count);
    table.rot3.reserve(count);
    table.fdc0.reserve(count);
    table.fdc1.reserve(count);
    table.fdc2.reserve(count);
    table.opacity.reserve(count);
    const int side = std::max(1, static_cast<int>(std::ceil(std::sqrt(static_cast<double>(count)))));
    for (std::size_t i = 0; i < count; ++i) {
        const int ix = static_cast<int>(i % static_cast<std::size_t>(side));
        const int iz = static_cast<int>(i / static_cast<std::size_t>(side));
        const bool obstacle = (ix > side / 3 && ix < side / 3 + 3 && iz > side / 3 && iz < side / 3 + 3);
        table.x.push_back((static_cast<float>(ix) - static_cast<float>(side) * 0.5f) * 0.08f);
        table.y.push_back(obstacle ? 0.18f : 0.0f);
        table.z.push_back((static_cast<float>(iz) - static_cast<float>(side) * 0.5f) * 0.08f);
        table.scale0.push_back(std::log(0.055f));
        table.scale1.push_back(std::log(obstacle ? 0.11f : 0.025f));
        table.scale2.push_back(std::log(0.055f));
        table.rot0.push_back(1.0f);
        table.rot1.push_back(0.0f);
        table.rot2.push_back(0.0f);
        table.rot3.push_back(0.0f);
        table.fdc0.push_back(0.0f);
        table.fdc1.push_back(0.0f);
        table.fdc2.push_back(0.0f);
        table.opacity.push_back(5.0f);
    }
    table.validate();
    return table;
}

py::dict run_pipeline(
    const std::filesystem::path& input_path,
    const std::filesystem::path& output_dir,
    const py::object& recipe_obj,
    const py::dict& config
) {
    auto table = ga3d::read_file(input_path);
    table.validate();
    const auto input_summary = table_summary(table);
    const auto recipe = parse_recipe(recipe_obj);
    auto edited = ga3d::bake_edits(table, recipe);
    edited.validate();
    const auto voxel_options = parse_voxel_options(config);
    auto filtered = ga3d::filter_opacity_min(ga3d::filter_nan(edited), voxel_options.opacity_cutoff);
    filtered.validate();
    auto grid = ga3d::voxelize(filtered, voxel_options);
    if (grid.occupied_count() == 0) {
        throw std::runtime_error("voxel grid is empty after filtering");
    }
    auto mesh = ga3d::extract_occlusion_mesh(grid, parse_mesh_mode(config));
    ensure_mesh(mesh, "occlusion");
    auto navmesh = ga3d::build_navmesh(grid, parse_nav_config(config));
    auto navmesh_mesh = ga3d::navmesh_to_mesh(navmesh);
    ensure_mesh(navmesh_mesh, "navmesh");

    std::filesystem::create_directories(output_dir);
    ga3d::write_scene_glb(mesh, output_dir / "scene.glb");
    ga3d::write_occlusion_glb(mesh, output_dir / "occlusion.glb");
    ga3d::write_navmesh_glb(navmesh_mesh, output_dir / "navmesh.glb");
    ga3d::write_navmesh_json(navmesh, output_dir / "navmesh.json");

    py::dict out;
    out["input"] = input_summary;
    out["edited"] = table_summary(edited);
    out["filtered"] = table_summary(filtered);
    out["grid"] = grid_summary(grid);
    out["occlusionMesh"] = mesh_summary(mesh);
    out["navmesh"] = navmesh_summary(navmesh);
    return out;
}

py::dict run_synthetic_pipeline(std::size_t splats, const std::filesystem::path& output_dir, const py::dict& config) {
    auto table = make_synthetic_table(splats);
    const auto voxel_options = parse_voxel_options(config);
    auto filtered = ga3d::filter_opacity_min(ga3d::filter_nan(table), voxel_options.opacity_cutoff);
    auto grid = ga3d::voxelize(filtered, voxel_options);
    if (grid.occupied_count() == 0) {
        throw std::runtime_error("synthetic voxel grid is empty");
    }
    auto mesh = ga3d::extract_occlusion_mesh(grid, parse_mesh_mode(config));
    ensure_mesh(mesh, "synthetic occlusion");
    auto navmesh = ga3d::build_navmesh(grid, parse_nav_config(config));
    auto navmesh_mesh = ga3d::navmesh_to_mesh(navmesh);
    ensure_mesh(navmesh_mesh, "synthetic navmesh");

    std::filesystem::create_directories(output_dir);
    ga3d::write_scene_glb(mesh, output_dir / "scene.glb");
    ga3d::write_occlusion_glb(mesh, output_dir / "occlusion.glb");
    ga3d::write_navmesh_glb(navmesh_mesh, output_dir / "navmesh.glb");
    ga3d::write_navmesh_json(navmesh, output_dir / "navmesh.json");

    py::dict out;
    out["input"] = table_summary(table);
    out["filtered"] = table_summary(filtered);
    out["grid"] = grid_summary(grid);
    out["occlusionMesh"] = mesh_summary(mesh);
    out["navmesh"] = navmesh_summary(navmesh);
    return out;
}

} // namespace

PYBIND11_MODULE(_ga3d_core, m) {
    m.doc() = "ga3d C++ core bindings";

    py::class_<ga3d::DataTable>(m, "DataTable")
        .def(py::init<>())
        .def("validate", &ga3d::DataTable::validate)
        .def_property_readonly("size", &ga3d::DataTable::size);

    py::class_<ga3d::VoxelGrid>(m, "VoxelGrid")
        .def_property_readonly("occupied_count", &ga3d::VoxelGrid::occupied_count);

    py::class_<ga3d::Mesh>(m, "Mesh");
    py::class_<ga3d::NavMesh>(m, "NavMesh");

    m.def("read_input", &ga3d::read_file, py::arg("path"));
    m.def("read_ply", &ga3d::read_ply, py::arg("path"));
    m.def("read_splat", &ga3d::read_splat, py::arg("path"));
    m.def("read_sog", &ga3d::read_sog, py::arg("path"));
    m.def("bake_edits", [](const ga3d::DataTable& table, const py::object& recipe) {
        return ga3d::bake_edits(table, parse_recipe(recipe));
    }, py::arg("table"), py::arg("recipe"));
    m.def("filter_nan", &ga3d::filter_nan, py::arg("table"));
    m.def("filter_opacity_min", &ga3d::filter_opacity_min, py::arg("table"), py::arg("alpha_min"));
    m.def("voxelize", [](const ga3d::DataTable& table, const py::dict& config) {
        return ga3d::voxelize(table, parse_voxel_options(config));
    }, py::arg("table"), py::arg("config") = py::dict());
    m.def("extract_occlusion_mesh", [](const ga3d::VoxelGrid& grid, const std::string& mode) {
        py::dict config;
        config["meshMode"] = mode;
        return ga3d::extract_occlusion_mesh(grid, parse_mesh_mode(config));
    }, py::arg("grid"), py::arg("mode") = "faces");
    m.def("build_navmesh", [](const ga3d::VoxelGrid& grid, const py::dict& config) {
        return ga3d::build_navmesh(grid, parse_nav_config(config));
    }, py::arg("grid"), py::arg("config") = py::dict());
    m.def("navmesh_to_mesh", &ga3d::navmesh_to_mesh, py::arg("navmesh"));
    m.def("write_scene_glb", &ga3d::write_scene_glb, py::arg("mesh"), py::arg("path"));
    m.def("write_occlusion_glb", &ga3d::write_occlusion_glb, py::arg("mesh"), py::arg("path"));
    m.def("write_navmesh_glb", [](const ga3d::Mesh& mesh, const std::filesystem::path& path) {
        ga3d::write_navmesh_glb(mesh, path);
    }, py::arg("mesh"), py::arg("path"));
    m.def("write_navmesh_json", &ga3d::write_navmesh_json, py::arg("navmesh"), py::arg("path"));
    m.def("navmesh_to_json", &ga3d::navmesh_to_json, py::arg("navmesh"));
    m.def("table_summary", &table_summary, py::arg("table"));
    m.def("grid_summary", &grid_summary, py::arg("grid"));
    m.def("mesh_summary", &mesh_summary, py::arg("mesh"));
    m.def("navmesh_summary", &navmesh_summary, py::arg("navmesh"));
    m.def("run_pipeline", &run_pipeline, py::arg("input_path"), py::arg("output_dir"), py::arg("recipe"), py::arg("config") = py::dict());
    m.def("run_synthetic_pipeline", &run_synthetic_pipeline, py::arg("splats"), py::arg("output_dir"), py::arg("config") = py::dict());
}
