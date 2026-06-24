#pragma once

#include "ga3d/data_table.hpp"

#include <filesystem>

namespace ga3d {

DataTable read_ply(const std::filesystem::path& path);
DataTable read_splat(const std::filesystem::path& path);
DataTable read_sog(const std::filesystem::path& path);
DataTable read_file(const std::filesystem::path& path);

} // namespace ga3d
