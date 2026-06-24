#include "ga3d/readers.hpp"

#include <bit>
#include <cstdint>
#include <fstream>
#include <limits>
#include <set>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace ga3d {

namespace {

struct PlyProperty {
    std::string type;
    std::string name;
    std::size_t size = 0;
};

struct PlyElement {
    std::string name;
    std::size_t count = 0;
    std::vector<PlyProperty> properties;
};

std::string strip_cr(std::string line) {
    if (!line.empty() && line.back() == '\r') {
        line.pop_back();
    }
    return line;
}

std::size_t property_size(const std::string& type) {
    if (type == "char" || type == "uchar" || type == "int8" || type == "uint8") return 1;
    if (type == "short" || type == "ushort" || type == "int16" || type == "uint16") return 2;
    if (type == "int" || type == "uint" || type == "float" || type == "int32" || type == "uint32" || type == "float32") return 4;
    if (type == "double" || type == "float64") return 8;
    throw std::runtime_error("Unsupported PLY property type: " + type);
}

std::uint16_t read_u16_le(const std::uint8_t* bytes) {
    return (std::uint16_t(bytes[0]) << 0) |
           (std::uint16_t(bytes[1]) << 8);
}

std::uint32_t read_u32_le(const std::uint8_t* bytes) {
    return (std::uint32_t(bytes[0]) << 0) |
           (std::uint32_t(bytes[1]) << 8) |
           (std::uint32_t(bytes[2]) << 16) |
           (std::uint32_t(bytes[3]) << 24);
}

std::uint64_t read_u64_le(const std::uint8_t* bytes) {
    return (std::uint64_t(bytes[0]) << 0) |
           (std::uint64_t(bytes[1]) << 8) |
           (std::uint64_t(bytes[2]) << 16) |
           (std::uint64_t(bytes[3]) << 24) |
           (std::uint64_t(bytes[4]) << 32) |
           (std::uint64_t(bytes[5]) << 40) |
           (std::uint64_t(bytes[6]) << 48) |
           (std::uint64_t(bytes[7]) << 56);
}

float read_property_value(const PlyProperty& property, const std::uint8_t* bytes) {
    const auto& type = property.type;
    if (type == "char" || type == "int8") return float(static_cast<std::int8_t>(bytes[0]));
    if (type == "uchar" || type == "uint8") return float(bytes[0]);
    if (type == "short" || type == "int16") return float(static_cast<std::int16_t>(read_u16_le(bytes)));
    if (type == "ushort" || type == "uint16") return float(read_u16_le(bytes));
    if (type == "int" || type == "int32") return float(static_cast<std::int32_t>(read_u32_le(bytes)));
    if (type == "uint" || type == "uint32") return float(read_u32_le(bytes));
    if (type == "float" || type == "float32") return std::bit_cast<float>(read_u32_le(bytes));
    if (type == "double" || type == "float64") return float(std::bit_cast<double>(read_u64_le(bytes)));
    throw std::runtime_error("Unsupported PLY property type: " + type);
}

bool starts_with(const std::string& value, const std::string& prefix) {
    return value.rfind(prefix, 0) == 0;
}

std::size_t parse_index_suffix(const std::string& name, const std::string& prefix) {
    if (!starts_with(name, prefix)) {
        throw std::runtime_error("Invalid indexed property: " + name);
    }
    const auto suffix = name.substr(prefix.size());
    if (suffix.empty()) {
        throw std::runtime_error("Invalid indexed property: " + name);
    }
    std::size_t consumed = 0;
    const auto index = std::stoul(suffix, &consumed);
    if (consumed != suffix.size()) {
        throw std::runtime_error("Invalid indexed property: " + name);
    }
    return index;
}

void reserve_known_columns(DataTable& table, std::size_t count, const std::set<std::string>& names) {
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

    for (const auto& name : names) {
        if (starts_with(name, "f_rest_")) {
            const auto index = parse_index_suffix(name, "f_rest_");
            if (table.sh_rest.size() <= index) {
                table.sh_rest.resize(index + 1);
            }
            table.sh_rest[index].reserve(count);
        }
    }
}

void append_property(DataTable& table, const std::string& name, float value) {
    if (name == "x") table.x.push_back(value);
    else if (name == "y") table.y.push_back(value);
    else if (name == "z") table.z.push_back(value);
    else if (name == "scale_0") table.scale0.push_back(value);
    else if (name == "scale_1") table.scale1.push_back(value);
    else if (name == "scale_2") table.scale2.push_back(value);
    else if (name == "rot_0") table.rot0.push_back(value);
    else if (name == "rot_1") table.rot1.push_back(value);
    else if (name == "rot_2") table.rot2.push_back(value);
    else if (name == "rot_3") table.rot3.push_back(value);
    else if (name == "f_dc_0") table.fdc0.push_back(value);
    else if (name == "f_dc_1") table.fdc1.push_back(value);
    else if (name == "f_dc_2") table.fdc2.push_back(value);
    else if (name == "opacity") table.opacity.push_back(value);
    else if (starts_with(name, "f_rest_")) {
        const auto index = parse_index_suffix(name, "f_rest_");
        if (table.sh_rest.size() <= index) {
            table.sh_rest.resize(index + 1);
        }
        table.sh_rest[index].push_back(value);
    }
}

void require_vertex_columns(const std::set<std::string>& names) {
    static const char* required[] = {
        "x", "y", "z",
        "scale_0", "scale_1", "scale_2",
        "rot_0", "rot_1", "rot_2", "rot_3",
        "f_dc_0", "f_dc_1", "f_dc_2",
        "opacity"
    };
    for (const auto* name : required) {
        if (!names.contains(name)) {
            throw std::runtime_error(std::string("PLY vertex element missing required property: ") + name);
        }
    }
}

std::size_t row_size(const PlyElement& element) {
    std::size_t size = 0;
    for (const auto& property : element.properties) {
        size += property.size;
    }
    return size;
}

} // namespace

DataTable read_ply(const std::filesystem::path& path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        throw std::runtime_error("Unable to open file: " + path.string());
    }

    std::string line;
    if (!std::getline(file, line) || strip_cr(line) != "ply") {
        throw std::runtime_error("Invalid PLY header: missing ply magic");
    }

    bool format_ok = false;
    bool saw_end_header = false;
    std::vector<PlyElement> elements;
    PlyElement* current = nullptr;

    while (std::getline(file, line)) {
        line = strip_cr(line);
        if (line == "end_header") {
            saw_end_header = true;
            break;
        }

        std::istringstream input(line);
        std::string tag;
        input >> tag;
        if (tag.empty() || tag == "comment" || tag == "obj_info") {
            continue;
        }
        if (tag == "format") {
            std::string format;
            input >> format;
            if (format != "binary_little_endian") {
                throw std::runtime_error("Unsupported PLY format: " + format);
            }
            format_ok = true;
            continue;
        }
        if (tag == "element") {
            std::string name;
            std::size_t count = 0;
            input >> name >> count;
            if (name.empty() || !input) {
                throw std::runtime_error("Invalid PLY element header");
            }
            elements.push_back(PlyElement{ name, count, {} });
            current = &elements.back();
            continue;
        }
        if (tag == "property") {
            if (!current) {
                throw std::runtime_error("PLY property declared before element");
            }
            std::string type;
            std::string name;
            input >> type;
            if (type == "list") {
                throw std::runtime_error("PLY list properties are not supported");
            }
            input >> name;
            if (name.empty() || !input) {
                throw std::runtime_error("Invalid PLY property header");
            }
            current->properties.push_back(PlyProperty{ type, name, property_size(type) });
            continue;
        }

        throw std::runtime_error("Unrecognized PLY header tag: " + tag);
    }

    if (!format_ok) {
        throw std::runtime_error("Invalid PLY header: missing binary_little_endian format");
    }
    if (!saw_end_header) {
        throw std::runtime_error("Invalid PLY header: missing end_header");
    }

    DataTable table;
    bool saw_vertex = false;
    constexpr std::size_t kChunkRows = 4096;

    for (const auto& element : elements) {
        const auto bytes_per_row = row_size(element);
        if (bytes_per_row == 0 && element.count > 0) {
            throw std::runtime_error("PLY element has rows but no properties: " + element.name);
        }

        std::set<std::string> property_names;
        if (element.name == "vertex") {
            saw_vertex = true;
            if (element.count == 0) {
                throw std::runtime_error("PLY vertex element is empty");
            }
            for (const auto& property : element.properties) {
                property_names.insert(property.name);
            }
            require_vertex_columns(property_names);
            reserve_known_columns(table, element.count, property_names);
        }

        std::vector<std::uint8_t> buffer;
        for (std::size_t consumed = 0; consumed < element.count;) {
            const auto rows = std::min(kChunkRows, element.count - consumed);
            buffer.resize(rows * bytes_per_row);
            if (!buffer.empty()) {
                file.read(reinterpret_cast<char*>(buffer.data()), static_cast<std::streamsize>(buffer.size()));
                if (file.gcount() != static_cast<std::streamsize>(buffer.size())) {
                    throw std::runtime_error("Unexpected EOF while reading PLY element: " + element.name);
                }
            }

            if (element.name == "vertex") {
                for (std::size_t row = 0; row < rows; ++row) {
                    const auto* cursor = buffer.data() + row * bytes_per_row;
                    for (const auto& property : element.properties) {
                        append_property(table, property.name, read_property_value(property, cursor));
                        cursor += property.size;
                    }
                }
            }
            consumed += rows;
        }
    }

    if (!saw_vertex) {
        throw std::runtime_error("PLY file does not contain a vertex element");
    }

    table.validate();
    return table;
}

} // namespace ga3d
