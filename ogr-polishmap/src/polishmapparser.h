/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Polish Map format parser - header file
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

#ifndef POLISHMAPPARSER_H_INCLUDED
#define POLISHMAPPARSER_H_INCLUDED

#include "cpl_port.h"
#include "cpl_string.h"
#include <map>
#include <string>
#include <vector>

/************************************************************************/
/*                            SectionType                               */
/*                                                                      */
/* Enum for Polish Map section types - used by unified ParseNextSection */
/* REFACTORING: DRY elimination of duplicated parsing methods.          */
/************************************************************************/

enum class SectionType {
    POI,        // [POI] or [RGN10] - Point features
    Polyline,   // [POLYLINE] - Linear features
    Polygon     // [POLYGON] - Area features
};

/************************************************************************/
/*                        PolishMapSection                              */
/*                                                                      */
/* Unified IR structure for all section types (POI, POLYLINE, POLYGON). */
/* REFACTORING: Replaces duplicate POI/Polyline/Polygon structures for  */
/* the unified ParseNextSection() method.                               */
/************************************************************************/

struct PolishMapSection {
    SectionType eType;                               // Section type
    std::string osType;                              // Garmin type code "0x2C00", "0x16", etc.
    std::string osLabel;                             // UTF-8 after conversion
    std::vector<std::pair<double, double>> aoCoords; // Coordinates (1 for POI, N for POLY*)
    int nEndLevel;                                   // 0-9, -1 if absent
    std::string osLevels;                            // "0-3" or empty
    std::map<std::string, std::string> aoOtherFields;// Additional fields

    // Default constructor
    PolishMapSection() : eType(SectionType::POI), nEndLevel(-1) {}

    // Constructor with type
    explicit PolishMapSection(SectionType type) : eType(type), nEndLevel(-1) {}

    // Clear all data
    void Clear() {
        osType.clear();
        osLabel.clear();
        aoCoords.clear();
        nEndLevel = -1;
        osLevels.clear();
        aoOtherFields.clear();
    }

    // Get minimum point count for valid geometry
    int GetMinPointCount() const {
        switch (eType) {
            case SectionType::POI: return 1;
            case SectionType::Polyline: return 2;
            case SectionType::Polygon: return 3;
        }
        return 1;
    }

    // Get section marker string
    const char* GetSectionMarker() const {
        switch (eType) {
            case SectionType::POI: return "[POI]";
            case SectionType::Polyline: return "[POLYLINE]";
            case SectionType::Polygon: return "[POLYGON]";
        }
        return "[UNKNOWN]";
    }

    // Get section type name for logging
    const char* GetTypeName() const {
        switch (eType) {
            case SectionType::POI: return "POI";
            case SectionType::Polyline: return "POLYLINE";
            case SectionType::Polygon: return "POLYGON";
        }
        return "UNKNOWN";
    }
};

/************************************************************************/
/*                        PolishMapHeaderData                           */
/*                                                                      */
/* Intermediate Representation (IR) structure for [IMG ID] header data. */
/* Architecture: Minimal IR, cleared after each section processed.      */
/************************************************************************/

struct PolishMapHeaderData {
    std::string osName;           // Map name
    std::string osID;             // Map ID
    std::string osCodePage;       // Encoding (default: 1252)
    std::string osDatum;          // Coordinate system (default: WGS 84)
    std::string osElevation;      // Elevation unit (M/F)
    std::map<std::string, std::string> aoOtherFields;  // All other key=value pairs

    // Default values
    PolishMapHeaderData() : osCodePage("1252"), osDatum("WGS 84") {}

    // Clear all data
    void Clear() {
        osName.clear();
        osID.clear();
        osCodePage = "1252";
        osDatum = "WGS 84";
        osElevation.clear();
        aoOtherFields.clear();
    }
};

/************************************************************************/
/*                        PolishMapPOISection                           */
/*                                                                      */
/* Intermediate Representation (IR) structure for [POI] section data.   */
/* Story 1.4: IR minimaliste pour une seule section POI à la fois.     */
/************************************************************************/

struct PolishMapPOISection {
    std::string osType;                    // "0x2C00"
    std::string osLabel;                   // UTF-8 après conversion
    std::pair<double, double> oCoords;     // (lat, lon)
    int nEndLevel;                         // 0-9, -1 si absent
    std::string osLevels;                  // "0-3" ou vide
    std::map<std::string, std::string> aoOtherFields;  // Data1, Data2, etc.

    // Default values
    PolishMapPOISection() : oCoords(0.0, 0.0), nEndLevel(-1) {}

    // Clear all data
    void Clear() {
        osType.clear();
        osLabel.clear();
        oCoords = std::make_pair(0.0, 0.0);
        nEndLevel = -1;
        osLevels.clear();
        aoOtherFields.clear();
    }
};

/************************************************************************/
/*                      PolishMapPolylineSection                        */
/*                                                                      */
/* Intermediate Representation (IR) structure for [POLYLINE] section.   */
/* Story 1.5: IR minimaliste pour une seule section POLYLINE à la fois.*/
/************************************************************************/

struct PolishMapPolylineSection {
    std::string osType;                              // "0x16"
    std::string osLabel;                             // UTF-8 après conversion
    std::vector<std::pair<double, double>> aoCoords; // [(lat1, lon1), (lat2, lon2), ...]
    int nEndLevel;                                   // 0-9, -1 si absent
    std::string osLevels;                            // "0-3" ou vide
    std::map<std::string, std::string> aoOtherFields;// Champs additionnels

    // Default values
    PolishMapPolylineSection() : nEndLevel(-1) {}

    // Clear all data
    void Clear() {
        osType.clear();
        osLabel.clear();
        aoCoords.clear();
        nEndLevel = -1;
        osLevels.clear();
        aoOtherFields.clear();
    }
};

/************************************************************************/
/*                      PolishMapPolygonSection                         */
/*                                                                      */
/* Intermediate Representation (IR) structure for [POLYGON] section.    */
/* Story 1.6: IR minimaliste pour une seule section POLYGON à la fois.  */
/************************************************************************/

struct PolishMapPolygonSection {
    std::string osType;                              // "0x4C"
    std::string osLabel;                             // UTF-8 après conversion
    std::vector<std::pair<double, double>> aoCoords; // [(lat1, lon1), (lat2, lon2), ...]
    int nEndLevel;                                   // 0-9, -1 si absent
    std::string osLevels;                            // "0-3" ou vide
    std::map<std::string, std::string> aoOtherFields;// Champs additionnels

    // Default values
    PolishMapPolygonSection() : nEndLevel(-1) {}

    // Clear all data
    void Clear() {
        osType.clear();
        osLabel.clear();
        aoCoords.clear();
        nEndLevel = -1;
        osLevels.clear();
        aoOtherFields.clear();
    }
};

/************************************************************************/
/*                         PolishMapParser                              */
/*                                                                      */
/* Hybrid parser for Polish Map format files:                           */
/* - Level 1: Section detection via [SECTION_NAME] markers              */
/* - Level 2: Key=value parsing inside sections                         */
/* - State machine for section transitions                              */
/************************************************************************/

class PolishMapParser {
public:
    explicit PolishMapParser(const char* pszFilePath);
    ~PolishMapParser();

    // Disable copy and assignment
    PolishMapParser(const PolishMapParser&) = delete;
    PolishMapParser& operator=(const PolishMapParser&) = delete;

    // Parse the [IMG ID] header section
    // Returns TRUE on success, FALSE on failure
    bool ParseHeader();

    // Get parsed header data (valid after successful ParseHeader())
    const PolishMapHeaderData& GetHeaderData() const { return m_oHeaderData; }

    // Check if file was successfully opened
    bool IsOpen() const { return m_fpFile != nullptr; }

    // REFACTORING: Unified section parsing (DRY pattern)
    // Parse next section of specified type from file
    // Returns TRUE if section found and parsed, FALSE if no more sections of that type
    bool ParseNextSection(SectionType eTargetType, PolishMapSection& oSection);

    // Reset reading position to start of data sections (after header)
    void ResetSectionReading();

    // Story 1.4: POI section parsing (wrapper for backward compatibility)
    // Parse next [POI] section from file
    // Returns TRUE if POI found and parsed, FALSE if no more POI sections
    bool ParseNextPOI(PolishMapPOISection& oSection);

    // Reset reading position to start of POI sections (after header)
    // Note: Alias for ResetSectionReading() - all layers share same position
    void ResetPOIReading() { ResetSectionReading(); }

    // Story 1.5: POLYLINE section parsing (wrapper for backward compatibility)
    // Parse next [POLYLINE] section from file
    // Returns TRUE if POLYLINE found and parsed, FALSE if no more POLYLINE sections
    bool ParseNextPolyline(PolishMapPolylineSection& oSection);

    // Reset reading position to start of POLYLINE sections (after header)
    // Note: Alias for ResetSectionReading() - all layers share same position
    void ResetPolylineReading() { ResetSectionReading(); }

    // Story 1.6: POLYGON section parsing (wrapper for backward compatibility)
    // Parse next [POLYGON] section from file
    // Returns TRUE if POLYGON found and parsed, FALSE if no more POLYGON sections
    bool ParseNextPolygon(PolishMapPolygonSection& oSection);

    // Reset reading position to start of POLYGON sections (after header)
    // Note: Alias for ResetSectionReading() - all layers share same position
    void ResetPolygonReading() { ResetSectionReading(); }

    // Get current line number (for debugging)
    int GetCurrentLine() const { return m_nCurrentLine; }

private:
    CPLString m_osFilePath;
    VSILFILE* m_fpFile;
    PolishMapHeaderData m_oHeaderData;
    vsi_l_offset m_nAfterHeaderPos;  // File position after header (start of data sections)
    int m_nCurrentLine;               // Current line number for error reporting

    // Helper methods
    bool ReadLine(CPLString& osLine);
    bool ParseKeyValue(const CPLString& osLine, CPLString& osKey, CPLString& osValue);
    CPLString RecodeToUTF8(const CPLString& osValue);
    bool ParseCoordinates(const CPLString& osValue, double& dfLat, double& dfLon);

    // Story 1.5 REFACTORING: Parse coordinate LIST from Data0
    // Format: "(lat1,lon1),(lat2,lon2),..." OR "lat1,lon1,lat2,lon2,..."
    // Returns number of points parsed (0 on error)
    int ParseCoordinateList(const CPLString& osValue,
                            std::vector<std::pair<double, double>>& aoCoords);
};

#endif /* POLISHMAPPARSER_H_INCLUDED */
