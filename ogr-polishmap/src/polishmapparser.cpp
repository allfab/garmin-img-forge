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
    : m_osFilePath(pszFilePath), m_fpFile(nullptr) {
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

    return true;
}
