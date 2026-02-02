/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Polish Map format writer - implementation
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

#include "polishmapwriter.h"
#include "cpl_error.h"
#include "cpl_conv.h"

// Default POI type code when Type field is not set
static const char* const DEFAULT_POI_TYPE = "0x0000";

/************************************************************************/
/*                          PolishMapWriter()                            */
/************************************************************************/

PolishMapWriter::PolishMapWriter(VSILFILE* fpOutput)
    : m_fpOutput(fpOutput)
    , m_bHeaderWritten(false)
{
    // File handle is borrowed - we don't own it
}

/************************************************************************/
/*                         ~PolishMapWriter()                            */
/************************************************************************/

PolishMapWriter::~PolishMapWriter()
{
    // Do NOT close file - it's a borrowed handle
    // Owner (OGRPolishMapDataSource) is responsible for closing
}

/************************************************************************/
/*                           WriteHeader()                               */
/*                                                                      */
/* Story 2.1 Task 3.2: Write minimal [IMG ID] header section.           */
/* Story 2.2 Review: Refactored to delegate to WriteHeader(map) to      */
/* avoid code duplication.                                               */
/* Output:                                                              */
/*   [IMG ID]                                                           */
/*   Name=<name>                                                        */
/*   CodePage=<codepage>                                                */
/*   [END]                                                              */
/************************************************************************/

bool PolishMapWriter::WriteHeader(const std::string& osName, const std::string& osCodePage)
{
    // Delegate to map-based overload to avoid code duplication
    std::map<std::string, std::string> aoMetadata;
    aoMetadata["Name"] = osName;
    aoMetadata["CodePage"] = osCodePage;
    return WriteHeader(aoMetadata);
}

/************************************************************************/
/*                              Flush()                                  */
/************************************************************************/

bool PolishMapWriter::Flush()
{
    if (m_fpOutput == nullptr) {
        return false;
    }

    return VSIFFlushL(m_fpOutput) == 0;
}

/************************************************************************/
/*                         RecodeToCP1252()                              */
/*                                                                      */
/* Story 2.2 Task 4: Convert UTF-8 string to CP1252 encoding.           */
/* Uses CPLRecode API. Falls back to original value on failure.          */
/************************************************************************/

std::string PolishMapWriter::RecodeToCP1252(const std::string& osUTF8Value)
{
    // Convert from UTF-8 to CP1252 for output
    char* pszCP1252 = CPLRecode(osUTF8Value.c_str(), "UTF-8", "CP1252");
    if (pszCP1252 == nullptr) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "Failed to convert string from UTF-8 to CP1252, using raw value: %s",
                 osUTF8Value.c_str());
        return osUTF8Value;  // Fallback to original
    }

    std::string osResult(pszCP1252);
    CPLFree(pszCP1252);  // CRITICAL: Always free CPLRecode result
    return osResult;
}

/************************************************************************/
/*                    WriteHeader(aoMetadata)                            */
/*                                                                      */
/* Story 2.2 Task 3: Write [IMG ID] header with metadata map.           */
/* Handles field ordering, default values, and UTF-8→CP1252 conversion. */
/************************************************************************/

bool PolishMapWriter::WriteHeader(const std::map<std::string, std::string>& aoMetadata)
{
    if (m_fpOutput == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "PolishMapWriter::WriteHeader() - file handle is null");
        return false;
    }

    if (m_bHeaderWritten) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "PolishMapWriter::WriteHeader() - header already written");
        return true;  // Not a fatal error
    }

    // Default values if not provided
    std::string osName = "Untitled";
    std::string osCodePage = "1252";

    // Extract known fields from metadata
    auto itName = aoMetadata.find("Name");
    if (itName != aoMetadata.end()) {
        osName = RecodeToCP1252(itName->second);  // UTF-8 → CP1252
    }

    auto itCodePage = aoMetadata.find("CodePage");
    if (itCodePage != aoMetadata.end()) {
        osCodePage = itCodePage->second;  // CodePage is numeric, no recode needed
    }

    // Write [IMG ID] section
    if (VSIFPrintfL(m_fpOutput, "[IMG ID]\n") < 0) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeader() - failed to write [IMG ID]");
        return false;
    }

    // Write Name field first (always present)
    if (VSIFPrintfL(m_fpOutput, "Name=%s\n", osName.c_str()) < 0) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeader() - failed to write Name");
        return false;
    }

    // Write other metadata fields (ID, Elevation, Preprocess, etc.)
    // Ordered set of known fields to write in logical order
    static const char* const apszKnownFields[] = {
        "ID", "Elevation", "Preprocess", nullptr
    };

    for (int i = 0; apszKnownFields[i] != nullptr; i++) {
        auto it = aoMetadata.find(apszKnownFields[i]);
        if (it != aoMetadata.end()) {
            std::string osValue = RecodeToCP1252(it->second);
            if (VSIFPrintfL(m_fpOutput, "%s=%s\n", it->first.c_str(), osValue.c_str()) < 0) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "PolishMapWriter::WriteHeader() - failed to write %s",
                         it->first.c_str());
                return false;
            }
        }
    }

    // Write any remaining custom fields (not in known list)
    for (const auto& kv : aoMetadata) {
        // Skip already-written fields
        if (kv.first == "Name" || kv.first == "CodePage") {
            continue;
        }
        bool bIsKnown = false;
        for (int i = 0; apszKnownFields[i] != nullptr; i++) {
            if (kv.first == apszKnownFields[i]) {
                bIsKnown = true;
                break;
            }
        }
        if (!bIsKnown) {
            std::string osValue = RecodeToCP1252(kv.second);
            if (VSIFPrintfL(m_fpOutput, "%s=%s\n", kv.first.c_str(), osValue.c_str()) < 0) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "PolishMapWriter::WriteHeader() - failed to write %s",
                         kv.first.c_str());
                return false;
            }
        }
    }

    // CodePage always last before [END]
    if (VSIFPrintfL(m_fpOutput, "CodePage=%s\n", osCodePage.c_str()) < 0) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeader() - failed to write CodePage");
        return false;
    }

    // Write [END] marker
    if (VSIFPrintfL(m_fpOutput, "[END]\n") < 0) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeader() - failed to write [END]");
        return false;
    }

    m_bHeaderWritten = true;

    CPLDebug("OGR_POLISHMAP", "WriteHeader: Name=%s, CodePage=%s, %d fields total",
             osName.c_str(), osCodePage.c_str(),
             static_cast<int>(aoMetadata.size()));

    return true;
}

/************************************************************************/
/*                            WritePOI()                                 */
/*                                                                      */
/* Story 2.3 Task 2: Write POI feature to output file.                   */
/* Format:                                                               */
/*   [POI]                                                               */
/*   Type=<type_code>                                                    */
/*   Label=<label>       (optional, UTF-8 → CP1252)                      */
/*   Data0=(lat,lon)     (6 decimal precision)                           */
/*   EndLevel=<level>    (optional)                                      */
/*   [END-POI]                                                           */
/************************************************************************/

bool PolishMapWriter::WritePOI(OGRFeature* poFeature)
{
    // Task 2.2: Validate input
    if (poFeature == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOI: NULL feature pointer");
        return false;
    }

    if (m_fpOutput == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOI: file handle is null");
        return false;
    }

    // Ensure header is written before first feature
    // If not written yet, write with default values
    if (!m_bHeaderWritten) {
        std::map<std::string, std::string> aoDefaultMetadata;
        aoDefaultMetadata["Name"] = "Untitled";
        aoDefaultMetadata["CodePage"] = "1252";
        if (!WriteHeader(aoDefaultMetadata)) {
            CPLError(CE_Failure, CPLE_FileIO,
                     "WritePOI: failed to write header");
            return false;
        }
    }

    // Task 2.2: Extract and verify geometry is Point
    OGRGeometry* poGeom = poFeature->GetGeometryRef();
    if (poGeom == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOI: feature has no geometry");
        return false;
    }

    if (wkbFlatten(poGeom->getGeometryType()) != wkbPoint) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOI: feature geometry is not Point (type=%d)",
                 static_cast<int>(poGeom->getGeometryType()));
        return false;
    }

    OGRPoint* poPoint = poGeom->toPoint();
    if (poPoint == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOI: failed to cast geometry to Point");
        return false;
    }

    // Write [POI] section marker
    if (VSIFPrintfL(m_fpOutput, "[POI]\n") < 0) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "WritePOI: failed to write [POI] marker");
        return false;
    }

    // Task 2.4: Extract and write Type field (required)
    const char* pszType = poFeature->GetFieldAsString("Type");
    if (pszType != nullptr && pszType[0] != '\0') {
        if (VSIFPrintfL(m_fpOutput, "Type=%s\n", pszType) < 0) {
            CPLError(CE_Failure, CPLE_FileIO, "WritePOI: failed to write Type");
            return false;
        }
    } else {
        // Default POI type
        if (VSIFPrintfL(m_fpOutput, "Type=%s\n", DEFAULT_POI_TYPE) < 0) {
            CPLError(CE_Failure, CPLE_FileIO, "WritePOI: failed to write default Type");
            return false;
        }
    }

    // Task 2.4, 2.5, 2.7: Extract and write Label field (optional)
    const char* pszLabel = poFeature->GetFieldAsString("Label");
    if (pszLabel != nullptr && pszLabel[0] != '\0') {
        // Task 2.5: Convert UTF-8 to CP1252
        std::string osLabelCP1252 = RecodeToCP1252(pszLabel);
        if (VSIFPrintfL(m_fpOutput, "Label=%s\n", osLabelCP1252.c_str()) < 0) {
            CPLError(CE_Failure, CPLE_FileIO, "WritePOI: failed to write Label");
            return false;
        }
    }
    // Task 2.7: If Label is empty/null, omit the line entirely

    // Task 2.3: Write Data0 with 6 decimal precision
    // CRITICAL: Polish Map format uses (lat, lon) order, NOT (lon, lat)!
    // OGRPoint: getX() = longitude, getY() = latitude
    double dfLat = poPoint->getY();
    double dfLon = poPoint->getX();

    if (VSIFPrintfL(m_fpOutput, "Data0=(%.6f,%.6f)\n", dfLat, dfLon) < 0) {
        CPLError(CE_Failure, CPLE_FileIO, "WritePOI: failed to write Data0");
        return false;
    }

    // Task 2.4, 2.7: Extract and write EndLevel field (optional)
    int nEndLevelIdx = poFeature->GetFieldIndex("EndLevel");
    if (nEndLevelIdx >= 0 && poFeature->IsFieldSetAndNotNull(nEndLevelIdx)) {
        int nEndLevel = poFeature->GetFieldAsInteger("EndLevel");
        if (VSIFPrintfL(m_fpOutput, "EndLevel=%d\n", nEndLevel) < 0) {
            CPLError(CE_Failure, CPLE_FileIO, "WritePOI: failed to write EndLevel");
            return false;
        }
    }

    // Task 2.6: Write [END] marker (Polish Map format standard)
    // Note: Polish Map uses [END] for all sections, not [END-POI]
    if (VSIFPrintfL(m_fpOutput, "[END]\n\n") < 0) {
        CPLError(CE_Failure, CPLE_FileIO, "WritePOI: failed to write [END]");
        return false;
    }

    CPLDebug("OGR_POLISHMAP", "WritePOI: Type=%s, Label=%s, (%.6f,%.6f)",
             pszType ? pszType : "(default)",
             pszLabel ? pszLabel : "(null)",
             dfLat, dfLon);

    return true;
}
