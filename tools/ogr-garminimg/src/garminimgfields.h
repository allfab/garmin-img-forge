/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Field definitions for Garmin IMG OGR layers
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a
 * copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included
 * in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
 * OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 ****************************************************************************/

#ifndef GARMINIMGFIELDS_H_INCLUDED
#define GARMINIMGFIELDS_H_INCLUDED

#include "ogr_core.h"

#include <vector>

/************************************************************************/
/*                      Layer bitmask constants                         */
/************************************************************************/

constexpr unsigned int LAYER_POI      = 0x01;
constexpr unsigned int LAYER_POLYLINE = 0x02;
constexpr unsigned int LAYER_POLYGON  = 0x04;
constexpr unsigned int LAYER_ROAD     = 0x08;
constexpr unsigned int LAYER_NODE     = 0x10;
constexpr unsigned int LAYER_ALL      = 0x1F;

// Geometry layers only (POI + POLYLINE + POLYGON)
constexpr unsigned int LAYER_GEOM     = 0x07;

/************************************************************************/
/*                      GarminIMGFieldDef                               */
/************************************************************************/

struct GarminIMGFieldDef {
    const char* pszName;
    OGRFieldType eType;
    unsigned int nLayerMask;
};

/************************************************************************/
/*                      Field definition table                          */
/************************************************************************/

// clang-format off
static const GarminIMGFieldDef g_aoGarminIMGFields[] = {
    // Common fields (all geometry layers)
    { "Type",           OFTString,  LAYER_ALL  },
    { "SubType",        OFTString,  LAYER_ALL  },
    { "Label",          OFTString,  LAYER_ALL  },
    { "EndLevel",       OFTInteger, LAYER_GEOM },
    { "Levels",         OFTString,  LAYER_GEOM },

    // POI-specific
    { "City",           OFTString,  LAYER_POI  },
    { "HouseNumber",    OFTString,  LAYER_POI  },
    { "Phone",          OFTString,  LAYER_POI  },

    // POLYLINE-specific
    { "DirIndicator",   OFTInteger, LAYER_POLYLINE },

    // ROAD_NETWORK fields
    { "RoadClass",      OFTInteger, LAYER_ROAD },
    { "Speed",          OFTInteger, LAYER_ROAD },
    { "OneWay",         OFTInteger, LAYER_ROAD },
    { "Toll",           OFTInteger, LAYER_ROAD },
    { "AccessFlags",    OFTString,  LAYER_ROAD },
    { "DeniedEmergency",OFTInteger, LAYER_ROAD },
    { "DeniedDelivery", OFTInteger, LAYER_ROAD },
    { "DeniedCar",      OFTInteger, LAYER_ROAD },
    { "DeniedBus",      OFTInteger, LAYER_ROAD },
    { "DeniedTaxi",     OFTInteger, LAYER_ROAD },
    { "DeniedPedestrian",OFTInteger,LAYER_ROAD },
    { "DeniedBicycle",  OFTInteger, LAYER_ROAD },
    { "DeniedTruck",    OFTInteger, LAYER_ROAD },
    { "RoadLength",     OFTReal,    LAYER_ROAD },

    // ROUTING_NODE fields
    { "NodeType",       OFTString,  LAYER_NODE },
    { "ArcCount",       OFTInteger, LAYER_NODE },
    { "ConnectedRoads", OFTString,  LAYER_NODE },

    // TYP symbology fields (all geometry layers)
    { "TYP_FillColor",  OFTString,  LAYER_GEOM },
    { "TYP_BorderColor",OFTString,  LAYER_GEOM },
    { "TYP_LineColor",  OFTString,  LAYER_GEOM },
    { "TYP_LineWidth",  OFTInteger, LAYER_GEOM },
    { "TYP_IconData",   OFTBinary,  LAYER_POI  },
    { "TYP_PatternData",OFTBinary,  LAYER_POLYGON },
    { "TYP_DisplayName",OFTString,  LAYER_GEOM },
};
// clang-format on

/************************************************************************/
/*                      Helper functions                                */
/************************************************************************/

inline unsigned int GetGarminIMGLayerFlag(const char* pszLayerName) {
    if (strcmp(pszLayerName, "POI") == 0) return LAYER_POI;
    if (strcmp(pszLayerName, "POLYLINE") == 0) return LAYER_POLYLINE;
    if (strcmp(pszLayerName, "POLYGON") == 0) return LAYER_POLYGON;
    if (strcmp(pszLayerName, "ROAD_NETWORK") == 0) return LAYER_ROAD;
    if (strcmp(pszLayerName, "ROUTING_NODE") == 0) return LAYER_NODE;
    return 0;
}

inline std::vector<const GarminIMGFieldDef*>
GetFieldsForLayer(unsigned int nLayerFlag) {
    std::vector<const GarminIMGFieldDef*> aoFields;
    for (const auto& oField : g_aoGarminIMGFields) {
        if (oField.nLayerMask & nLayerFlag) {
            aoFields.push_back(&oField);
        }
    }
    return aoFields;
}

#endif /* GARMINIMGFIELDS_H_INCLUDED */
