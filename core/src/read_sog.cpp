#include "ga3d/readers.hpp"

#include <webp/decode.h>
#include <zlib.h>

#include <algorithm>
#include <array>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <functional>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

namespace ga3d {

namespace {

struct Json {
    enum class Type { Null, Bool, Number, String, Array, Object };

    Type type = Type::Null;
    bool boolean = false;
    double number = 0.0;
    std::string string;
    std::vector<Json> array;
    std::map<std::string, Json> object;
};

class JsonParser {
public:
    explicit JsonParser(std::string text) : text_(std::move(text)) {}

    Json parse() {
        auto value = parse_value();
        skip_ws();
        if (pos_ != text_.size()) {
            throw std::runtime_error("Unexpected trailing JSON data");
        }
        return value;
    }

private:
    Json parse_value() {
        skip_ws();
        if (pos_ >= text_.size()) {
            throw std::runtime_error("Unexpected end of JSON");
        }
        const char ch = text_[pos_];
        if (ch == '{') return parse_object();
        if (ch == '[') return parse_array();
        if (ch == '"') {
            Json value;
            value.type = Json::Type::String;
            value.string = parse_string();
            return value;
        }
        if (ch == '-' || (ch >= '0' && ch <= '9')) return parse_number();
        if (match("true")) {
            Json value;
            value.type = Json::Type::Bool;
            value.boolean = true;
            return value;
        }
        if (match("false")) {
            Json value;
            value.type = Json::Type::Bool;
            value.boolean = false;
            return value;
        }
        if (match("null")) {
            return Json{};
        }
        throw std::runtime_error("Invalid JSON value");
    }

    Json parse_object() {
        expect('{');
        Json value;
        value.type = Json::Type::Object;
        skip_ws();
        if (peek('}')) {
            ++pos_;
            return value;
        }
        while (true) {
            skip_ws();
            const auto key = parse_string();
            skip_ws();
            expect(':');
            value.object.emplace(key, parse_value());
            skip_ws();
            if (peek('}')) {
                ++pos_;
                return value;
            }
            expect(',');
        }
    }

    Json parse_array() {
        expect('[');
        Json value;
        value.type = Json::Type::Array;
        skip_ws();
        if (peek(']')) {
            ++pos_;
            return value;
        }
        while (true) {
            value.array.push_back(parse_value());
            skip_ws();
            if (peek(']')) {
                ++pos_;
                return value;
            }
            expect(',');
        }
    }

    Json parse_number() {
        const char* begin = text_.c_str() + pos_;
        char* end = nullptr;
        const double number = std::strtod(begin, &end);
        if (end == begin) {
            throw std::runtime_error("Invalid JSON number");
        }
        pos_ += static_cast<std::size_t>(end - begin);
        Json value;
        value.type = Json::Type::Number;
        value.number = number;
        return value;
    }

    std::string parse_string() {
        expect('"');
        std::string out;
        while (pos_ < text_.size()) {
            const char ch = text_[pos_++];
            if (ch == '"') {
                return out;
            }
            if (ch != '\\') {
                out.push_back(ch);
                continue;
            }
            if (pos_ >= text_.size()) {
                throw std::runtime_error("Invalid JSON escape");
            }
            const char esc = text_[pos_++];
            switch (esc) {
                case '"': out.push_back('"'); break;
                case '\\': out.push_back('\\'); break;
                case '/': out.push_back('/'); break;
                case 'b': out.push_back('\b'); break;
                case 'f': out.push_back('\f'); break;
                case 'n': out.push_back('\n'); break;
                case 'r': out.push_back('\r'); break;
                case 't': out.push_back('\t'); break;
                case 'u': {
                    if (pos_ + 4 > text_.size()) {
                        throw std::runtime_error("Invalid JSON unicode escape");
                    }
                    pos_ += 4;
                    out.push_back('?');
                    break;
                }
                default:
                    throw std::runtime_error("Invalid JSON escape");
            }
        }
        throw std::runtime_error("Unterminated JSON string");
    }

    void skip_ws() {
        while (pos_ < text_.size()) {
            const char ch = text_[pos_];
            if (ch != ' ' && ch != '\n' && ch != '\r' && ch != '\t') {
                break;
            }
            ++pos_;
        }
    }

    bool peek(char expected) const {
        return pos_ < text_.size() && text_[pos_] == expected;
    }

    bool match(const char* text) {
        const std::string token(text);
        if (text_.compare(pos_, token.size(), token) == 0) {
            pos_ += token.size();
            return true;
        }
        return false;
    }

    void expect(char expected) {
        skip_ws();
        if (pos_ >= text_.size() || text_[pos_] != expected) {
            throw std::runtime_error(std::string("Expected JSON character: ") + expected);
        }
        ++pos_;
    }

    std::string text_;
    std::size_t pos_ = 0;
};

struct RgbaImage {
    std::vector<std::uint8_t> rgba;
    int width = 0;
    int height = 0;
};

using BlobLoader = std::function<std::vector<std::uint8_t>(const std::string&)>;

const Json& require_field(const Json& value, const std::string& name) {
    if (value.type != Json::Type::Object) {
        throw std::runtime_error("Expected JSON object for field: " + name);
    }
    const auto it = value.object.find(name);
    if (it == value.object.end()) {
        throw std::runtime_error("Missing SOG meta field: " + name);
    }
    return it->second;
}

const Json* optional_field(const Json& value, const std::string& name) {
    if (value.type != Json::Type::Object) {
        throw std::runtime_error("Expected JSON object for field: " + name);
    }
    const auto it = value.object.find(name);
    if (it == value.object.end() || it->second.type == Json::Type::Null) {
        return nullptr;
    }
    return &it->second;
}

double as_number(const Json& value, const std::string& name) {
    if (value.type != Json::Type::Number) {
        throw std::runtime_error("Expected numeric SOG meta field: " + name);
    }
    return value.number;
}

std::size_t as_size(const Json& value, const std::string& name) {
    const auto number = as_number(value, name);
    if (number < 0.0 || std::floor(number) != number) {
        throw std::runtime_error("Expected non-negative integer SOG meta field: " + name);
    }
    return static_cast<std::size_t>(number);
}

std::vector<float> number_array(const Json& value, const std::string& name) {
    if (value.type != Json::Type::Array) {
        throw std::runtime_error("Expected numeric array SOG meta field: " + name);
    }
    std::vector<float> out;
    out.reserve(value.array.size());
    for (const auto& item : value.array) {
        out.push_back(static_cast<float>(as_number(item, name)));
    }
    return out;
}

std::vector<std::string> string_array(const Json& value, const std::string& name) {
    if (value.type != Json::Type::Array) {
        throw std::runtime_error("Expected string array SOG meta field: " + name);
    }
    std::vector<std::string> out;
    out.reserve(value.array.size());
    for (const auto& item : value.array) {
        if (item.type != Json::Type::String) {
            throw std::runtime_error("Expected string inside SOG meta field: " + name);
        }
        out.push_back(item.string);
    }
    return out;
}

void require_count(const std::vector<float>& values, std::size_t count, const std::string& name) {
    if (values.size() < count) {
        throw std::runtime_error("SOG meta field has too few values: " + name);
    }
}

void require_count(const std::vector<std::string>& values, std::size_t count, const std::string& name) {
    if (values.size() < count) {
        throw std::runtime_error("SOG meta field has too few files: " + name);
    }
}

std::vector<std::uint8_t> read_binary(const std::filesystem::path& path) {
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

std::string bytes_to_string(const std::vector<std::uint8_t>& bytes) {
    return std::string(reinterpret_cast<const char*>(bytes.data()), bytes.size());
}

void require_range(const std::vector<std::uint8_t>& bytes, std::size_t offset, std::size_t size) {
    if (offset > bytes.size() || size > bytes.size() - offset) {
        throw std::runtime_error("Truncated ZIP payload");
    }
}

std::uint16_t le16(const std::vector<std::uint8_t>& bytes, std::size_t offset) {
    require_range(bytes, offset, 2);
    return (std::uint16_t(bytes[offset + 0]) << 0) |
           (std::uint16_t(bytes[offset + 1]) << 8);
}

std::uint32_t le32(const std::vector<std::uint8_t>& bytes, std::size_t offset) {
    require_range(bytes, offset, 4);
    return (std::uint32_t(bytes[offset + 0]) << 0) |
           (std::uint32_t(bytes[offset + 1]) << 8) |
           (std::uint32_t(bytes[offset + 2]) << 16) |
           (std::uint32_t(bytes[offset + 3]) << 24);
}

std::vector<std::uint8_t> inflate_raw(const std::vector<std::uint8_t>& bytes, std::size_t offset, std::size_t compressed_size, std::size_t uncompressed_size) {
    require_range(bytes, offset, compressed_size);
    std::vector<std::uint8_t> out(uncompressed_size);
    z_stream stream{};
    stream.next_in = const_cast<Bytef*>(reinterpret_cast<const Bytef*>(bytes.data() + offset));
    stream.avail_in = static_cast<uInt>(compressed_size);
    stream.next_out = reinterpret_cast<Bytef*>(out.data());
    stream.avail_out = static_cast<uInt>(out.size());

    int ret = inflateInit2(&stream, -MAX_WBITS);
    if (ret != Z_OK) {
        throw std::runtime_error("Failed to initialize ZIP inflater");
    }
    ret = inflate(&stream, Z_FINISH);
    const auto total_out = stream.total_out;
    inflateEnd(&stream);
    if (ret != Z_STREAM_END || total_out != uncompressed_size) {
        throw std::runtime_error("Failed to inflate ZIP entry");
    }
    return out;
}

std::map<std::string, std::vector<std::uint8_t>> read_zip_entries(const std::filesystem::path& path) {
    const auto bytes = read_binary(path);
    if (bytes.size() < 22) {
        throw std::runtime_error("Invalid .sog ZIP: file too small");
    }

    std::size_t eocd = std::string::npos;
    const auto min_pos = bytes.size() > 65557 ? bytes.size() - 65557 : 0;
    for (std::size_t pos = bytes.size() - 22;; --pos) {
        if (le32(bytes, pos) == 0x06054b50u) {
            eocd = pos;
            break;
        }
        if (pos == min_pos) {
            break;
        }
    }
    if (eocd == std::string::npos) {
        throw std::runtime_error("Invalid .sog ZIP: missing central directory");
    }

    const auto entry_count = le16(bytes, eocd + 10);
    const auto central_size = le32(bytes, eocd + 12);
    const auto central_offset = le32(bytes, eocd + 16);
    require_range(bytes, central_offset, central_size);

    std::map<std::string, std::vector<std::uint8_t>> entries;
    std::size_t cursor = central_offset;
    for (std::size_t entry = 0; entry < entry_count; ++entry) {
        if (le32(bytes, cursor) != 0x02014b50u) {
            throw std::runtime_error("Invalid .sog ZIP: malformed central header");
        }
        const auto flags = le16(bytes, cursor + 8);
        const auto method = le16(bytes, cursor + 10);
        const auto compressed_size = le32(bytes, cursor + 20);
        const auto uncompressed_size = le32(bytes, cursor + 24);
        const auto name_len = le16(bytes, cursor + 28);
        const auto extra_len = le16(bytes, cursor + 30);
        const auto comment_len = le16(bytes, cursor + 32);
        const auto local_offset = le32(bytes, cursor + 42);
        require_range(bytes, cursor + 46, name_len);
        const std::string name(reinterpret_cast<const char*>(bytes.data() + cursor + 46), name_len);

        if ((flags & 1u) != 0) {
            throw std::runtime_error("Encrypted .sog ZIP entries are not supported");
        }
        if (compressed_size == 0xffffffffu || uncompressed_size == 0xffffffffu) {
            throw std::runtime_error("Zip64 .sog entries are not supported");
        }
        if (le32(bytes, local_offset) != 0x04034b50u) {
            throw std::runtime_error("Invalid .sog ZIP: malformed local header");
        }
        const auto local_name_len = le16(bytes, local_offset + 26);
        const auto local_extra_len = le16(bytes, local_offset + 28);
        const auto data_offset = local_offset + 30 + local_name_len + local_extra_len;

        if (!name.empty() && name.back() != '/') {
            if (method == 0) {
                require_range(bytes, data_offset, compressed_size);
                entries[name] = std::vector<std::uint8_t>(bytes.begin() + static_cast<std::ptrdiff_t>(data_offset), bytes.begin() + static_cast<std::ptrdiff_t>(data_offset + compressed_size));
            } else if (method == 8) {
                entries[name] = inflate_raw(bytes, data_offset, compressed_size, uncompressed_size);
            } else {
                throw std::runtime_error("Unsupported .sog ZIP compression method: " + std::to_string(method));
            }
        }

        cursor += 46 + name_len + extra_len + comment_len;
    }

    return entries;
}

RgbaImage decode_webp_rgba(const std::vector<std::uint8_t>& bytes) {
    int width = 0;
    int height = 0;
    if (!WebPGetInfo(bytes.data(), bytes.size(), &width, &height) || width <= 0 || height <= 0) {
        throw std::runtime_error("Invalid WebP payload in SOG texture");
    }
    std::uint8_t* raw = WebPDecodeRGBA(bytes.data(), bytes.size(), &width, &height);
    if (!raw) {
        throw std::runtime_error("Failed to decode WebP payload in SOG texture");
    }
    const auto size = static_cast<std::size_t>(width) * static_cast<std::size_t>(height) * 4;
    RgbaImage image;
    image.width = width;
    image.height = height;
    image.rgba.assign(raw, raw + size);
    WebPFree(raw);
    return image;
}

void require_pixels(const RgbaImage& image, std::size_t count, const std::string& label) {
    if (static_cast<std::size_t>(image.width) * static_cast<std::size_t>(image.height) < count) {
        throw std::runtime_error("SOG " + label + " texture too small for count");
    }
}

float inv_log_transform(float value) {
    const float magnitude = std::exp(std::abs(value)) - 1.0f;
    return value < 0.0f ? -magnitude : magnitude;
}

float sigmoid_inv(float y) {
    y = std::clamp(y, 1e-6f, 1.0f - 1e-6f);
    return std::log(y / (1.0f - y));
}

std::array<float, 4> unpack_quat(std::uint8_t px, std::uint8_t py, std::uint8_t pz, std::uint8_t tag) {
    if (tag < 252 || tag > 255) {
        return { 1.0f, 0.0f, 0.0f, 0.0f };
    }
    const int max_comp = int(tag) - 252;
    const float a = (float(px) / 255.0f) * 2.0f - 1.0f;
    const float b = (float(py) / 255.0f) * 2.0f - 1.0f;
    const float c = (float(pz) / 255.0f) * 2.0f - 1.0f;
    const float inv_sqrt2 = 1.0f / std::sqrt(2.0f);
    std::array<float, 4> comps{ 0.0f, 0.0f, 0.0f, 0.0f };
    static constexpr int indices[4][3] = {
        { 1, 2, 3 },
        { 0, 2, 3 },
        { 0, 1, 3 },
        { 0, 1, 2 }
    };
    comps[indices[max_comp][0]] = a * inv_sqrt2;
    comps[indices[max_comp][1]] = b * inv_sqrt2;
    comps[indices[max_comp][2]] = c * inv_sqrt2;
    const float known = comps[0] * comps[0] + comps[1] * comps[1] + comps[2] * comps[2] + comps[3] * comps[3];
    comps[max_comp] = std::sqrt(std::max(0.0f, 1.0f - known));
    return comps;
}

void resize_base_columns(DataTable& table, std::size_t count) {
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
}

float codebook_value(const std::vector<float>& codebook, std::uint8_t index, const std::string& label) {
    if (index >= codebook.size()) {
        throw std::runtime_error("SOG " + label + " codebook index out of range");
    }
    return codebook[index];
}

void decode_means(DataTable& table, const RgbaImage& lo, const RgbaImage& hi, const std::vector<float>& mins, const std::vector<float>& maxs, std::size_t count) {
    require_pixels(lo, count, "means low");
    require_pixels(hi, count, "means high");
    require_count(mins, 3, "means.mins");
    require_count(maxs, 3, "means.maxs");

    const float x_scale = (maxs[0] - mins[0]) == 0.0f ? 1.0f : (maxs[0] - mins[0]);
    const float y_scale = (maxs[1] - mins[1]) == 0.0f ? 1.0f : (maxs[1] - mins[1]);
    const float z_scale = (maxs[2] - mins[2]) == 0.0f ? 1.0f : (maxs[2] - mins[2]);

    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = i * 4;
        const auto x = std::uint16_t(lo.rgba[offset + 0] | (hi.rgba[offset + 0] << 8));
        const auto y = std::uint16_t(lo.rgba[offset + 1] | (hi.rgba[offset + 1] << 8));
        const auto z = std::uint16_t(lo.rgba[offset + 2] | (hi.rgba[offset + 2] << 8));
        table.x[i] = inv_log_transform(mins[0] + x_scale * (float(x) / 65535.0f));
        table.y[i] = inv_log_transform(mins[1] + y_scale * (float(y) / 65535.0f));
        table.z[i] = inv_log_transform(mins[2] + z_scale * (float(z) / 65535.0f));
    }
}

void decode_quats(DataTable& table, const RgbaImage& image, std::size_t count) {
    require_pixels(image, count, "quats");
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = i * 4;
        const auto quat = unpack_quat(image.rgba[offset + 0], image.rgba[offset + 1], image.rgba[offset + 2], image.rgba[offset + 3]);
        table.rot0[i] = quat[0];
        table.rot1[i] = quat[1];
        table.rot2[i] = quat[2];
        table.rot3[i] = quat[3];
    }
}

DataTable decode_sog_v2(const Json& meta, const BlobLoader& load) {
    const auto count = as_size(require_field(meta, "count"), "count");
    DataTable table;
    resize_base_columns(table, count);

    const auto& means = require_field(meta, "means");
    const auto means_mins = number_array(require_field(means, "mins"), "means.mins");
    const auto means_maxs = number_array(require_field(means, "maxs"), "means.maxs");
    const auto means_files = string_array(require_field(means, "files"), "means.files");
    require_count(means_files, 2, "means.files");
    decode_means(table, decode_webp_rgba(load(means_files[0])), decode_webp_rgba(load(means_files[1])), means_mins, means_maxs, count);

    const auto& quats = require_field(meta, "quats");
    const auto quats_files = string_array(require_field(quats, "files"), "quats.files");
    require_count(quats_files, 1, "quats.files");
    decode_quats(table, decode_webp_rgba(load(quats_files[0])), count);

    const auto& scales = require_field(meta, "scales");
    const auto scales_codebook = number_array(require_field(scales, "codebook"), "scales.codebook");
    const auto scales_files = string_array(require_field(scales, "files"), "scales.files");
    require_count(scales_files, 1, "scales.files");
    const auto scales_image = decode_webp_rgba(load(scales_files[0]));
    require_pixels(scales_image, count, "scales");
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = i * 4;
        table.scale0[i] = codebook_value(scales_codebook, scales_image.rgba[offset + 0], "scales");
        table.scale1[i] = codebook_value(scales_codebook, scales_image.rgba[offset + 1], "scales");
        table.scale2[i] = codebook_value(scales_codebook, scales_image.rgba[offset + 2], "scales");
    }

    const auto& sh0 = require_field(meta, "sh0");
    const auto sh0_codebook = number_array(require_field(sh0, "codebook"), "sh0.codebook");
    const auto sh0_files = string_array(require_field(sh0, "files"), "sh0.files");
    require_count(sh0_files, 1, "sh0.files");
    const auto sh0_image = decode_webp_rgba(load(sh0_files[0]));
    require_pixels(sh0_image, count, "sh0");
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = i * 4;
        table.fdc0[i] = codebook_value(sh0_codebook, sh0_image.rgba[offset + 0], "sh0");
        table.fdc1[i] = codebook_value(sh0_codebook, sh0_image.rgba[offset + 1], "sh0");
        table.fdc2[i] = codebook_value(sh0_codebook, sh0_image.rgba[offset + 2], "sh0");
        table.opacity[i] = sigmoid_inv(float(sh0_image.rgba[offset + 3]) / 255.0f);
    }

    if (const auto* shn = optional_field(meta, "shN")) {
        const auto bands = as_size(require_field(*shn, "bands"), "shN.bands");
        const auto palette_count = as_size(require_field(*shn, "count"), "shN.count");
        const int sh_coeffs = bands == 1 ? 3 : bands == 2 ? 8 : bands == 3 ? 15 : 0;
        if (sh_coeffs == 0) {
            throw std::runtime_error("Unsupported SOG shN band count");
        }
        const auto codebook = number_array(require_field(*shn, "codebook"), "shN.codebook");
        const auto files = string_array(require_field(*shn, "files"), "shN.files");
        require_count(files, 2, "shN.files");
        const auto centroids = decode_webp_rgba(load(files[0]));
        const auto labels = decode_webp_rgba(load(files[1]));
        require_pixels(labels, count, "shN labels");
        if (centroids.width != 64 * sh_coeffs) {
            throw std::runtime_error("SOG shN centroids texture width does not match band count");
        }
        if (static_cast<std::size_t>(centroids.height) * 64 < palette_count) {
            throw std::runtime_error("SOG shN centroids texture too small for palette count");
        }

        table.sh_rest.assign(static_cast<std::size_t>(sh_coeffs) * 3, std::vector<float>(count, 0.0f));
        for (std::size_t i = 0; i < count; ++i) {
            const auto label_offset = i * 4;
            const std::size_t label = labels.rgba[label_offset + 0] | (labels.rgba[label_offset + 1] << 8);
            if (label >= palette_count) {
                continue;
            }
            for (int coeff = 0; coeff < sh_coeffs; ++coeff) {
                const auto cx = (label % 64) * static_cast<std::size_t>(sh_coeffs) + static_cast<std::size_t>(coeff);
                const auto cy = label / 64;
                const auto pixel = (cy * static_cast<std::size_t>(centroids.width) + cx) * 4;
                table.sh_rest[coeff + sh_coeffs * 0][i] = codebook_value(codebook, centroids.rgba[pixel + 0], "shN");
                table.sh_rest[coeff + sh_coeffs * 1][i] = codebook_value(codebook, centroids.rgba[pixel + 1], "shN");
                table.sh_rest[coeff + sh_coeffs * 2][i] = codebook_value(codebook, centroids.rgba[pixel + 2], "shN");
            }
        }
    }

    table.validate();
    return table;
}

DataTable decode_sog_v1(const Json& meta, const BlobLoader& load) {
    const auto& means = require_field(meta, "means");
    const auto shape = number_array(require_field(means, "shape"), "means.shape");
    require_count(shape, 1, "means.shape");
    const auto count = static_cast<std::size_t>(shape[0]);

    DataTable table;
    resize_base_columns(table, count);

    const auto means_mins = number_array(require_field(means, "mins"), "means.mins");
    const auto means_maxs = number_array(require_field(means, "maxs"), "means.maxs");
    const auto means_files = string_array(require_field(means, "files"), "means.files");
    require_count(means_files, 2, "means.files");
    decode_means(table, decode_webp_rgba(load(means_files[0])), decode_webp_rgba(load(means_files[1])), means_mins, means_maxs, count);

    const auto& quats = require_field(meta, "quats");
    const auto quats_files = string_array(require_field(quats, "files"), "quats.files");
    require_count(quats_files, 1, "quats.files");
    decode_quats(table, decode_webp_rgba(load(quats_files[0])), count);

    const auto& scales = require_field(meta, "scales");
    const auto scales_mins = number_array(require_field(scales, "mins"), "scales.mins");
    const auto scales_maxs = number_array(require_field(scales, "maxs"), "scales.maxs");
    require_count(scales_mins, 3, "scales.mins");
    require_count(scales_maxs, 3, "scales.maxs");
    const auto scales_files = string_array(require_field(scales, "files"), "scales.files");
    require_count(scales_files, 1, "scales.files");
    const auto scales_image = decode_webp_rgba(load(scales_files[0]));
    require_pixels(scales_image, count, "scales");
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = i * 4;
        table.scale0[i] = scales_mins[0] + (scales_maxs[0] - scales_mins[0]) * (float(scales_image.rgba[offset + 0]) / 255.0f);
        table.scale1[i] = scales_mins[1] + (scales_maxs[1] - scales_mins[1]) * (float(scales_image.rgba[offset + 1]) / 255.0f);
        table.scale2[i] = scales_mins[2] + (scales_maxs[2] - scales_mins[2]) * (float(scales_image.rgba[offset + 2]) / 255.0f);
    }

    const auto& sh0 = require_field(meta, "sh0");
    const auto sh0_mins = number_array(require_field(sh0, "mins"), "sh0.mins");
    const auto sh0_maxs = number_array(require_field(sh0, "maxs"), "sh0.maxs");
    require_count(sh0_mins, 4, "sh0.mins");
    require_count(sh0_maxs, 4, "sh0.maxs");
    const auto sh0_files = string_array(require_field(sh0, "files"), "sh0.files");
    require_count(sh0_files, 1, "sh0.files");
    const auto sh0_image = decode_webp_rgba(load(sh0_files[0]));
    require_pixels(sh0_image, count, "sh0");
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = i * 4;
        table.fdc0[i] = sh0_mins[0] + (sh0_maxs[0] - sh0_mins[0]) * (float(sh0_image.rgba[offset + 0]) / 255.0f);
        table.fdc1[i] = sh0_mins[1] + (sh0_maxs[1] - sh0_mins[1]) * (float(sh0_image.rgba[offset + 1]) / 255.0f);
        table.fdc2[i] = sh0_mins[2] + (sh0_maxs[2] - sh0_mins[2]) * (float(sh0_image.rgba[offset + 2]) / 255.0f);
        table.opacity[i] = sh0_mins[3] + (sh0_maxs[3] - sh0_mins[3]) * (float(sh0_image.rgba[offset + 3]) / 255.0f);
    }

    if (const auto* shn = optional_field(meta, "shN")) {
        const auto files = string_array(require_field(*shn, "files"), "shN.files");
        require_count(files, 2, "shN.files");
        const auto centroids = decode_webp_rgba(load(files[0]));
        const auto labels = decode_webp_rgba(load(files[1]));
        require_pixels(labels, count, "shN labels");
        const int bands = centroids.width == 192 ? 1 : centroids.width == 512 ? 2 : centroids.width == 960 ? 3 : 0;
        const int sh_coeffs = bands == 1 ? 3 : bands == 2 ? 8 : bands == 3 ? 15 : 0;
        if (sh_coeffs == 0) {
            throw std::runtime_error("SOG shN centroids texture has unrecognized width");
        }
        const auto sh_min = static_cast<float>(as_number(require_field(*shn, "mins"), "shN.mins"));
        const auto sh_max = static_cast<float>(as_number(require_field(*shn, "maxs"), "shN.maxs"));
        const auto palette_count = static_cast<std::size_t>(centroids.width / sh_coeffs) * static_cast<std::size_t>(centroids.height);

        table.sh_rest.assign(static_cast<std::size_t>(sh_coeffs) * 3, std::vector<float>(count, 0.0f));
        for (std::size_t i = 0; i < count; ++i) {
            const auto label_offset = i * 4;
            const std::size_t label = labels.rgba[label_offset + 0] | (labels.rgba[label_offset + 1] << 8);
            if (label >= palette_count) {
                continue;
            }
            for (int coeff = 0; coeff < sh_coeffs; ++coeff) {
                const auto cx = (label % 64) * static_cast<std::size_t>(sh_coeffs) + static_cast<std::size_t>(coeff);
                const auto cy = label / 64;
                const auto pixel = (cy * static_cast<std::size_t>(centroids.width) + cx) * 4;
                const auto dequant = [&](std::uint8_t byte) {
                    return sh_min + (sh_max - sh_min) * (float(byte) / 255.0f);
                };
                table.sh_rest[coeff + sh_coeffs * 0][i] = dequant(centroids.rgba[pixel + 0]);
                table.sh_rest[coeff + sh_coeffs * 1][i] = dequant(centroids.rgba[pixel + 1]);
                table.sh_rest[coeff + sh_coeffs * 2][i] = dequant(centroids.rgba[pixel + 2]);
            }
        }
    }

    table.validate();
    return table;
}

std::string lower_string(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(), [](unsigned char ch) { return static_cast<char>(std::tolower(ch)); });
    return value;
}

} // namespace

DataTable read_sog(const std::filesystem::path& path) {
    std::vector<std::uint8_t> meta_bytes;
    BlobLoader load;

    const auto extension = lower_string(path.extension().string());
    if (extension == ".sog") {
        auto entries = std::make_shared<std::map<std::string, std::vector<std::uint8_t>>>(read_zip_entries(path));
        const auto meta_it = entries->find("meta.json");
        if (meta_it == entries->end()) {
            throw std::runtime_error("Bundled .sog is missing meta.json");
        }
        meta_bytes = meta_it->second;
        load = [entries](const std::string& name) {
            const auto it = entries->find(name);
            if (it == entries->end()) {
                throw std::runtime_error("Bundled .sog is missing texture: " + name);
            }
            return it->second;
        };
    } else {
        meta_bytes = read_binary(path);
        const auto base_dir = path.parent_path();
        load = [base_dir](const std::string& name) {
            return read_binary(base_dir / std::filesystem::path(name));
        };
    }

    const auto meta = JsonParser(bytes_to_string(meta_bytes)).parse();
    if (const auto* version = optional_field(meta, "version")) {
        const auto version_value = as_size(*version, "version");
        if (version_value != 2) {
            throw std::runtime_error("Unsupported SOG meta version: " + std::to_string(version_value));
        }
        return decode_sog_v2(meta, load);
    }
    return decode_sog_v1(meta, load);
}

DataTable read_file(const std::filesystem::path& path) {
    const auto extension = lower_string(path.extension().string());
    const auto filename = lower_string(path.filename().string());
    if (extension == ".ply") {
        return read_ply(path);
    }
    if (extension == ".splat") {
        return read_splat(path);
    }
    if (extension == ".sog" || filename == "meta.json") {
        return read_sog(path);
    }
    throw std::runtime_error("Unsupported input format: " + path.string());
}

} // namespace ga3d
