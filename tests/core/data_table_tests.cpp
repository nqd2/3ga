#include "ga3d/data_table.hpp"

#include <cstdlib>
#include <iostream>
#include <stdexcept>
#include <string>

namespace {

void expect_throw_mismatched_columns() {
    ga3d::DataTable table;
    table.x = {0.0f};
    table.y = {0.0f, 1.0f};

    try {
        table.validate();
    } catch (const std::runtime_error& error) {
        const std::string message = error.what();
        if (message.find("Column 'y' has inconsistent number of rows") != std::string::npos) {
            return;
        }
        std::cerr << "Unexpected error: " << message << '\n';
        std::exit(1);
    }

    std::cerr << "Expected DataTable::validate() to throw for mismatched columns\n";
    std::exit(1);
}

void expect_valid_table_passes() {
    ga3d::DataTable table;
    table.x = {0.0f};
    table.y = {0.0f};
    table.z = {0.0f};
    table.scale0 = {0.0f};
    table.scale1 = {0.0f};
    table.scale2 = {0.0f};
    table.rot0 = {1.0f};
    table.rot1 = {0.0f};
    table.rot2 = {0.0f};
    table.rot3 = {0.0f};
    table.fdc0 = {0.0f};
    table.fdc1 = {0.0f};
    table.fdc2 = {0.0f};
    table.opacity = {0.0f};
    table.sh_rest = {{0.0f}};

    table.validate();
}

} // namespace

int main() {
    expect_throw_mismatched_columns();
    expect_valid_table_passes();
    return 0;
}
