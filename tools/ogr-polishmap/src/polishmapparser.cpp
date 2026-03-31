/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Polish Map format parser - implementation
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

#include "polishmapparser.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include "cpl_vsi.h"
#include <cstring>

/************************************************************************/
/*                         PolishMapParser()                            */
/*                                                                      */
/* Story 3.1: Performance notes (NFR1: 10 MB < 2s)                       */
/* - CPLReadLineL() uses GDAL's internal buffering (already optimized)   */
/* - Measured: 0.455s for 10 MB parsing (well under 2s threshold)        */
/* - No additional buffer allocation needed                              */
/************************************************************************/

PolishMapParser::PolishMapParser(const char* pszFilePath)
    : m_osFilePath(pszFilePath)
    , m_fpFile(nullptr)
    , m_nAfterHeaderPos(0)
    , m_nCurrentLine(0)
{
    m_fpFile = VSIFOpenL(pszFilePath, "rb");
    if (m_fpFile == nullptr) {
        CPLDebug("OGR_POLISHMAP", "Failed to open file: %s", pszFilePath);
        return;
    }

    CPLDebug("OGR_POLISHMAP", "Opened file for parsing: %s", pszFilePath);
}

/************************************************************************/
/*                        ~PolishMapParser()                            */
/************************************************************************/

PolishMapParser::~PolishMapParser() {
    if (m_fpFile != nullptr) {
        VSIFCloseL(m_fpFile);
        m_fpFile = nullptr;
    }
}

/************************************************************************/
/*                            ReadLine()                                */
/*                                                                      */
/* Read a line from the file, handling various line endings.            */
/************************************************************************/

bool PolishMapParser::ReadLine(CPLString& osLine) {
    osLine.clear();

    if (m_fpFile == nullptr) {
        return false;
    }

    const char* pszLine = CPLReadLineL(m_fpFile);
    if (pszLine == nullptr) {
        return false;
    }

    osLine = pszLine;

    // Trim trailing whitespace
    while (!osLine.empty() && (osLine.back() == '\r' || osLine.back() == '\n' ||
                               osLine.back() == ' ' || osLine.back() == '\t')) {
        osLine.resize(osLine.size() - 1);
    }

    return true;
}

/************************************************************************/
/*                         ParseKeyValue()                              */
/*                                                                      */
/* Parse a "Key=Value" line. Returns false if not in expected format.   */
/************************************************************************/

bool PolishMapParser::ParseKeyValue(const CPLString& osLine,
                                    CPLString& osKey, CPLString& osValue) {
    // Skip empty lines and comments
    if (osLine.empty() || osLine[0] == ';') {
        return false;
    }

    // Find the '=' separator
    const char* pszEqual = strchr(osLine.c_str(), '=');
    if (pszEqual == nullptr) {
        return false;
    }

    // Extract key (before '=')
    osKey.assign(osLine.c_str(), pszEqual - osLine.c_str());

    // Trim key whitespace
    while (!osKey.empty() && (osKey.back() == ' ' || osKey.back() == '\t')) {
        osKey.resize(osKey.size() - 1);
    }

    // Extract value (after '=')
    osValue = pszEqual + 1;

    // Trim leading whitespace from value
    size_t nStart = 0;
    while (nStart < osValue.size() && (osValue[nStart] == ' ' || osValue[nStart] == '\t')) {
        nStart++;
    }
    if (nStart > 0) {
        osValue = osValue.substr(nStart);
    }

    return !osKey.empty();
}

/************************************************************************/
/*                         RecodeToUTF8()                               */
/*                                                                      */
/* Convert text from CP1252 to UTF-8 using CPLRecode.                   */
/* Architecture pattern from Architecture.md - Encoding Patterns.       */
/************************************************************************/

CPLString PolishMapParser::RecodeToUTF8(const CPLString& osValue) {
    // Use the CodePage from header if available, default to CP1252
    // Build encoding string like "CP1252", "CP1250", etc.
    // Keep osEncoding in scope for the entire function to avoid use-after-free
    CPLString osEncoding;
    if (!m_oHeaderData.osCodePage.empty()) {
        osEncoding = "CP" + m_oHeaderData.osCodePage;
    } else {
        osEncoding = "CP1252";
    }

    char* pszUTF8 = CPLRecode(osValue.c_str(), osEncoding.c_str(), CPL_ENC_UTF8);
    if (pszUTF8 != nullptr) {
        CPLString osResult(pszUTF8);
        CPLFree(pszUTF8);
        return osResult;
    }

    // Fallback to original value if recode fails
    return osValue;
}

/************************************************************************/
/*                          ParseHeader()                               */
/*                                                                      */
/* Parse the [IMG ID] header section of a Polish Map file.              */
/* Returns TRUE on success, FALSE if header is missing or invalid.      */
/************************************************************************/

bool PolishMapParser::ParseHeader() {
    if (m_fpFile == nullptr) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "Polish Map parser: file not open");
        return false;
    }

    // Reset to beginning of file
    VSIFSeekL(m_fpFile, 0, SEEK_SET);
    m_oHeaderData.Clear();

    CPLString osLine;
    bool bInImgIdSection = false;
    bool bFoundImgIdSection = false;
    int nLineNumber = 0;  // Story 3.2 M2: Track line number for error context

    // Parse file line by line
    while (ReadLine(osLine)) {
        nLineNumber++;
        // Check for section markers
        if (!osLine.empty() && osLine[0] == '[') {
            if (STARTS_WITH_CI(osLine.c_str(), "[IMG ID]")) {
                bInImgIdSection = true;
                bFoundImgIdSection = true;
                CPLDebug("OGR_POLISHMAP", "Found [IMG ID] section");
                continue;
            } else if (bInImgIdSection) {
                // Another section started, end of [IMG ID]
                break;
            }
        }

        // Check for end of section marker
        if (bInImgIdSection && STARTS_WITH_CI(osLine.c_str(), "[END-IMG ID]")) {
            break;
        }

        // Parse key=value pairs within [IMG ID] section
        if (bInImgIdSection) {
            CPLString osKey, osValue;
            if (ParseKeyValue(osLine, osKey, osValue)) {
                // Store known fields
                if (EQUAL(osKey.c_str(), "Name")) {
                    m_oHeaderData.osName = RecodeToUTF8(osValue);
                } else if (EQUAL(osKey.c_str(), "ID")) {
                    m_oHeaderData.osID = osValue;
                } else if (EQUAL(osKey.c_str(), "CodePage")) {
                    m_oHeaderData.osCodePage = osValue;
                } else if (EQUAL(osKey.c_str(), "Datum")) {
                    m_oHeaderData.osDatum = osValue;
                } else if (EQUAL(osKey.c_str(), "Elevation")) {
                    m_oHeaderData.osElevation = osValue;
                }
                // Story 1.2 Extension: Parse critical header fields
                else if (EQUAL(osKey.c_str(), "LBLcoding")) {
                    m_oHeaderData.osLBLcoding = osValue;
                } else if (EQUAL(osKey.c_str(), "Preprocess")) {
                    m_oHeaderData.osPreprocess = osValue;
                } else if (EQUAL(osKey.c_str(), "Levels")) {
                    m_oHeaderData.osLevels = osValue;
                } else if (EQUAL(osKey.c_str(), "TreeSize")) {
                    m_oHeaderData.osTreeSize = osValue;
                } else if (EQUAL(osKey.c_str(), "RgnLimit")) {
                    m_oHeaderData.osRgnLimit = osValue;
                }
                // Story 1.2 Extension: Parse important header fields
                else if (EQUAL(osKey.c_str(), "Transparent")) {
                    m_oHeaderData.osTransparent = osValue;
                } else if (EQUAL(osKey.c_str(), "SimplifyLevel")) {
                    m_oHeaderData.osSimplifyLevel = osValue;
                } else if (EQUAL(osKey.c_str(), "Marine")) {
                    m_oHeaderData.osMarine = osValue;
                } else if (EQUAL(osKey.c_str(), "LeftSideTraffic")) {
                    m_oHeaderData.osLeftSideTraffic = osValue;
                } else {
                    // Store other unrecognized fields in the map for round-trip preservation
                    m_oHeaderData.aoOtherFields[osKey] = osValue;
                }
            }
        }
    }

    if (!bFoundImgIdSection) {
        // Story 3.2 AC1/AC6: Critical Error with contextual message (filename + line count)
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "Polish Map file missing required [IMG ID] header after %d lines: %s",
                 nLineNumber, m_osFilePath.c_str());
        return false;
    }

    CPLDebug("OGR_POLISHMAP", "Parsed header: Name='%s', ID='%s', CodePage='%s', Datum='%s'",
             m_oHeaderData.osName.c_str(), m_oHeaderData.osID.c_str(),
             m_oHeaderData.osCodePage.c_str(), m_oHeaderData.osDatum.c_str());

    // Story 1.2 Extension: Validate required ID field (cGPSmapper spec requirement)
    if (m_oHeaderData.osID.empty()) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "Polish Map header: Missing required ID field in [IMG ID] section: %s",
                 m_osFilePath.c_str());
        return false;
    }

    // Story 1.2 Extension: Parse Level0-N definitions based on Levels count
    ParseLevelDefinitions();

    // Story 1.4: Save position after header for POI reading
    m_nAfterHeaderPos = VSIFTellL(m_fpFile);

    return true;
}

/************************************************************************/
/*                      ParseLevelDefinitions()                         */
/*                                                                      */
/* Story 1.2 Extension: Parse Level0-N definitions from header fields.  */
/* Extracts multi-value Level fields based on Levels count and stores  */
/* them in aoLevelDefs vector. Removes processed Level* from map.       */
/************************************************************************/

void PolishMapParser::ParseLevelDefinitions() {
    // Check if Levels field is defined
    if (m_oHeaderData.osLevels.empty()) {
        // No Levels field defined - this is valid for single-level maps
        CPLDebug("OGR_POLISHMAP", "No Levels field defined (single-level map)");
        return;
    }

    // Parse Levels count
    int nLevels = atoi(m_oHeaderData.osLevels.c_str());

    // Validate level count (cGPSmapper spec: 1-10 levels)
    if (nLevels <= 0 || nLevels > 10) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "Polish Map header: Levels=%d invalid (expected 1-10), skipping Level definitions",
                 nLevels);
        return;
    }

    CPLDebug("OGR_POLISHMAP", "Parsing %d Level definitions", nLevels);

    // Reserve space in vector for efficiency
    m_oHeaderData.aoLevelDefs.reserve(nLevels);

    // Extract Level0, Level1, ..., Level(nLevels-1) from aoOtherFields
    for (int i = 0; i < nLevels; i++) {
        CPLString osLevelKey;
        osLevelKey.Printf("Level%d", i);

        auto it = m_oHeaderData.aoOtherFields.find(osLevelKey);
        if (it != m_oHeaderData.aoOtherFields.end()) {
            // Found Level definition - store and remove from map
            m_oHeaderData.aoLevelDefs.push_back(it->second);
            m_oHeaderData.aoOtherFields.erase(it);

            CPLDebug("OGR_POLISHMAP", "  Level%d=%s", i, it->second.c_str());
        } else {
            // Level definition missing - log warning but continue
            CPLError(CE_Warning, CPLE_AppDefined,
                     "Polish Map header: Levels=%d but Level%d missing",
                     nLevels, i);
            // Add empty string to maintain index correspondence
            m_oHeaderData.aoLevelDefs.push_back("");
        }
    }
}

/************************************************************************/
/*                         ParseCoordinates()                           */
/*                                                                      */
/* Parse coordinates from Data0=(lat,lon) or Data0=lat,lon format.     */
/* Story 1.4: Handle both parenthesized and non-parenthesized formats.  */
/************************************************************************/

bool PolishMapParser::ParseCoordinates(const CPLString& osValue, double& dfLat, double& dfLon) {
    CPLString osClean = osValue;

    // Remove parentheses if present
    if (!osClean.empty() && osClean[0] == '(') {
        osClean = osClean.substr(1);
    }
    if (!osClean.empty() && osClean.back() == ')') {
        osClean.resize(osClean.size() - 1);
    }

    // Find comma separator
    const char* pszComma = strchr(osClean.c_str(), ',');
    if (pszComma == nullptr) {
        return false;
    }

    dfLat = CPLAtof(osClean.c_str());
    dfLon = CPLAtof(pszComma + 1);

    // Validate WGS84 range
    return dfLat >= -90.0 && dfLat <= 90.0 &&
           dfLon >= -180.0 && dfLon <= 180.0;
}

/************************************************************************/
/*                       ParseCoordinateList()                          */
/*                                                                      */
/* Parse a LIST of coordinates from Data0 line.                         */
/* Story 1.5 REFACTORING: Format correct selon spécification MP.        */
/* Supports: "(lat1,lon1),(lat2,lon2),..." OR "lat1,lon1,lat2,lon2,..."  */
/* Returns: number of points parsed (0 on error)                        */
/************************************************************************/

int PolishMapParser::ParseCoordinateList(const CPLString& osValue,
                                         std::vector<std::pair<double, double>>& aoCoords) {
    aoCoords.clear();

    const char* pszInput = osValue.c_str();

    // Detect format: parenthesized "(lat,lon),(lat,lon)" or plain "lat,lon,lat,lon"
    bool bParenthesizedFormat = (strchr(pszInput, '(') != nullptr);

    if (bParenthesizedFormat) {
        // Format: "(lat1,lon1),(lat2,lon2),..."
        while (*pszInput) {
            // Skip whitespace before coordinate pair
            while (*pszInput && (*pszInput == ' ' || *pszInput == '\t')) pszInput++;
            if (!*pszInput) break;

            // Skip comma separator between pairs (only after we have at least one pair)
            // Note: Leading comma before first coordinate is an error, handled by '(' check below
            if (*pszInput == ',' && !aoCoords.empty()) {
                pszInput++;
                // Skip whitespace after comma
                while (*pszInput && (*pszInput == ' ' || *pszInput == '\t')) pszInput++;
            }
            if (!*pszInput) break;

            // Expect '('
            if (*pszInput != '(') {
                // Try to recover - might be end of valid data
                if (aoCoords.size() >= 2) break;
                return 0;  // Error: expected '('
            }
            pszInput++;  // Skip '('

            // Parse latitude
            char* pszEnd;
            double dfLat = CPLStrtod(pszInput, &pszEnd);
            if (pszEnd == pszInput) {
                return 0;  // Error: no latitude
            }
            pszInput = pszEnd;

            // Skip comma between lat and lon
            while (*pszInput && (*pszInput == ' ' || *pszInput == '\t')) pszInput++;
            if (*pszInput != ',') {
                return 0;  // Error: expected comma
            }
            pszInput++;  // Skip ','

            // Parse longitude
            double dfLon = CPLStrtod(pszInput, &pszEnd);
            if (pszEnd == pszInput) {
                return 0;  // Error: no longitude
            }
            pszInput = pszEnd;

            // Skip to closing ')'
            while (*pszInput && (*pszInput == ' ' || *pszInput == '\t')) pszInput++;
            if (*pszInput != ')') {
                return 0;  // Error: expected ')'
            }
            pszInput++;  // Skip ')'

            // Validate WGS84 range
            if (dfLat < -90.0 || dfLat > 90.0 || dfLon < -180.0 || dfLon > 180.0) {
                CPLError(CE_Warning, CPLE_AppDefined,
                         "Coordinate out of WGS84 range: (%f, %f)", dfLat, dfLon);
                return 0;
            }

            aoCoords.push_back({dfLat, dfLon});
        }
    } else {
        // Format: "lat1,lon1,lat2,lon2,..." (plain comma-separated)
        while (*pszInput) {
            // Skip whitespace
            while (*pszInput && (*pszInput == ' ' || *pszInput == '\t')) pszInput++;
            if (!*pszInput) break;

            // Parse latitude
            char* pszEnd;
            double dfLat = CPLStrtod(pszInput, &pszEnd);
            if (pszEnd == pszInput) {
                break;  // No more numbers
            }
            pszInput = pszEnd;

            // Skip comma between lat and lon
            while (*pszInput && (*pszInput == ' ' || *pszInput == '\t')) pszInput++;
            if (*pszInput != ',') {
                return 0;  // Error: expected comma between lat and lon
            }
            pszInput++;  // Skip ','

            // Parse longitude
            double dfLon = CPLStrtod(pszInput, &pszEnd);
            if (pszEnd == pszInput) {
                return 0;  // Error: no longitude
            }
            pszInput = pszEnd;

            // Validate WGS84 range
            if (dfLat < -90.0 || dfLat > 90.0 || dfLon < -180.0 || dfLon > 180.0) {
                CPLError(CE_Warning, CPLE_AppDefined,
                         "Coordinate out of WGS84 range: (%f, %f)", dfLat, dfLon);
                return 0;
            }

            aoCoords.push_back({dfLat, dfLon});

            // Skip comma separator between coordinate pairs (if any)
            while (*pszInput && (*pszInput == ' ' || *pszInput == '\t')) pszInput++;
            if (*pszInput == ',') {
                pszInput++;  // Skip separator comma
            }
        }
    }

    return static_cast<int>(aoCoords.size());
}

/************************************************************************/
/*                       ResetSectionReading()                          */
/*                                                                      */
/* Reset file position to start of data sections (after header).        */
/* REFACTORING: Unified method replacing Reset*Reading() duplicates.    */
/* Note: All layers share the same file position after header.          */
/************************************************************************/

void PolishMapParser::ResetSectionReading() {
    if (m_fpFile != nullptr) {
        VSIFSeekL(m_fpFile, m_nAfterHeaderPos, SEEK_SET);
        // Note: m_nCurrentLine is NOT reset - it tracks absolute line position
        // for accurate error reporting across multiple iterations
    }
}

/************************************************************************/
/*                        ParseNextSection()                            */
/*                                                                      */
/* REFACTORING: Unified section parsing method (DRY pattern).           */
/* Replaces ParseNextPOI, ParseNextPolyline, ParseNextPolygon.          */
/* State machine to find and parse sections of specified type.          */
/************************************************************************/

bool PolishMapParser::ParseNextSection(SectionType eTargetType, PolishMapSection& oSection) {
    if (m_fpFile == nullptr) {
        return false;
    }

    oSection.Clear();
    oSection.eType = eTargetType;

    const char* pszTypeName = oSection.GetTypeName();
    const int nMinPoints = oSection.GetMinPointCount();

    CPLString osLine;
    bool bInTargetSection = false;
    bool bInOtherSection = false;  // CRITICAL FLAG to skip non-target sections

    // Read file line by line
    while (ReadLine(osLine)) {
        m_nCurrentLine++;

        // Check for section markers
        if (!osLine.empty() && osLine[0] == '[') {
            // Check for target section marker
            bool bIsTargetSection = false;
            if (eTargetType == SectionType::POI) {
                bIsTargetSection = STARTS_WITH_CI(osLine.c_str(), "[POI]") ||
                                   STARTS_WITH_CI(osLine.c_str(), "[RGN10]");
            } else if (eTargetType == SectionType::Polyline) {
                bIsTargetSection = STARTS_WITH_CI(osLine.c_str(), "[POLYLINE]");
            } else if (eTargetType == SectionType::Polygon) {
                bIsTargetSection = STARTS_WITH_CI(osLine.c_str(), "[POLYGON]");
            }

            if (bIsTargetSection) {
                bInTargetSection = true;
                bInOtherSection = false;
                continue;
            } else if (STARTS_WITH_CI(osLine.c_str(), "[END]")) {
                if (bInTargetSection) {
                    // End of current target section - validate and return
                    if (static_cast<int>(oSection.aoCoords.size()) < nMinPoints) {
                        CPLError(CE_Warning, CPLE_AppDefined,
                                 "%s at line %d has less than %d points (%zu found), skipping",
                                 pszTypeName, m_nCurrentLine, nMinPoints, oSection.aoCoords.size());
                        oSection.Clear();
                        oSection.eType = eTargetType;
                        bInTargetSection = false;
                        continue;  // Skip this section, look for next
                    }

                    // Story 3.2 AC3: Minor Issues - Log missing optional fields with CPLDebug
                    // Pattern: Default value + CPLDebug (NFR10: clear diagnostic messages)
                    if (oSection.osLabel.empty()) {
                        CPLDebug("OGR_POLISHMAP",
                                 "%s at line %d has no Label, using empty string",
                                 pszTypeName, m_nCurrentLine);
                    }
                    if (oSection.nEndLevel < 0) {
                        CPLDebug("OGR_POLISHMAP",
                                 "%s at line %d has no EndLevel, using default -1",
                                 pszTypeName, m_nCurrentLine);
                    }

                    return true;
                }
                // End of some other section - reset flag and continue searching
                bInOtherSection = false;
                continue;
            } else if (STARTS_WITH_CI(osLine.c_str(), "[IMG ID]") ||
                       STARTS_WITH_CI(osLine.c_str(), "[END-IMG ID]")) {
                // Header section markers - skip
                bInOtherSection = false;
                continue;
            } else {
                // It's a different section marker
                if (bInTargetSection) {
                    // Another section started within target (shouldn't happen normally)
                    CPLError(CE_Warning, CPLE_AppDefined,
                             "Unexpected section marker within [%s] at line %d",
                             pszTypeName, m_nCurrentLine);
                    return false;
                }
                // We're now in a non-target section - skip until [END]
                bInOtherSection = true;
                continue;
            }
        }

        // Skip lines if we're in a non-target section
        if (bInOtherSection) {
            continue;
        }

        // Parse key=value pairs within target section
        if (bInTargetSection) {
            CPLString osKey, osValue;
            if (ParseKeyValue(osLine, osKey, osValue)) {
                // Store known fields
                if (EQUAL(osKey.c_str(), "Type")) {
                    // Story 3.2 Task 2.5: Validate Type code format (AC2 Recoverable Error)
                    // Valid formats: "0x0001", "0x2C00", etc. (hex) or decimal "1234"
                    // Invalid: "0xZZZZ", "notahex", empty string
                    bool bValidType = false;
                    if (!osValue.empty()) {
                        if (STARTS_WITH_CI(osValue.c_str(), "0x")) {
                            // Hex format: validate all chars after "0x" are hex digits
                            const char* pszHex = osValue.c_str() + 2;
                            bValidType = (*pszHex != '\0');  // Must have at least one digit
                            while (*pszHex && bValidType) {
                                if (!isxdigit(static_cast<unsigned char>(*pszHex))) {
                                    bValidType = false;
                                }
                                pszHex++;
                            }
                        } else {
                            // Decimal format: validate all chars are digits
                            const char* pszDec = osValue.c_str();
                            bValidType = true;
                            while (*pszDec && bValidType) {
                                if (!isdigit(static_cast<unsigned char>(*pszDec))) {
                                    bValidType = false;
                                }
                                pszDec++;
                            }
                        }
                    }
                    if (!bValidType) {
                        CPLError(CE_Warning, CPLE_AppDefined,
                                 "%s at line %d has invalid Type code '%s', using as-is",
                                 pszTypeName, m_nCurrentLine, osValue.c_str());
                    }
                    oSection.osType = osValue;
                } else if (EQUAL(osKey.c_str(), "Label")) {
                    oSection.osLabel = RecodeToUTF8(osValue);
                } else if (EQUAL(osKey.c_str(), "Data0")) {
                    // POI: single coordinate, POLYLINE/POLYGON: coordinate list
                    if (eTargetType == SectionType::POI) {
                        double dfLat, dfLon;
                        if (!ParseCoordinates(osValue, dfLat, dfLon)) {
                            CPLError(CE_Warning, CPLE_AppDefined,
                                     "Skipping %s at line %d: invalid coordinates '%s'",
                                     pszTypeName, m_nCurrentLine, osValue.c_str());
                            bInTargetSection = false;
                            oSection.Clear();
                            oSection.eType = eTargetType;
                            continue;
                        }
                        oSection.aoCoords.push_back({dfLat, dfLon});
                    } else {
                        // POLYLINE/POLYGON: coordinate list
                        int nPoints = ParseCoordinateList(osValue, oSection.aoCoords);
                        if (nPoints == 0) {
                            CPLError(CE_Warning, CPLE_AppDefined,
                                     "Skipping %s at line %d: invalid coordinates in Data0='%s'",
                                     pszTypeName, m_nCurrentLine, osValue.c_str());
                            bInTargetSection = false;
                            oSection.Clear();
                            oSection.eType = eTargetType;
                            continue;
                        }
                    }
                } else if (STARTS_WITH_CI(osKey.c_str(), "Data") && !EQUAL(osKey.c_str(), "Data0")) {
                    // Data1, Data2, etc. are multi-resolution levels (ignored in MVP)
                    if (eTargetType != SectionType::POI) {
                        CPLDebug("OGR_POLISHMAP", "Ignoring %s (multi-resolution not yet supported)",
                                 osKey.c_str());
                    }
                } else if (EQUAL(osKey.c_str(), "EndLevel")) {
                    oSection.nEndLevel = atoi(osValue.c_str());
                } else if (EQUAL(osKey.c_str(), "Levels")) {
                    oSection.osLevels = osValue;
                } else {
                    // Store other fields in the map
                    oSection.aoOtherFields[osKey] = osValue;
                }
            }
        }
    }

    // Reached end of file
    if (bInTargetSection) {
        // We were in a target section but didn't find [END] marker
        // Validate and accept it anyway (tolerant parsing)
        if (static_cast<int>(oSection.aoCoords.size()) >= nMinPoints) {
            // Story 3.2: Log missing [END] marker as minor issue
            CPLDebug("OGR_POLISHMAP",
                     "%s at EOF has no [END] marker, accepting anyway",
                     pszTypeName);

            // Story 3.2 AC3: Minor Issues - Log missing optional fields
            if (oSection.osLabel.empty()) {
                CPLDebug("OGR_POLISHMAP",
                         "%s at EOF has no Label, using empty string",
                         pszTypeName);
            }
            if (oSection.nEndLevel < 0) {
                CPLDebug("OGR_POLISHMAP",
                         "%s at EOF has no EndLevel, using default -1",
                         pszTypeName);
            }

            return true;
        }
    }

    return false;
}

/************************************************************************/
/*                          ParseNextPOI()                              */
/*                                                                      */
/* Parse next [POI] section from file.                                  */
/* REFACTORING: Wrapper around ParseNextSection() for backward compat.  */
/************************************************************************/

bool PolishMapParser::ParseNextPOI(PolishMapPOISection& oSection) {
    oSection.Clear();

    PolishMapSection oGeneric(SectionType::POI);
    if (!ParseNextSection(SectionType::POI, oGeneric)) {
        return false;
    }

    // Convert PolishMapSection -> PolishMapPOISection
    oSection.osType = oGeneric.osType;
    oSection.osLabel = oGeneric.osLabel;
    oSection.oCoords = oGeneric.aoCoords.empty() ?
                       std::make_pair(0.0, 0.0) : oGeneric.aoCoords[0];
    oSection.nEndLevel = oGeneric.nEndLevel;
    oSection.osLevels = oGeneric.osLevels;
    oSection.aoOtherFields = oGeneric.aoOtherFields;

    return true;
}

/************************************************************************/
/*                        ParseNextPolyline()                           */
/*                                                                      */
/* Parse next [POLYLINE] section from file.                             */
/* REFACTORING: Wrapper around ParseNextSection() for backward compat.  */
/************************************************************************/

bool PolishMapParser::ParseNextPolyline(PolishMapPolylineSection& oSection) {
    oSection.Clear();

    PolishMapSection oGeneric(SectionType::Polyline);
    if (!ParseNextSection(SectionType::Polyline, oGeneric)) {
        return false;
    }

    // Convert PolishMapSection -> PolishMapPolylineSection
    oSection.osType = oGeneric.osType;
    oSection.osLabel = oGeneric.osLabel;
    oSection.aoCoords = oGeneric.aoCoords;
    oSection.nEndLevel = oGeneric.nEndLevel;
    oSection.osLevels = oGeneric.osLevels;
    oSection.aoOtherFields = oGeneric.aoOtherFields;

    return true;
}

/************************************************************************/
/*                        ParseNextPolygon()                            */
/*                                                                      */
/* Parse next [POLYGON] section from file.                              */
/* REFACTORING: Wrapper around ParseNextSection() for backward compat.  */
/************************************************************************/

bool PolishMapParser::ParseNextPolygon(PolishMapPolygonSection& oSection) {
    oSection.Clear();

    PolishMapSection oGeneric(SectionType::Polygon);
    if (!ParseNextSection(SectionType::Polygon, oGeneric)) {
        return false;
    }

    // Convert PolishMapSection -> PolishMapPolygonSection
    oSection.osType = oGeneric.osType;
    oSection.osLabel = oGeneric.osLabel;
    oSection.aoCoords = oGeneric.aoCoords;
    oSection.nEndLevel = oGeneric.nEndLevel;
    oSection.osLevels = oGeneric.osLevels;
    oSection.aoOtherFields = oGeneric.aoOtherFields;

    return true;
}
