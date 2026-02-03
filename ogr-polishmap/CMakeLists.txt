cmake_minimum_required(VERSION 3.20)
project(ogr-polishmap VERSION 1.0.0 LANGUAGES CXX)

# C++17 standard required (GDAL 3.6+ requirement)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Find GDAL 3.6 or higher
find_package(GDAL 3.6 REQUIRED)

if(GDAL_FOUND)
    message(STATUS "Found GDAL version: ${GDAL_VERSION}")
else()
    message(FATAL_ERROR "GDAL 3.6+ is required but not found")
endif()

# Source files
set(POLISHMAP_SOURCES
    src/ogrpolishmapdriver.cpp
    src/ogrpolishmapdatasource.cpp
    src/ogrpolishmaplayer.cpp
    src/polishmapparser.cpp
    src/polishmapwriter.cpp
)

# Build shared library (GDAL plugin)
add_library(gdal_POLISHMAP MODULE ${POLISHMAP_SOURCES})

# Include directories
target_include_directories(gdal_POLISHMAP PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

# Link against GDAL
target_link_libraries(gdal_POLISHMAP PRIVATE ${GDAL_LIBRARIES})

# Position-independent code (required for shared library/plugin)
# PREFIX "" removes the "lib" prefix so the file is named gdal_POLISHMAP.so
# This is required for GDAL to find the plugin
set_target_properties(gdal_POLISHMAP PROPERTIES
    POSITION_INDEPENDENT_CODE ON
    CXX_Vlibjson
ISIBILITY_PRESET hidden
    PREFIX ""
)

# Compiler flags
if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    target_compile_options(gdal_POLISHMAP PRIVATE
        -Wall
        -Wextra
        -Wpedantic
        -Wno-unused-parameter
    )
elseif(CMAKE_CXX_COMPILER_ID MATCHES "MSVC")
    target_compile_options(gdal_POLISHMAP PRIVATE
        /W4
        /wd4100  # unreferenced formal parameter
    )
endif()

# Determine GDAL plugin installation directory
if(NOT DEFINED GDAL_PLUGIN_DIR)
    if(UNIX AND NOT APPLE)
        # Linux: Try to detect system plugin directory
        execute_process(
            COMMAND gdal-config --prefix
            OUTPUT_VARIABLE GDAL_PREFIX
            OUTPUT_STRIP_TRAILING_WHITESPACE
            ERROR_QUIET
        )
        if(GDAL_PREFIX)
            set(GDAL_PLUGIN_DIR "${GDAL_PREFIX}/lib/gdalplugins" CACHE PATH "GDAL plugin directory")
        else()
            set(GDAL_PLUGIN_DIR "/usr/lib/gdalplugins" CACHE PATH "GDAL plugin directory")
        endif()
    elseif(APPLE)
        set(GDAL_PLUGIN_DIR "/usr/local/lib/gdalplugins" CACHE PATH "GDAL plugin directory")
    elseif(WIN32)
        set(GDAL_PLUGIN_DIR "C:/OSGeo4W64/bin/gdalplugins" CACHE PATH "GDAL plugin directory")
    endif()
endif()

# Installation
install(TARGETS gdal_POLISHMAP
    LIBRARY DESTINATION ${GDAL_PLUGIN_DIR}
    COMPONENT driver
)

# Test executable - Driver Registration
add_executable(test_driver_registration test/test_driver_registration.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_driver_registration PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_driver_registration PRIVATE ${GDAL_LIBRARIES})

# Test executable - Identify Content Validation
add_executable(test_identify_content test/test_identify_content.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_identify_content PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_identify_content PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_identify_content PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - Parser and Open()
add_executable(test_parser_and_open test/test_parser_and_open.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_parser_and_open PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_parser_and_open PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_parser_and_open PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - Dataset Layers (Story 1.3)
add_executable(test_dataset_layers test/test_dataset_layers.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_dataset_layers PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_dataset_layers PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_dataset_layers PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - POI Layer (Story 1.4)
add_executable(test_poi_layer test/test_poi_layer.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_poi_layer PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_poi_layer PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_poi_layer PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - POLYLINE Layer (Story 1.5)
add_executable(test_polyline_layer test/test_polyline_layer.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_polyline_layer PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_polyline_layer PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_polyline_layer PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - POLYGON Layer (Story 1.6)
add_executable(test_polygon_layer test/test_polygon_layer.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_polygon_layer PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_polygon_layer PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_polygon_layer PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - Filters (Story 1.7)
add_executable(test_filters test/test_filters.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_filters PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_filters PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_filters PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - Create (Story 2.1)
add_executable(test_create test/test_create.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_create PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_create PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_create PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - Header (Story 2.2)
add_executable(test_header test/test_header.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_header PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_header PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_header PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - POI Write (Story 2.3)
add_executable(test_poi_write test/test_poi_write.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_poi_write PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_poi_write PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_poi_write PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - POLYLINE Write (Story 2.4)
add_executable(test_polyline_write test/test_polyline_write.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_polyline_write PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_polyline_write PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_polyline_write PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - POLYGON Write (Story 2.5)
add_executable(test_polygon_write test/test_polygon_write.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_polygon_write PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_polygon_write PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_polygon_write PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - Driver Metadata (Story 2.6)
add_executable(test_driver_metadata test/test_driver_metadata.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_driver_metadata PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_driver_metadata PRIVATE ${GDAL_LIBRARIES})

# Summary
message(STATUS "=== OGR PolishMap Driver Configuration ===")
message(STATUS "Build type: ${CMAKE_BUILD_TYPE}")
message(STATUS "C++ Standard: ${CMAKE_CXX_STANDARD}")
message(STATUS "GDAL include: ${GDAL_INCLUDE_DIRS}")
message(STATUS "GDAL library: ${GDAL_LIBRARIES}")
message(STATUS "Plugin install dir: ${GDAL_PLUGIN_DIR}")
message(STATUS "=========================================")
