cmake_minimum_required(VERSION 3.20)
project(ogr-garminimg VERSION 1.0.0 LANGUAGES CXX)

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
set(GARMINIMG_SOURCES
    src/ogrgarminimgdriver.cpp
    src/ogrgarminimgdatasource.cpp
    src/ogrgarminimglayer.cpp
    src/garminimgfilesystem.cpp
    src/garminimglblparser.cpp
    src/garminimgtreparser.cpp
    src/garminimgbitreader.cpp
    src/garminimgrgnparser.cpp
    src/garminimgnetparser.cpp
    src/garminimgnodparser.cpp
    src/garminimgtypparser.cpp
    src/garminimgbitwriter.cpp
    src/garminimglblwriter.cpp
    src/garminimgtrewriter.cpp
    src/garminimgrgnwriter.cpp
    src/garminimgnetwriter.cpp
    src/garminimgnodwriter.cpp
    src/garminimgwriter.cpp
    src/typ2qml.cpp
)

# Build shared library (GDAL plugin)
add_library(ogr_GarminIMG MODULE ${GARMINIMG_SOURCES})

# Include directories
target_include_directories(ogr_GarminIMG PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

# Link against GDAL
target_link_libraries(ogr_GarminIMG PRIVATE ${GDAL_LIBRARIES})

# Position-independent code (required for shared library/plugin)
# PREFIX "" removes the "lib" prefix so the file is named ogr_GarminIMG.so
set_target_properties(ogr_GarminIMG PROPERTIES
    POSITION_INDEPENDENT_CODE ON
    CXX_VISIBILITY_PRESET hidden
    PREFIX ""
)

# Compiler flags
if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    target_compile_options(ogr_GarminIMG PRIVATE
        -Wall
        -Wextra
        -Wpedantic
        -Wno-unused-parameter
    )
elseif(CMAKE_CXX_COMPILER_ID MATCHES "MSVC")
    target_compile_options(ogr_GarminIMG PRIVATE
        /W4
        /wd4100  # unreferenced formal parameter
    )
endif()

# Determine GDAL plugin installation directory
if(NOT DEFINED GDAL_PLUGIN_DIR)
    if(UNIX AND NOT APPLE)
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
install(TARGETS ogr_GarminIMG
    LIBRARY DESTINATION ${GDAL_PLUGIN_DIR}
    COMPONENT driver
)

# Test executable - Driver Registration
add_executable(test_driver_registration test/test_driver_registration.cpp ${GARMINIMG_SOURCES})
target_include_directories(test_driver_registration PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_driver_registration PRIVATE ${GDAL_LIBRARIES})

# Test executable - Identify
add_executable(test_identify test/test_identify.cpp ${GARMINIMG_SOURCES})
target_include_directories(test_identify PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_identify PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_identify PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# Test executable - Filesystem
add_executable(test_filesystem test/test_filesystem.cpp ${GARMINIMG_SOURCES})
target_include_directories(test_filesystem PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(test_filesystem PRIVATE ${GDAL_LIBRARIES})
target_compile_definitions(test_filesystem PRIVATE
    -DTEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/test/data"
)

# CLI tool - typ2qml
add_executable(typ2qml tools/typ2qml_cli.cpp ${GARMINIMG_SOURCES})
target_include_directories(typ2qml PRIVATE
    ${GDAL_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(typ2qml PRIVATE ${GDAL_LIBRARIES})

# Summary
message(STATUS "=== OGR GarminIMG Driver Configuration ===")
message(STATUS "Build type: ${CMAKE_BUILD_TYPE}")
message(STATUS "C++ Standard: ${CMAKE_CXX_STANDARD}")
message(STATUS "GDAL include: ${GDAL_INCLUDE_DIRS}")
message(STATUS "GDAL library: ${GDAL_LIBRARIES}")
message(STATUS "Plugin install dir: ${GDAL_PLUGIN_DIR}")
message(STATUS "==========================================")
