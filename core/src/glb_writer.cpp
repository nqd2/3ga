#include "ga3d/glb_writer.hpp"

#include <algorithm>
#include <cstdint>
#include <fstream>
#include <iomanip>
#include <limits>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace ga3d {

namespace {

Bounds invalid_bounds() {
    const float inf = std::numeric_limits<float>::infinity();
    return {{inf, inf, inf}, {-inf, -inf, -inf}};
}

Bounds mesh_bounds(const Mesh& mesh) {
    if (mesh.positions.size() % 3 != 0) {
        throw std::runtime_error("mesh positions must be xyz triples");
    }
    Bounds bounds = invalid_bounds();
    for (std::size_t i = 0; i < mesh.positions.size(); i += 3) {
        bounds.expand(Vec3{mesh.positions[i], mesh.positions[i + 1], mesh.positions[i + 2]});
    }
    return bounds;
}

std::string json_escape(const std::string& value) {
    std::string out;
    out.reserve(value.size());
    for (const char ch : value) {
        if (ch == '"' || ch == '\\') {
            out.push_back('\\');
        }
        out.push_back(ch);
    }
    return out;
}

std::uint32_t align4(std::uint32_t value) {
    return (value + 3U) & ~3U;
}

} // namespace

void write_mesh_glb(const Mesh& mesh, const std::filesystem::path& path, const GlbWriteOptions& options) {
    if (mesh.positions.empty() || mesh.indices.empty()) {
        throw std::runtime_error("write_mesh_glb requires non-empty positions and indices");
    }

    const auto bounds = mesh_bounds(mesh);
    const auto position_bytes = static_cast<std::uint32_t>(mesh.positions.size() * sizeof(float));
    const auto index_offset = align4(position_bytes);
    const auto index_bytes = static_cast<std::uint32_t>(mesh.indices.size() * sizeof(std::uint32_t));
    const auto bin_length = align4(index_offset + index_bytes);
    const auto name = json_escape(options.name);
    const auto role = json_escape(options.role);

    std::ostringstream json;
    json << std::setprecision(9);
    json
        << "{\"asset\":{\"version\":\"2.0\",\"generator\":\"ga3d\"},"
        << "\"scene\":0,\"scenes\":[{\"nodes\":[0]}],"
        << "\"nodes\":[{\"mesh\":0,\"name\":\"" << name << "\"}],"
        << "\"materials\":[{\"name\":\"" << name << "\",\"extras\":{\"ga3dRole\":\"" << role
        << "\",\"visible\":" << (options.visible ? "true" : "false") << "},"
        << "\"pbrMetallicRoughness\":{\"baseColorFactor\":[1,1,1," << (options.visible ? "1" : "0")
        << "],\"metallicFactor\":0,\"roughnessFactor\":1}}],"
        << "\"meshes\":[{\"name\":\"" << name << "\",\"primitives\":[{\"attributes\":{\"POSITION\":0},\"indices\":1,\"material\":0}]}],"
        << "\"buffers\":[{\"byteLength\":" << bin_length << "}],"
        << "\"bufferViews\":["
        << "{\"buffer\":0,\"byteOffset\":0,\"byteLength\":" << position_bytes << ",\"target\":34962},"
        << "{\"buffer\":0,\"byteOffset\":" << index_offset << ",\"byteLength\":" << index_bytes << ",\"target\":34963}],"
        << "\"accessors\":["
        << "{\"bufferView\":0,\"componentType\":5126,\"count\":" << (mesh.positions.size() / 3)
        << ",\"type\":\"VEC3\",\"min\":[" << bounds.min.x << ',' << bounds.min.y << ',' << bounds.min.z
        << "],\"max\":[" << bounds.max.x << ',' << bounds.max.y << ',' << bounds.max.z << "]},"
        << "{\"bufferView\":1,\"componentType\":5125,\"count\":" << mesh.indices.size() << ",\"type\":\"SCALAR\"}],"
        << "\"extras\":{\"coordinateSystem\":{\"upAxis\":\"Y\",\"unit\":\"meter\"}}}";

    std::string json_chunk = json.str();
    while (json_chunk.size() % 4 != 0) {
        json_chunk.push_back(' ');
    }

    std::vector<char> bin(bin_length, 0);
    const auto* position_data = reinterpret_cast<const char*>(mesh.positions.data());
    std::copy(position_data, position_data + position_bytes, bin.begin());
    const auto* index_data = reinterpret_cast<const char*>(mesh.indices.data());
    std::copy(index_data, index_data + index_bytes, bin.begin() + index_offset);

    std::ofstream out(path, std::ios::binary);
    if (!out) {
        throw std::runtime_error("failed to open glb output");
    }

    auto write_u32 = [&](std::uint32_t value) {
        out.write(reinterpret_cast<const char*>(&value), sizeof(value));
    };

    const auto json_length = static_cast<std::uint32_t>(json_chunk.size());
    const std::uint32_t total_length = 12U + 8U + json_length + 8U + bin_length;
    write_u32(0x46546C67U);
    write_u32(2U);
    write_u32(total_length);
    write_u32(json_length);
    write_u32(0x4E4F534AU);
    out.write(json_chunk.data(), json_chunk.size());
    write_u32(bin_length);
    write_u32(0x004E4942U);
    out.write(bin.data(), bin.size());
}

void write_scene_glb(const Mesh& mesh, const std::filesystem::path& path) {
    write_mesh_glb(mesh, path, {.name = "GA3D_SCENE", .role = "scene", .visible = true});
}

void write_occlusion_glb(const Mesh& mesh, const std::filesystem::path& path) {
    write_mesh_glb(mesh, path, {.name = "GA3D_OCCLUSION", .role = "occlusion", .visible = false});
}

void write_navmesh_glb(const Mesh& mesh, const std::filesystem::path& path) {
    write_mesh_glb(mesh, path, {.name = "GA3D_NAVMESH", .role = "navmesh", .visible = true});
}

} // namespace ga3d
