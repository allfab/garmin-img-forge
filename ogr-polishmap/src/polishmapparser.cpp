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
/************************************************************************/

PolishMapParser::PolishMapParser(const char* pszFilePath)
    : m_osFilePath(pszFilePath), m_fpFile(nullptr), m_nAfterHeaderPos(0), m_nCurrentLine(0) {
    m_fpFile = VSIFOpenL(pszFilePath, "rb");
    if (m_fpFile == nullptr) {
        CPLDebug("OGR_POLISHMAP", "Failed to open file: %s", pszFilePath);
    }
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

    // Parse file line by line
    while (ReadLine(osLine)) {
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
                } else {
                    // Store other fields in the map
                    m_oHeaderData.aoOtherFields[osKey] = osValue;
                }
            }
        }
    }

    if (!bFoundImgIdSection) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "Polish Map file missing required [IMG ID] header");
        return false;
    }

    CPLDebug("OGR_POLISHMAP", "Parsed header: Name='%s', ID='%s', CodePage='%s', Datum='%s'",
             m_oHeaderData.osName.c_str(), m_oHeaderData.osID.c_str(),
             m_oHeaderData.osCodePage.c_str(), m_oHeaderData.osDatum.c_str());

    // Story 1.4: Save position after header for POI reading
    m_nAfterHeaderPos = VSIFTellL(m_fpFile);

    return true;
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
/*                          ResetPOIReading()                           */
/*                                                                      */
/* Reset file position to start of POI sections (after header).         */
/* Story 1.4: Allow re-iteration through POI sections.                  */
/************************************************************************/

void PolishMapParser::ResetPOIReading() {
    if (m_fpFile != nullptr) {
        VSIFSeekL(m_fpFile, m_nAfterHeaderPos, SEEK_SET);
        // Note: m_nCurrentLine is NOT reset - it tracks absolute line position
        // for accurate error reporting across multiple iterations
    }
}

/************************************************************************/
/*                       ResetPolylineReading()                         */
/*                                                                      */
/* Reset file position to start of POLYLINE sections (after header).    */
/* Story 1.5: Allow re-iteration through POLYLINE sections.             */
/* Note: Reuses same position as POI (m_nAfterHeaderPos) since all      */
/* layers share the same file and parse from the beginning.             */
/************************************************************************/

void PolishMapParser::ResetPolylineReading() {
    if (m_fpFile != nullptr) {
        VSIFSeekL(m_fpFile, m_nAfterHeaderPos, SEEK_SET);
        // Note: m_nCurrentLine is NOT reset - it tracks absolute line position
        // for accurate error reporting across multiple iterations
    }
}

/************************************************************************/
/*                          ParseNextPOI()                              */
/*                                                                      */
/* Parse next [POI] section from file.                                  */
/* Story 1.4: State machine to skip non-POI sections.                   */
/************************************************************************/

bool PolishMapParser::ParseNextPOI(PolishMapPOISection& oSection) {
    if (m_fpFile == nullptr) {
        return false;
    }

    oSection.Clear();

    CPLString osLine;
    bool bInPOISection = false;
    bool bInOtherSection = false;  // Track if we're in a non-POI section to skip

    // Read file line by line
    while (ReadLine(osLine)) {
        m_nCurrentLine++;

        // Check for section markers
        if (!osLine.empty() && osLine[0] == '[') {
            if (STARTS_WITH_CI(osLine.c_str(), "[POI]") || STARTS_WITH_CI(osLine.c_str(), "[RGN10]")) {
                bInPOISection = true;
                bInOtherSection = false;
                continue;
            } else if (STARTS_WITH_CI(osLine.c_str(), "[END]")) {
                if (bInPOISection) {
                    // End of current POI section - return it
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
                // It's a different section marker ([POLYLINE], [POLYGON], etc.)
                if (bInPOISection) {
                    // Another section started within POI (shouldn't happen normally)
                    CPLError(CE_Warning, CPLE_AppDefined,
                             "Unexpected section marker within [POI] at line %d",
                             m_nCurrentLine);
                    return false;
                }
                // We're now in a non-POI section - skip until [END]
                bInOtherSection = true;
                continue;
            }
        }

        // Skip lines if we're in a non-POI section
        if (bInOtherSection) {
            continue;
        }

        // Parse key=value pairs within [POI] section
        if (bInPOISection) {
            CPLString osKey, osValue;
            if (ParseKeyValue(osLine, osKey, osValue)) {
                // Store known fields
                if (EQUAL(osKey.c_str(), "Type")) {
                    oSection.osType = osValue;
                } else if (EQUAL(osKey.c_str(), "Label")) {
                    oSection.osLabel = RecodeToUTF8(osValue);
                } else if (EQUAL(osKey.c_str(), "Data0")) {
                    // Parse coordinates
                    if (!ParseCoordinates(osValue, oSection.oCoords.first, oSection.oCoords.second)) {
                        CPLError(CE_Warning, CPLE_AppDefined,
                                 "Skipping POI at line %d: invalid coordinates '%s'",
                                 m_nCurrentLine, osValue.c_str());
                        // Skip to next POI section
                        bInPOISection = false;
                        oSection.Clear();
                        continue;
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
    if (bInPOISection) {
        // We were in a POI section but didn't find [END] marker
        // Accept it anyway (tolerant parsing)
        return true;
    }

    return false;
}

/************************************************************************/
/*                        ParseNextPolyline()                           */
/*                                                                      */
/* Parse next [POLYLINE] section from file.                             */
/* Story 1.5: State machine to skip non-POLYLINE sections (POI/POLYGON).*/
/* LEÇON 1.4: FLAG bInOtherSection CRITIQUE pour éviter le bug de 1.4. */
/************************************************************************/

bool PolishMapParser::ParseNextPolyline(PolishMapPolylineSection& oSection) {
    if (m_fpFile == nullptr) {
        return false;
    }

    oSection.Clear();

    CPLString osLine;
    bool bInPolylineSection = false;
    bool bInOtherSection = false;  // FLAG CRITIQUE pour skipper POI/POLYGON

    // Read file line by line
    while (ReadLine(osLine)) {
        m_nCurrentLine++;

        // Check for section markers
        if (!osLine.empty() && osLine[0] == '[') {
            if (STARTS_WITH_CI(osLine.c_str(), "[POLYLINE]")) {
                bInPolylineSection = true;
                bInOtherSection = false;
                continue;
            } else if (STARTS_WITH_CI(osLine.c_str(), "[END]")) {
                if (bInPolylineSection) {
                    // End of current POLYLINE section - validate and return
                    // Must have at least 2 points for valid POLYLINE
                    if (oSection.aoCoords.size() < 2) {
                        CPLError(CE_Warning, CPLE_AppDefined,
                                 "POLYLINE at line %d has less than 2 points (%zu found), skipping",
                                 m_nCurrentLine, oSection.aoCoords.size());
                        oSection.Clear();
                        bInPolylineSection = false;
                        continue;  // Skip this POLYLINE, look for next
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
                // It's a different section marker ([POI], [POLYGON], [RGN*], etc.)
                if (bInPolylineSection) {
                    // Another section started within POLYLINE (shouldn't happen normally)
                    CPLError(CE_Warning, CPLE_AppDefined,
                             "Unexpected section marker within [POLYLINE] at line %d",
                             m_nCurrentLine);
                    return false;
                }
                // We're now in a non-POLYLINE section - skip until [END]
                bInOtherSection = true;
                continue;
            }
        }

        // SKIP toutes les lignes dans les sections non-POLYLINE
        if (bInOtherSection) {
            continue;
        }

        // Parse key=value pairs within [POLYLINE] section
        if (bInPolylineSection) {
            CPLString osKey, osValue;
            if (ParseKeyValue(osLine, osKey, osValue)) {
                // Store known fields
                if (EQUAL(osKey.c_str(), "Type")) {
                    oSection.osType = osValue;
                } else if (EQUAL(osKey.c_str(), "Label")) {
                    oSection.osLabel = RecodeToUTF8(osValue);
                } else if (STARTS_WITH_CI(osKey.c_str(), "Data")) {
                    // Parse coordinates: Data0, Data1, Data2, ..., DataN
                    double dfLat, dfLon;
                    if (!ParseCoordinates(osValue, dfLat, dfLon)) {
                        CPLError(CE_Warning, CPLE_AppDefined,
                                 "Skipping POLYLINE at line %d: invalid coordinates in %s='%s'",
                                 m_nCurrentLine, osKey.c_str(), osValue.c_str());
                        // Skip to next POLYLINE section
                        bInPolylineSection = false;
                        oSection.Clear();
                        continue;
                    }
                    oSection.aoCoords.push_back({dfLat, dfLon});
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
    if (bInPolylineSection) {
        // We were in a POLYLINE section but didn't find [END] marker
        // Validate and accept it anyway (tolerant parsing)
        if (oSection.aoCoords.size() >= 2) {
            return true;
        }
    }

    return false;
}

/************************************************************************/
/*                       ResetPolygonReading()                          */
/*                                                                      */
/* Reset file position to start of POLYGON sections (after header).     */
/* Story 1.6: Allow re-iteration through POLYGON sections.              */
/* Note: Reuses same position as POI/POLYLINE (m_nAfterHeaderPos) since */
/* all layers share the same file and parse from the beginning.         */
/************************************************************************/

void PolishMapParser::ResetPolygonReading() {
    if (m_fpFile != nullptr) {
        VSIFSeekL(m_fpFile, m_nAfterHeaderPos, SEEK_SET);
        // Note: m_nCurrentLine is NOT reset - it tracks absolute line position
        // for accurate error reporting across multiple iterations
    }
}

/************************************************************************/
/*                        ParseNextPolygon()                            */
/*                                                                      */
/* Parse next [POLYGON] section from file.                              */
/* Story 1.6: State machine to skip non-POLYGON sections (POI/POLYLINE).*/
/* LEÇON 1.4: FLAG bInOtherSection CRITIQUE pour éviter le bug de 1.4.  */
/************************************************************************/

bool PolishMapParser::ParseNextPolygon(PolishMapPolygonSection& oSection) {
    if (m_fpFile == nullptr) {
        return false;
    }

    oSection.Clear();

    CPLString osLine;
    bool bInPolygonSection = false;
    bool bInOtherSection = false;  // FLAG CRITIQUE pour skipper POI/POLYLINE

    // Read file line by line
    while (ReadLine(osLine)) {
        m_nCurrentLine++;

        // Check for section markers
        if (!osLine.empty() && osLine[0] == '[') {
            if (STARTS_WITH_CI(osLine.c_str(), "[POLYGON]")) {
                bInPolygonSection = true;
                bInOtherSection = false;
                continue;
            } else if (STARTS_WITH_CI(osLine.c_str(), "[END]")) {
                if (bInPolygonSection) {
                    // End of current POLYGON section - validate and return
                    // Must have at least 3 points for valid POLYGON
                    if (oSection.aoCoords.size() < 3) {
                        CPLError(CE_Warning, CPLE_AppDefined,
                                 "POLYGON at line %d has less than 3 points (%zu found), skipping",
                                 m_nCurrentLine, oSection.aoCoords.size());
                        oSection.Clear();
                        bInPolygonSection = false;
                        continue;  // Skip this POLYGON, look for next
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
                // It's a different section marker ([POI], [POLYLINE], [RGN*], etc.)
                if (bInPolygonSection) {
                    // Another section started within POLYGON (shouldn't happen normally)
                    CPLError(CE_Warning, CPLE_AppDefined,
                             "Unexpected section marker within [POLYGON] at line %d",
                             m_nCurrentLine);
                    return false;
                }
                // We're now in a non-POLYGON section - skip until [END]
                bInOtherSection = true;
                continue;
            }
        }

        // SKIP toutes les lignes dans les sections non-POLYGON
        if (bInOtherSection) {
            continue;
        }

        // Parse key=value pairs within [POLYGON] section
        if (bInPolygonSection) {
            CPLString osKey, osValue;
            if (ParseKeyValue(osLine, osKey, osValue)) {
                // Store known fields
                if (EQUAL(osKey.c_str(), "Type")) {
                    oSection.osType = osValue;
                } else if (EQUAL(osKey.c_str(), "Label")) {
                    oSection.osLabel = RecodeToUTF8(osValue);
                } else if (STARTS_WITH_CI(osKey.c_str(), "Data")) {
                    // Parse coordinates: Data0, Data1, Data2, ..., DataN
                    double dfLat, dfLon;
                    if (!ParseCoordinates(osValue, dfLat, dfLon)) {
                        CPLError(CE_Warning, CPLE_AppDefined,
                                 "Skipping POLYGON at line %d: invalid coordinates in %s='%s'",
                                 m_nCurrentLine, osKey.c_str(), osValue.c_str());
                        // Skip to next POLYGON section
                        bInPolygonSection = false;
                        oSection.Clear();
                        continue;
                    }
                    oSection.aoCoords.push_back({dfLat, dfLon});
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
    if (bInPolygonSection) {
        // We were in a POLYGON section but didn't find [END] marker
        // Validate and accept it anyway (tolerant parsing)
        if (oSection.aoCoords.size() >= 3) {
            return true;
        }
    }

    return false;
}
