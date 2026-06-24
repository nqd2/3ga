#include "ga3d/readers.hpp"

#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>

namespace {

void require(bool condition, const std::string& message) {
    if (!condition) {
        std::cerr << message << '\n';
        std::exit(1);
    }
}

void require_close(float actual, float expected, float tolerance, const std::string& message) {
    if (std::fabs(actual - expected) > tolerance) {
        std::cerr << message << ": expected " << expected << ", got " << actual << '\n';
        std::exit(1);
    }
}

template <typename Fn>
void expect_throw_contains(Fn&& fn, const std::string& expected) {
    try {
        fn();
    } catch (const std::runtime_error& error) {
        const std::string message = error.what();
        if (message.find(expected) != std::string::npos) {
            return;
        }
        std::cerr << "Unexpected error: " << message << '\n';
        std::exit(1);
    }
    std::cerr << "Expected exception containing: " << expected << '\n';
    std::exit(1);
}

void read_ply_minimal_fixture() {
    const auto table = ga3d::read_ply("tests/fixtures/minimal.ply");
    require(table.size() == 1, "minimal.ply row count mismatch");
    require(table.x.size() == table.opacity.size(), "minimal.ply column length mismatch");
    require_close(table.x[0], 1.0f, 1e-6f, "minimal.ply x mismatch");
    require_close(table.y[0], 2.0f, 1e-6f, "minimal.ply y mismatch");
    require_close(table.z[0], 3.0f, 1e-6f, "minimal.ply z mismatch");
    require_close(table.rot0[0], 1.0f, 1e-6f, "minimal.ply rot_0 mismatch");
}

void read_splat_minimal_fixture() {
    const auto table = ga3d::read_splat("tests/fixtures/minimal.splat");
    require(table.size() > 0, "minimal.splat should contain rows");
    require(table.x.size() == table.opacity.size(), "minimal.splat column length mismatch");
    const auto all_finite_rot = std::all_of(table.rot0.begin(), table.rot0.end(), [](float value) {
        return std::isfinite(value);
    });
    require(all_finite_rot, "minimal.splat rot_0 contains non-finite values");
}

void read_file_dispatches_ply() {
    const auto table = ga3d::read_file("tests/fixtures/minimal.ply");
    require(table.size() == 1, "read_file should dispatch .ply");
}

void read_sog_rejects_unknown_version() {
    const auto path = std::filesystem::temp_directory_path() / "ga3d-invalid-sog-meta.json";
    {
        std::ofstream file(path);
        file << R"({"version":3,"count":0})";
    }
    expect_throw_contains([&]() { (void)ga3d::read_sog(path); }, "Unsupported SOG meta version");
    std::filesystem::remove(path);
}

} // namespace

int main() {
    read_ply_minimal_fixture();
    read_splat_minimal_fixture();
    read_file_dispatches_ply();
    read_sog_rejects_unknown_version();
    return 0;
}
