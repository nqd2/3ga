#include "ga3d/rust_bindings.hpp"
#include "ga3d/data_table.hpp"
#include "ga3d/edit_recipe.hpp"
#include "ga3d/glb_writer.hpp"
#include "ga3d/navmesh.hpp"
#include "ga3d/readers.hpp"
#include "ga3d/voxel.hpp"

#include <chrono>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <stdexcept>
#include <iomanip>
#include <cmath>

namespace ga3d {

namespace {

std::string source_format_from_path(const std::filesystem::path& path) {
    std::string ext = path.extension().string();
    if (!ext.empty() && ext[0] == '.') {
        ext = ext.substr(1);
    }
    for (auto& c : ext) c = std::tolower(c);
    if (ext == "ply" || ext == "splat" || ext == "sog") {
        return ext;
    }
    return "ply"; // fallback
}

std::string escape_json_string(const std::string& s) {
    std::ostringstream o;
    for (auto c : s) {
        if (c == '"' || c == '\\') o << '\\' << c;
        else if (c == '\b') o << "\\b";
        else if (c == '\f') o << "\\f";
        else if (c == '\n') o << "\\n";
        else if (c == '\r') o << "\\r";
        else if (c == '\t') o << "\\t";
        else if (static_cast<unsigned char>(c) < 32) {
            o << "\\u" << std::hex << std::setw(4) << std::setfill('0') << static_cast<int>(c);
        } else {
            o << c;
        }
    }
    return o.str();
}

} // namespace

std::string run_pipeline_rust(
    const std::string& input_path,
    const std::string& output_dir,
    const std::vector<RustEditOp>& recipe_ops,
    const RustVoxelOptions& voxel_opts,
    const RustNavConfig& nav_cfg,
    const std::string& mesh_mode
) {
    auto start_time = std::chrono::high_resolution_clock::now();

    std::filesystem::path in_file(input_path);
    std::filesystem::path out_dir(output_dir);

    if (!std::filesystem::exists(in_file)) {
        throw std::runtime_error("input file does not exist: " + input_path);
    }

    // 1. Read input
    auto table = ga3d::read_file(in_file);
    table.validate();
    const std::size_t input_splat_count = table.size();

    // 2. Parse and bake edits
    EditRecipe recipe;
    for (const auto& op : recipe_ops) {
        if (op.op_type == "selectAll") {
            recipe.ops.emplace_back(SelectAll{});
        } else if (op.op_type == "selectNone") {
            recipe.ops.emplace_back(SelectNone{});
        } else if (op.op_type == "selectBox") {
            SelectMode mode = SelectMode::Set;
            if (op.box_mode == 1) mode = SelectMode::Add;
            else if (op.box_mode == 2) mode = SelectMode::Remove;
            recipe.ops.emplace_back(SelectBox{mode, op.center, op.size});
        } else if (op.op_type == "deleteSelected") {
            recipe.ops.emplace_back(DeleteSelected{});
        } else if (op.op_type == "transformSelected") {
            recipe.ops.emplace_back(TransformSelected{op.matrix});
        } else if (op.op_type == "filterOpacity") {
            recipe.ops.emplace_back(FilterOpacity{op.opacity_min});
        }
    }
    auto edited = ga3d::bake_edits(table, recipe);
    edited.validate();
    const std::size_t baked_splat_count = edited.size();

    // 3. Filter opacities
    auto filtered = ga3d::filter_opacity_min(ga3d::filter_nan(edited), voxel_opts.opacity_cutoff);
    filtered.validate();
    const std::size_t filtered_splat_count = filtered.size();

    // 4. Voxelize
    VoxelOptions v_opts;
    v_opts.voxel_size = voxel_opts.voxel_size;
    v_opts.opacity_cutoff = voxel_opts.opacity_cutoff;
    v_opts.sigma = voxel_opts.sigma;
    v_opts.align_to_blocks = voxel_opts.align_to_blocks;

    auto grid = ga3d::voxelize(filtered, v_opts);
    if (grid.occupied_count() == 0) {
        throw std::runtime_error("voxel grid is empty after filtering");
    }

    // 5. Extract occlusion mesh
    MeshMode m_mode = MeshMode::Faces;
    if (mesh_mode == "smooth") {
        m_mode = MeshMode::Smooth;
    }
    auto mesh = ga3d::extract_occlusion_mesh(grid, m_mode);
    if (mesh.positions.empty() || mesh.indices.empty()) {
        throw std::runtime_error("occlusion mesh is empty after processing");
    }

    // 6. Build navmesh
    NavConfig n_cfg;
    n_cfg.enabled = nav_cfg.enabled;
    n_cfg.seed = Vec3{nav_cfg.seed[0], nav_cfg.seed[1], nav_cfg.seed[2]};
    n_cfg.agent_height = nav_cfg.agent_height;
    n_cfg.agent_radius = nav_cfg.agent_radius;
    n_cfg.max_slope_degrees = nav_cfg.max_slope_degrees;
    n_cfg.cell_size = nav_cfg.cell_size;
    n_cfg.cell_height = nav_cfg.cell_height;

    auto navmesh = ga3d::build_navmesh(grid, n_cfg);
    auto navmesh_mesh = ga3d::navmesh_to_mesh(navmesh);
    if (navmesh_mesh.positions.empty() || navmesh_mesh.indices.empty()) {
        throw std::runtime_error("navmesh mesh is empty after processing");
    }

    // 7. Write outputs
    std::filesystem::create_directories(out_dir);
    ga3d::write_scene_glb(mesh, out_dir / "scene.glb");
    ga3d::write_occlusion_glb(mesh, out_dir / "occlusion.glb");
    ga3d::write_navmesh_glb(navmesh_mesh, out_dir / "navmesh.glb");
    ga3d::write_navmesh_json(navmesh, out_dir / "navmesh.json");

    // 8. Calculate manifest stats
    auto end_time = std::chrono::high_resolution_clock::now();
    double duration_ms = std::chrono::duration<double, std::milli>(end_time - start_time).count();

    // Bounds info
    Bounds mesh_bounds = mesh.bounds;
    float min_x = mesh_bounds.valid() ? mesh_bounds.min.x : 0.0f;
    float min_y = mesh_bounds.valid() ? mesh_bounds.min.y : 0.0f;
    float min_z = mesh_bounds.valid() ? mesh_bounds.min.z : 0.0f;
    float max_x = mesh_bounds.valid() ? mesh_bounds.max.x : 0.0f;
    float max_y = mesh_bounds.valid() ? mesh_bounds.max.y : 0.0f;
    float max_z = mesh_bounds.valid() ? mesh_bounds.max.z : 0.0f;

    // Write manifest.json
    std::ofstream manifest_file(out_dir / "manifest.json");
    if (!manifest_file.is_open()) {
        throw std::runtime_error("failed to open manifest json output");
    }

    manifest_file << std::setprecision(9);
    manifest_file << "{"
                  << "\"version\":1,"
                  << "\"source\":{"
                  << "\"format\":\"" << escape_json_string(source_format_from_path(in_file)) << "\","
                  << "\"splatCount\":" << input_splat_count
                  << "},"
                  << "\"coordinateSystem\":{\"upAxis\":\"Y\",\"unit\":\"meter\",\"scale\":1.0},"
                  << "\"bounds\":{"
                  << "\"min\":[" << min_x << "," << min_y << "," << min_z << "],"
                  << "\"max\":[" << max_x << "," << max_y << "," << max_z << "]"
                  << "},"
                  << "\"artifacts\":{"
                  << "\"scene\":\"scene.glb\","
                  << "\"occlusion\":\"occlusion.glb\","
                  << "\"navmesh\":\"navmesh.glb\","
                  << "\"navmeshJson\":\"navmesh.json\""
                  << "},"
                  << "\"metrics\":{"
                  << "\"durationMs\":" << duration_ms << ","
                  << "\"peakMemoryMb\":0.0,"
                  << "\"inputSplatCount\":" << input_splat_count << ","
                  << "\"bakedSplatCount\":" << baked_splat_count << ","
                  << "\"filteredSplatCount\":" << filtered_splat_count << ","
                  << "\"occupiedVoxelCount\":" << grid.occupied_count() << ","
                  << "\"occlusionTriangleCount\":" << (mesh.indices.size() / 3) << ","
                  << "\"navmeshTriangleCount\":" << (navmesh.indices.size() / 3)
                  << "}"
                  << "}";
    manifest_file.close();

    // 9. Format response metrics to return to Rust backend
    std::ostringstream ss;
    ss << std::setprecision(9);
    ss << "{"
       << "\"id\":\"desktop-job\","
       << "\"state\":\"done\","
       << "\"states\":[\"queued\",\"reading\",\"editing\",\"filtering\",\"voxelizing\",\"meshing\",\"navmesh\",\"exporting\",\"done\"],"
       << "\"artifacts\":{"
       << "\"scene\":\"" << escape_json_string((out_dir / "scene.glb").string()) << "\","
       << "\"occlusion\":\"" << escape_json_string((out_dir / "occlusion.glb").string()) << "\","
       << "\"navmesh\":\"" << escape_json_string((out_dir / "navmesh.glb").string()) << "\","
       << "\"navmeshJson\":\"" << escape_json_string((out_dir / "navmesh.json").string()) << "\","
       << "\"manifest\":\"" << escape_json_string((out_dir / "manifest.json").string()) << "\""
       << "}"
       << "}";

    return ss.str();
}

} // namespace ga3d
