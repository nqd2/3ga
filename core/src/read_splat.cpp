#include "ga3d/readers.hpp"

#include <algorithm>
#include <bit>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace ga3d {

namespace {

constexpr std::size_t kBytesPerSplat = 32;
constexpr float kShC0 = 0.28209479177387814f;

std::vector<std::uint8_t> read_all_bytes(const std::filesystem::path& path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        throw std::runtime_error("Unable to open file: " + path.string());
    }

    file.seekg(0, std::ios::end);
    const auto size = file.tellg();
    if (size < 0) {
        throw std::runtime_error("Unable to determine file size: " + path.string());
    }
    file.seekg(0, std::ios::beg);

    std::vector<std::uint8_t> bytes(static_cast<std::size_t>(size));
    if (!bytes.empty()) {
        file.read(reinterpret_cast<char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
        if (!file) {
            throw std::runtime_error("Unable to read file: " + path.string());
        }
    }
    return bytes;
}

std::uint32_t read_u32_le(const std::uint8_t* bytes) {
    return (std::uint32_t(bytes[0]) << 0) |
           (std::uint32_t(bytes[1]) << 8) |
           (std::uint32_t(bytes[2]) << 16) |
           (std::uint32_t(bytes[3]) << 24);
}

float read_f32_le(const std::uint8_t* bytes) {
    const auto value = read_u32_le(bytes);
    return std::bit_cast<float>(value);
}

float logit(float p) {
    p = std::clamp(p, 1e-6f, 1.0f - 1e-6f);
    return std::log(p / (1.0f - p));
}

float dc_from_u8(std::uint8_t value) {
    return ((float(value) / 255.0f) - 0.5f) / kShC0;
}

} // namespace

DataTable read_splat(const std::filesystem::path& path) {
    const auto bytes = read_all_bytes(path);
    if (bytes.empty()) {
        throw std::runtime_error("Invalid .splat file: file is empty");
    }
    if (bytes.size() % kBytesPerSplat != 0) {
        throw std::runtime_error("Invalid .splat file: file size is not a multiple of 32 bytes");
    }

    const auto count = bytes.size() / kBytesPerSplat;
    DataTable table;
    table.x.resize(count);
    table.y.resize(count);
    table.z.resize(count);
    table.scale0.resize(count);
    table.scale1.resize(count);
    table.scale2.resize(count);
    table.fdc0.resize(count);
    table.fdc1.resize(count);
    table.fdc2.resize(count);
    table.opacity.resize(count);
    table.rot0.resize(count);
    table.rot1.resize(count);
    table.rot2.resize(count);
    table.rot3.resize(count);

    for (std::size_t i = 0; i < count; ++i) {
        const auto* row = bytes.data() + i * kBytesPerSplat;
        table.x[i] = read_f32_le(row + 0);
        table.y[i] = read_f32_le(row + 4);
        table.z[i] = read_f32_le(row + 8);
        table.scale0[i] = std::log(read_f32_le(row + 12));
        table.scale1[i] = std::log(read_f32_le(row + 16));
        table.scale2[i] = std::log(read_f32_le(row + 20));
        table.fdc0[i] = dc_from_u8(row[24]);
        table.fdc1[i] = dc_from_u8(row[25]);
        table.fdc2[i] = dc_from_u8(row[26]);
        table.opacity[i] = logit(float(row[27]) / 255.0f);

        const float r0 = (float(row[28]) / 255.0f) * 2.0f - 1.0f;
        const float r1 = (float(row[29]) / 255.0f) * 2.0f - 1.0f;
        const float r2 = (float(row[30]) / 255.0f) * 2.0f - 1.0f;
        const float r3 = (float(row[31]) / 255.0f) * 2.0f - 1.0f;
        const float length = std::sqrt(r0 * r0 + r1 * r1 + r2 * r2 + r3 * r3);
        if (length > 0.0f) {
            table.rot0[i] = r0 / length;
            table.rot1[i] = r1 / length;
            table.rot2[i] = r2 / length;
            table.rot3[i] = r3 / length;
        } else {
            table.rot0[i] = 1.0f;
            table.rot1[i] = 0.0f;
            table.rot2[i] = 0.0f;
            table.rot3[i] = 0.0f;
        }
    }

    table.validate();
    return table;
}

} // namespace ga3d
