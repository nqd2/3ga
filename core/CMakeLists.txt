find_package(ZLIB REQUIRED)
find_path(WEBP_INCLUDE_DIR NAMES webp/decode.h REQUIRED)
find_library(WEBP_LIBRARY NAMES webp REQUIRED)

add_library(ga3d_core
    src/collision_mesh.cpp
    src/data_table.cpp
    src/edit_recipe.cpp
    src/gaussian_extents.cpp
    src/glb_writer.cpp
    src/navmesh.cpp
    src/read_ply.cpp
    src/read_sog.cpp
    src/read_splat.cpp
    src/voxel_grid.cpp
    src/voxelize.cpp
    src/rust_bindings.cpp
)

target_include_directories(ga3d_core
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
    PRIVATE
        ${WEBP_INCLUDE_DIR}
)

target_compile_features(ga3d_core PUBLIC cxx_std_20)

target_link_libraries(ga3d_core
    PRIVATE
        ZLIB::ZLIB
        ${WEBP_LIBRARY}
)

if(GA3D_BUILD_TESTS)
    add_executable(ga3d_data_table_tests
        ../tests/core/data_table_tests.cpp
    )

    target_link_libraries(ga3d_data_table_tests
        PRIVATE
            ga3d_core
    )

    add_test(NAME DataTableTest COMMAND ga3d_data_table_tests)
    set_tests_properties(DataTableTest PROPERTIES WORKING_DIRECTORY ${PROJECT_SOURCE_DIR})

    add_executable(ga3d_reader_tests
        ../tests/core/reader_tests.cpp
    )

    target_link_libraries(ga3d_reader_tests
        PRIVATE
            ga3d_core
    )

    add_test(NAME ReaderTest COMMAND ga3d_reader_tests)
    set_tests_properties(ReaderTest PROPERTIES WORKING_DIRECTORY ${PROJECT_SOURCE_DIR})


    add_executable(ga3d_edit_recipe_tests
        ../tests/core/edit_recipe_tests.cpp
    )

    target_link_libraries(ga3d_edit_recipe_tests
        PRIVATE
            ga3d_core
    )

    add_test(NAME EditRecipeTest COMMAND ga3d_edit_recipe_tests)
    set_tests_properties(EditRecipeTest PROPERTIES WORKING_DIRECTORY ${PROJECT_SOURCE_DIR})

    add_executable(ga3d_voxel_tests
        ../tests/core/voxel_tests.cpp
    )

    target_link_libraries(ga3d_voxel_tests
        PRIVATE
            ga3d_core
    )

    add_test(NAME VoxelTest COMMAND ga3d_voxel_tests)
    set_tests_properties(VoxelTest PROPERTIES WORKING_DIRECTORY ${PROJECT_SOURCE_DIR})

    add_executable(ga3d_navmesh_tests
        ../tests/core/navmesh_tests.cpp
    )

    target_link_libraries(ga3d_navmesh_tests
        PRIVATE
            ga3d_core
    )

    add_test(NAME NavmeshTest COMMAND ga3d_navmesh_tests)
    set_tests_properties(NavmeshTest PROPERTIES WORKING_DIRECTORY ${PROJECT_SOURCE_DIR})

    add_executable(ga3d_glb_writer_tests
        ../tests/core/glb_writer_tests.cpp
    )

    target_link_libraries(ga3d_glb_writer_tests
        PRIVATE
            ga3d_core
    )

    add_test(NAME GlbWriterTest COMMAND ga3d_glb_writer_tests)
    set_tests_properties(GlbWriterTest PROPERTIES WORKING_DIRECTORY ${PROJECT_SOURCE_DIR})
endif()

install(TARGETS ga3d_core
    ARCHIVE DESTINATION lib
    LIBRARY DESTINATION lib
)
