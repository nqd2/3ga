#pragma once

#include <cstddef>
#include <string>
#include <vector>

namespace ga3d {

struct DataTable {
    std::vector<float> x, y, z;
    std::vector<float> scale0, scale1, scale2;
    std::vector<float> rot0, rot1, rot2, rot3;
    std::vector<float> fdc0, fdc1, fdc2;
    std::vector<float> opacity;
    std::vector<std::vector<float>> sh_rest;

    std::size_t size() const {
        return x.size();
    }

    void validate() const;
};

} // namespace ga3d
