/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  CLI tool for TYP to QML conversion
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#include "garminimgtypparser.h"
#include "garminimgfilesystem.h"
#include "typ2qml.h"
#include "cpl_conv.h"
#include "cpl_string.h"

#include <cstdio>
#include <cstring>
#include <string>

static void Usage() {
    fprintf(stderr,
        "Usage: typ2qml <input.typ|input.img> [options]\n"
        "\n"
        "Options:\n"
        "  --output-dir <dir>       Output directory (default: current)\n"
        "  --palette day|night      Color palette (default: day)\n"
        "  --layer poi|polyline|polygon|all  Layer filter (default: all)\n"
        "\n"
        "If input is .img, the embedded TYP is extracted automatically.\n"
        "Output: <basename>_poi.qml, <basename>_polyline.qml, <basename>_polygon.qml\n"
    );
}

int main(int argc, char* argv[]) {
    if (argc < 2) {
        Usage();
        return 1;
    }

    std::string osInput = argv[1];
    std::string osOutputDir = ".";
    std::string osPalette = "day";

    for (int i = 2; i < argc; i++) {
        if (strcmp(argv[i], "--output-dir") == 0 && i + 1 < argc) {
            osOutputDir = argv[++i];
        } else if (strcmp(argv[i], "--palette") == 0 && i + 1 < argc) {
            osPalette = argv[++i];
        } else if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
            Usage();
            return 0;
        }
    }

    // Determine basename
    std::string osBaseName = CPLGetBasename(osInput.c_str());

    // Parse TYP
    GarminIMGTYPParser oParser;
    bool bOk = false;

    const char* pszExt = CPLGetExtension(osInput.c_str());
    if (EQUAL(pszExt, "img")) {
        // Extract TYP from IMG
        GarminIMGFilesystem oFS;
        if (!oFS.Parse(osInput.c_str())) {
            fprintf(stderr, "Error: Cannot parse IMG file: %s\n", osInput.c_str());
            return 1;
        }

        // Find TYP subfile
        bool bFoundTYP = false;
        for (const auto& [osName, oSubfile] : oFS.GetSubfiles()) {
            if (oSubfile.osExtension == "TYP") {
                bOk = oParser.Parse(oSubfile.abyData.data(),
                                    static_cast<uint32_t>(oSubfile.abyData.size()));
                bFoundTYP = true;
                break;
            }
        }

        if (!bFoundTYP) {
            fprintf(stderr, "Error: No TYP subfile found in IMG: %s\n",
                    osInput.c_str());
            return 1;
        }
    } else {
        // Parse TYP file directly
        bOk = oParser.ParseFile(osInput.c_str());
    }

    if (!bOk) {
        fprintf(stderr, "Error: Failed to parse TYP data\n");
        return 1;
    }

    if (!oParser.HasStyles()) {
        fprintf(stderr, "Warning: No styles found in TYP file\n");
    }

    // Convert to QML
    if (!ConvertTypToQML(oParser, osOutputDir, osBaseName, osPalette)) {
        fprintf(stderr, "Error: Failed to generate QML files\n");
        return 1;
    }

    printf("Generated QML files in %s:\n", osOutputDir.c_str());
    printf("  %s_poi.qml\n", osBaseName.c_str());
    printf("  %s_polyline.qml\n", osBaseName.c_str());
    printf("  %s_polygon.qml\n", osBaseName.c_str());

    return 0;
}
