#include "ga3d/glb_writer.hpp"

#include <cstdint>
#include <cstdlib>
#include <fstream>
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

ga3d::Mesh make_triangle_mesh() {
    ga3d::Mesh mesh;
    mesh.positions = {
        0.0f, 0.0f, 0.0f,
        1.0f, 0.0f, 0.0f,
        0.0f, 0.0f, 1.0f
    };
    mesh.indices = {0, 1, 2};
    mesh.bounds = {{0.0f, 0.0f, 0.0f}, {1.0f, 0.0f, 1.0f}};
    return mesh;
}

std::string read_json_chunk(const std::string& path) {
    std::ifstream in(path, std::ios::binary);
    require(static_cast<bool>(in), "failed to open GLB test output");

    std::uint32_t magic = 0;
    std::uint32_t version = 0;
    std::uint32_t length = 0;
    std::uint32_t json_length = 0;
    std::uint32_t json_type = 0;
    in.read(reinterpret_cast<char*>(&magic), sizeof(magic));
    in.read(reinterpret_cast<char*>(&version), sizeof(version));
    in.read(reinterpret_cast<char*>(&length), sizeof(length));
    in.read(reinterpret_cast<char*>(&json_length), sizeof(json_length));
    in.read(reinterpret_cast<char*>(&json_type), sizeof(json_type));

    require(magic == 0x46546C67U, "GLB magic mismatch");
    require(version == 2U, "GLB version mismatch");
    require(length > json_length, "GLB total length mismatch");
    require(json_type == 0x4E4F534AU, "GLB JSON chunk type mismatch");

    std::vector<char> json(json_length);
    in.read(json.data(), json.size());
    return std::string(json.begin(), json.end());
}

void writes_scene_occlusion_and_navmesh_glbs() {
    const auto mesh = make_triangle_mesh();
    ga3d::write_scene_glb(mesh, "/tmp/ga3d_scene_test.glb");
    ga3d::write_occlusion_glb(mesh, "/tmp/ga3d_occlusion_test.glb");
    ga3d::write_navmesh_glb(mesh, "/tmp/ga3d_navmesh_mesh_test.glb");

    const auto scene_json = read_json_chunk("/tmp/ga3d_scene_test.glb");
    require(scene_json.find("GA3D_SCENE") != std::string::npos, "scene GLB missing scene name");

    const auto occlusion_json = read_json_chunk("/tmp/ga3d_occlusion_test.glb");
    require(occlusion_json.find("GA3D_OCCLUSION") != std::string::npos, "occlusion GLB missing name");
    require(occlusion_json.find("\"ga3dRole\":\"occlusion\"") != std::string::npos, "occlusion GLB missing role");
    require(occlusion_json.find("\"visible\":false") != std::string::npos, "occlusion GLB should be hidden");

    const auto navmesh_json = read_json_chunk("/tmp/ga3d_navmesh_mesh_test.glb");
    require(navmesh_json.find("GA3D_NAVMESH") != std::string::npos, "navmesh GLB missing navmesh name");
}

} // namespace

int main() {
    writes_scene_occlusion_and_navmesh_glbs();
    return 0;
}
