/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Central field definition table for Polish Map attributes
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

#ifndef POLISHMAPFIELDS_H_INCLUDED
#define POLISHMAPFIELDS_H_INCLUDED

#include "ogrsf_frmts.h"
#include "cpl_string.h"
#include <string>
#include <vector>
#include <unordered_map>

/************************************************************************/
/*                         Layer type bitmask                           */
/************************************************************************/

static constexpr unsigned int LAYER_POI      = 0x01;
static constexpr unsigned int LAYER_POLYLINE = 0x02;
static constexpr unsigned int LAYER_POLYGON  = 0x04;
static constexpr unsigned int LAYER_ALL      = LAYER_POI | LAYER_POLYLINE | LAYER_POLYGON;

/************************************************************************/
/*                       PolishMapFieldDef                              */
/************************************************************************/

struct PolishMapFieldDef {
    const char* pszName;
    OGRFieldType eType;
    unsigned int nLayerMask;
};

/************************************************************************/
/*                  Field definition table                              */
/*                                                                      */
/* Central table of all Polish Map fields (except Data0 which is        */
/* added separately as it has special semantics).                       */
/* Order matters: this defines the field index order in the schema.     */
/************************************************************************/

static const PolishMapFieldDef g_aoPolishMapFields[] = {
    // Common fields (ALL layers)
    { "Type",        OFTString,  LAYER_ALL },
    { "SubType",     OFTString,  LAYER_ALL },
    { "Label",       OFTString,  LAYER_ALL },
    { "Marine",      OFTString,  LAYER_ALL },
    { "EndLevel",    OFTInteger, LAYER_ALL },
    { "Levels",      OFTString,  LAYER_ALL },

    // POI-specific fields
    { "City",        OFTString,  LAYER_POI },
    { "StreetDesc",  OFTString,  LAYER_POI },
    { "HouseNumber", OFTString,  LAYER_POI },
    { "PhoneNumber", OFTString,  LAYER_POI },
    { "Highway",     OFTString,  LAYER_POI },

    // Common geographic fields
    { "CityName",    OFTString,  LAYER_ALL },
    { "RegionName",  OFTString,  LAYER_ALL },
    { "CountryName", OFTString,  LAYER_ALL },

    // All layers — Zip is used on POI (addresses), POLYLINE (roads), POLYGON (communes)
    { "Zip",         OFTString,  LAYER_ALL },

    // POLYLINE-specific fields
    { "DirIndicator", OFTInteger, LAYER_POLYLINE },
    { "RoadID",       OFTString,  LAYER_POLYLINE },
    { "SpeedType",    OFTInteger, LAYER_POLYLINE },

    // Story 14.1: Routing attributes for POLYLINE
    { "RouteParam",   OFTString,  LAYER_POLYLINE },
    { "Roundabout",   OFTInteger, LAYER_POLYLINE },
    { "MaxHeight",    OFTInteger, LAYER_POLYLINE },
    { "MaxWeight",    OFTInteger, LAYER_POLYLINE },
    { "MaxWidth",     OFTInteger, LAYER_POLYLINE },
    { "MaxLength",    OFTInteger, LAYER_POLYLINE },
};

static constexpr int g_nPolishMapFieldCount =
    static_cast<int>(sizeof(g_aoPolishMapFields) / sizeof(g_aoPolishMapFields[0]));

/************************************************************************/
/*                         GetLayerFlag()                               */
/*                                                                      */
/* Convert layer name to bitmask flag.                                  */
/************************************************************************/

inline unsigned int GetLayerFlag(const char* pszLayerName) {
    if (EQUAL(pszLayerName, "POI"))      return LAYER_POI;
    if (EQUAL(pszLayerName, "POLYLINE")) return LAYER_POLYLINE;
    if (EQUAL(pszLayerName, "POLYGON"))  return LAYER_POLYGON;
    return 0;
}

/************************************************************************/
/*                       GetFieldsForLayer()                            */
/*                                                                      */
/* Returns the list of field definitions applicable to a given layer.   */
/************************************************************************/

inline std::vector<const PolishMapFieldDef*> GetFieldsForLayer(unsigned int nLayerFlag) {
    std::vector<const PolishMapFieldDef*> aoResult;
    for (int i = 0; i < g_nPolishMapFieldCount; i++) {
        if (g_aoPolishMapFields[i].nLayerMask & nLayerFlag) {
            aoResult.push_back(&g_aoPolishMapFields[i]);
        }
    }
    return aoResult;
}

/************************************************************************/
/*                      IsFieldForLayer()                               */
/*                                                                      */
/* Check if a canonical field name is applicable to a layer type.       */
/************************************************************************/

inline bool IsFieldForLayer(const char* pszCanonical, unsigned int nLayerFlag) {
    for (int i = 0; i < g_nPolishMapFieldCount; i++) {
        if (EQUAL(g_aoPolishMapFields[i].pszName, pszCanonical)) {
            return (g_aoPolishMapFields[i].nLayerMask & nLayerFlag) != 0;
        }
    }
    return false;
}

/************************************************************************/
/*                      GetFieldAliasMap()                              */
/*                                                                      */
/* Returns static alias map: source name -> canonical Polish Map name.  */
/* Thread-safe via static local (C++11 guarantees).                     */
/************************************************************************/

inline const std::unordered_map<std::string, std::string>& GetFieldAliasMap() {
    static const std::unordered_map<std::string, std::string> s_oAliasMap = {
        // Type aliases
        {"MP_TYPE",      "Type"},
        {"TYPE_CODE",    "Type"},
        // Label aliases
        {"NAME",         "Label"},
        {"NOM",          "Label"},
        // SubType aliases
        {"SUBTYPE",      "SubType"},
        {"SUB_TYPE",     "SubType"},
        // CityName aliases
        {"CITY",         "CityName"},
        {"VILLE",        "CityName"},
        {"CITY_NAME",    "CityName"},
        // RegionName aliases
        {"REGION",       "RegionName"},
        {"REGION_NAME",  "RegionName"},
        // CountryName aliases
        {"COUNTRY",      "CountryName"},
        {"COUNTRY_NAME", "CountryName"},
        {"PAYS",         "CountryName"},
        // StreetDesc aliases
        {"STREET",       "StreetDesc"},
        {"STREET_DESC",  "StreetDesc"},
        {"ADRESSE",      "StreetDesc"},
        // HouseNumber aliases
        {"HOUSE_NUMBER", "HouseNumber"},
        {"HOUSENUMBER",  "HouseNumber"},
        {"NUMERO",       "HouseNumber"},
        // Zip aliases
        {"ZIP",          "Zip"},
        {"ZIP_CODE",     "Zip"},
        {"CODE_POSTAL",  "Zip"},
        {"POSTAL_CODE",  "Zip"},
        // PhoneNumber aliases
        {"PHONE",        "PhoneNumber"},
        {"PHONE_NUMBER", "PhoneNumber"},
        {"TELEPHONE",    "PhoneNumber"},
        // Highway aliases
        {"HIGHWAY",      "Highway"},
        {"AUTOROUTE",    "Highway"},
        // City flag alias
        {"CITY_FLAG",    "City"},
        // DirIndicator aliases
        {"DIR_INDICATOR","DirIndicator"},
        {"DIRECTION",    "DirIndicator"},
        // RoadID alias
        {"ROAD_ID",      "RoadID"},
        // SpeedType aliases
        {"SPEED",        "SpeedType"},
        {"SPEED_TYPE",   "SpeedType"},
        // Story 14.1: Routing attribute aliases
        {"ROUTE_PARAM",  "RouteParam"},
        {"ROUTEPARAM",   "RouteParam"},
        {"ROUTEPARAMS",  "RouteParam"},
        {"ROUNDABOUT",   "Roundabout"},
        {"ROND_POINT",   "Roundabout"},
        {"MAX_HEIGHT",   "MaxHeight"},
        {"MAX_WEIGHT",   "MaxWeight"},
        {"MAX_WIDTH",    "MaxWidth"},
        {"MAX_LENGTH",   "MaxLength"},
    };
    return s_oAliasMap;
}

/************************************************************************/
/*                      ResolveFieldAlias()                             */
/*                                                                      */
/* Resolve a source field name to its canonical Polish Map name.        */
/* Returns the canonical name if an alias matches, or empty string.     */
/* Matching is case-insensitive.                                        */
/************************************************************************/

inline std::string ResolveFieldAlias(const char* pszFieldName) {
    // First: check if it's already a canonical field name (case-insensitive)
    for (int i = 0; i < g_nPolishMapFieldCount; i++) {
        if (EQUAL(pszFieldName, g_aoPolishMapFields[i].pszName)) {
            return g_aoPolishMapFields[i].pszName;
        }
    }

    // Second: check alias map (case-insensitive)
    // Convert to uppercase for alias lookup
    std::string osUpper(pszFieldName);
    for (auto& c : osUpper) {
        c = static_cast<char>(toupper(static_cast<unsigned char>(c)));
    }

    const auto& oAliasMap = GetFieldAliasMap();
    auto it = oAliasMap.find(osUpper);
    if (it != oAliasMap.end()) {
        return it->second;
    }

    return std::string();
}

#endif /* POLISHMAPFIELDS_H_INCLUDED */
