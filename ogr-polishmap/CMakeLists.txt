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
set_target_properties(gdal_POLISHMAP PROPERTIES
    POSITION_INDEPENDENT_CODE ON
    CXX_VISIBILITY_PRESET hidden
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

# Test executable
add_executable(test_driver_registration test/test_driver_registration.cpp ${POLISHMAP_SOURCES})
target_include_directories(test_driver_registration PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_driver_registration PRIVATE ${GDAL_LIBRARIES})

# Summary
message(STATUS "=== OGR PolishMap Driver Configuration ===")
message(STATUS "Build type: ${CMAKE_BUILD_TYPE}")
message(STATUS "C++ Standard: ${CMAKE_CXX_STANDARD}")
message(STATUS "GDAL include: ${GDAL_INCLUDE_DIRS}")
message(STATUS "GDAL library: ${GDAL_LIBRARIES}")
message(STATUS "Plugin install dir: ${GDAL_PLUGIN_DIR}")
message(STATUS "=========================================")
