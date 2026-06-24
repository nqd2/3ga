#include "ga3d/data_table.hpp"

#include <stdexcept>
#include <string>

namespace ga3d {

namespace {

void require_length(const std::string& name, std::size_t expected, std::size_t actual) {
    if (actual != expected) {
        throw std::runtime_error(
            "Column '" + name + "' has inconsistent number of rows: expected " +
            std::to_string(expected) + ", got " + std::to_string(actual)
        );
    }
}

} // namespace

void DataTable::validate() const {
    const auto expected = size();

    require_length("y", expected, y.size());
    require_length("z", expected, z.size());
    require_length("scale_0", expected, scale0.size());
    require_length("scale_1", expected, scale1.size());
    require_length("scale_2", expected, scale2.size());
    require_length("rot_0", expected, rot0.size());
    require_length("rot_1", expected, rot1.size());
    require_length("rot_2", expected, rot2.size());
    require_length("rot_3", expected, rot3.size());
    require_length("f_dc_0", expected, fdc0.size());
    require_length("f_dc_1", expected, fdc1.size());
    require_length("f_dc_2", expected, fdc2.size());
    require_length("opacity", expected, opacity.size());

    for (std::size_t i = 0; i < sh_rest.size(); ++i) {
        require_length("f_rest_" + std::to_string(i), expected, sh_rest[i].size());
    }
}

} // namespace ga3d
