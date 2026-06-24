cmake_minimum_required(VERSION 3.20)

project(ga3d
    VERSION 0.1.0
    DESCRIPTION "3D Gaussian Splatting processing tool for AR"
    LANGUAGES CXX
)

option(GA3D_BUILD_TESTS "Build ga3d C++ tests" OFF)

if(GA3D_BUILD_TESTS)
    enable_testing()
endif()

add_subdirectory(core)
