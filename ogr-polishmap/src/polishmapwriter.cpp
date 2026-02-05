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
#include <cstdarg>  // Story 3.1: For FormatString() helper

// Default POI type code when Type field is not set
static const char* const DEFAULT_POI_TYPE = "0x0000";

// Default POLYLINE type code when Type field is not set (road type)
static const char* const DEFAULT_POLYLINE_TYPE = "0x0001";

// Default POLYGON type code when Type field is not set (area type)
static const char* const DEFAULT_POLYGON_TYPE = "0x0001";

// Tolerance for ring closure detection (same as OGRPolishMapLayer::RING_CLOSURE_TOLERANCE)
static constexpr double RING_CLOSURE_TOLERANCE = 1e-9;

// Warning threshold for very large polygons (performance consideration)
static constexpr int LARGE_POLYGON_WARNING_THRESHOLD = 10000;

/************************************************************************/
/*                          PolishMapWriter()                            */
/*                                                                      */
/* Story 3.1: Initialize buffered writing (Architecture: Buffered I/O)   */
/************************************************************************/

PolishMapWriter::PolishMapWriter(VSILFILE* fpOutput)
    : m_fpOutput(fpOutput)
    , m_bHeaderWritten(false)
{
    // File handle is borrowed - we don't own it
    // Story 3.1: Reserve buffer capacity to avoid reallocations
    m_osWriteBuffer.reserve(WRITER_BUFFER_SIZE);
    CPLDebug("OGR_POLISHMAP", "Writer initialized with %zu byte write buffer",
             WRITER_BUFFER_SIZE);
}

/************************************************************************/
/*                         ~PolishMapWriter()                            */
/*                                                                      */
/* Story 3.1: Flush remaining buffer data before destruction             */
/************************************************************************/

PolishMapWriter::~PolishMapWriter()
{
    // Story 3.1: Flush any remaining buffered data
    if (!m_osWriteBuffer.empty() && m_fpOutput != nullptr) {
        FlushBuffer();
    }
    // Do NOT close file - it's a borrowed handle
    // Owner (OGRPolishMapDataSource) is responsible for closing
}

/************************************************************************/
/*                          BufferedWrite()                              */
/*                                                                      */
/* Story 3.1: Accumulate writes in buffer to reduce syscalls (NFR2)      */
/* Flushes automatically when buffer reaches WRITER_BUFFER_SIZE          */
/************************************************************************/

bool PolishMapWriter::BufferedWrite(const char* pszData)
{
    if (pszData == nullptr) {
        return true;  // Nothing to write
    }

    m_osWriteBuffer += pszData;

    // Flush if buffer exceeds threshold
    if (m_osWriteBuffer.size() >= WRITER_BUFFER_SIZE) {
        return FlushBuffer();
    }

    return true;
}

/************************************************************************/
/*                     BufferedPrintf() - static helper                  */
/*                                                                      */
/* Story 3.1: Printf-style wrapper for buffered writing                  */
/* Used internally to replace VSIFPrintfL calls                          */
/************************************************************************/

static std::string FormatString(const char* pszFormat, ...)
{
    va_list args;
    va_start(args, pszFormat);

    // First pass: get required size
    va_list args_copy;
    va_copy(args_copy, args);
    int nLen = vsnprintf(nullptr, 0, pszFormat, args_copy);
    va_end(args_copy);

    if (nLen < 0) {
        va_end(args);
        return "";
    }

    // Second pass: format into string
    std::string osResult(static_cast<size_t>(nLen) + 1, '\0');
    vsnprintf(&osResult[0], osResult.size(), pszFormat, args);
    va_end(args);

    osResult.resize(static_cast<size_t>(nLen));  // Remove null terminator
    return osResult;
}

/************************************************************************/
/*                          FlushBuffer()                                */
/*                                                                      */
/* Story 3.1: Write accumulated buffer to file (Architecture: I/O)       */
/************************************************************************/

bool PolishMapWriter::FlushBuffer()
{
    if (m_osWriteBuffer.empty()) {
        return true;  // Nothing to flush
    }

    if (m_fpOutput == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "FlushBuffer: file handle is null");
        return false;
    }

    size_t nWritten = VSIFWriteL(m_osWriteBuffer.c_str(), 1,
                                  m_osWriteBuffer.size(), m_fpOutput);

    if (nWritten != m_osWriteBuffer.size()) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "FlushBuffer: wrote %zu of %zu bytes",
                 nWritten, m_osWriteBuffer.size());
        return false;
    }

    m_osWriteBuffer.clear();
    return true;
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
/*                                                                      */
/* Story 3.1: Flush internal buffer + VSI flush (NFR2 performance)       */
/************************************************************************/

bool PolishMapWriter::Flush()
{
    if (m_fpOutput == nullptr) {
        return false;
    }

    // Story 3.1: Flush internal write buffer first
    if (!FlushBuffer()) {
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
/* Story 3.1: Updated to use buffered writing (NFR2 performance)         */
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

    // Story 3.1: Use buffered writing for performance (NFR2)
    // Write [IMG ID] section
    if (!BufferedWrite("[IMG ID]\n")) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeader() - failed to write [IMG ID]");
        return false;
    }

    // Write Name field first (always present)
    if (!BufferedWrite(FormatString("Name=%s\n", osName.c_str()).c_str())) {
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
            if (!BufferedWrite(FormatString("%s=%s\n", it->first.c_str(), osValue.c_str()).c_str())) {
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
            if (!BufferedWrite(FormatString("%s=%s\n", kv.first.c_str(), osValue.c_str()).c_str())) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "PolishMapWriter::WriteHeader() - failed to write %s",
                         kv.first.c_str());
                return false;
            }
        }
    }

    // CodePage always last before [END]
    if (!BufferedWrite(FormatString("CodePage=%s\n", osCodePage.c_str()).c_str())) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeader() - failed to write CodePage");
        return false;
    }

    // Write [END] marker
    if (!BufferedWrite("[END]\n")) {
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
/*                         WriteSinglePOI()                              */
/*                                                                      */
/* Story 4.2 Task 4.1: Write a single Point geometry as [POI] section.   */
/* Extracted from WritePOI() to enable MultiPoint decomposition.         */
/* Story 3.1: Uses buffered writing (NFR2 performance)                   */
/* Format:                                                               */
/*   [POI]                                                               */
/*   Type=<type_code>                                                    */
/*   Label=<label>       (optional, UTF-8 → CP1252)                      */
/*   Data0=(lat,lon)     (6 decimal precision)                           */
/*   EndLevel=<level>    (optional)                                      */
/*   [END]                                                               */
/************************************************************************/

bool PolishMapWriter::WriteSinglePOI(OGRPoint* poPoint, OGRFeature* poFeature)
{
    if (poPoint == nullptr || poFeature == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WriteSinglePOI: NULL pointer");
        return false;
    }

    // Write [POI] section marker
    if (!BufferedWrite("[POI]\n")) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "WriteSinglePOI: failed to write [POI] marker");
        return false;
    }

    // Extract and write Type field (required)
    const char* pszType = poFeature->GetFieldAsString("Type");
    if (pszType != nullptr && pszType[0] != '\0') {
        if (!BufferedWrite(FormatString("Type=%s\n", pszType).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOI: failed to write Type");
            return false;
        }
    } else {
        // Default POI type
        if (!BufferedWrite(FormatString("Type=%s\n", DEFAULT_POI_TYPE).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOI: failed to write default Type");
            return false;
        }
    }

    // Extract and write Label field (optional)
    const char* pszLabel = poFeature->GetFieldAsString("Label");
    if (pszLabel != nullptr && pszLabel[0] != '\0') {
        std::string osLabelCP1252 = RecodeToCP1252(pszLabel);
        if (!BufferedWrite(FormatString("Label=%s\n", osLabelCP1252.c_str()).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOI: failed to write Label");
            return false;
        }
    }

    // Write Data0 with 6 decimal precision
    // CRITICAL: Polish Map format uses (lat, lon) order, NOT (lon, lat)!
    double dfLat = poPoint->getY();
    double dfLon = poPoint->getX();

    if (!BufferedWrite(FormatString("Data0=(%.6f,%.6f)\n", dfLat, dfLon).c_str())) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOI: failed to write Data0");
        return false;
    }

    // Extract and write EndLevel field (optional)
    int nEndLevelIdx = poFeature->GetFieldIndex("EndLevel");
    if (nEndLevelIdx >= 0 && poFeature->IsFieldSetAndNotNull(nEndLevelIdx)) {
        int nEndLevel = poFeature->GetFieldAsInteger("EndLevel");
        if (!BufferedWrite(FormatString("EndLevel=%d\n", nEndLevel).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOI: failed to write EndLevel");
            return false;
        }
    }

    // Write [END] marker
    if (!BufferedWrite("[END]\n\n")) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOI: failed to write [END]");
        return false;
    }

    CPLDebug("OGR_POLISHMAP", "WriteSinglePOI: Type=%s, Label=%s, (%.6f,%.6f)",
             pszType ? pszType : "(default)",
             pszLabel ? pszLabel : "(null)",
             dfLat, dfLon);

    return true;
}

/************************************************************************/
/*                            WritePOI()                                 */
/*                                                                      */
/* Story 2.3 Task 2: Write POI feature to output file.                   */
/* Story 4.2: Updated to handle wkbMultiPoint decomposition (AC3).       */
/* Story 3.1: Uses buffered writing (NFR2 performance)                   */
/************************************************************************/

bool PolishMapWriter::WritePOI(OGRFeature* poFeature)
{
    // Validate input
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

    // Extract geometry
    OGRGeometry* poGeom = poFeature->GetGeometryRef();
    if (poGeom == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOI: feature has no geometry");
        return false;
    }

    OGRwkbGeometryType eType = wkbFlatten(poGeom->getGeometryType());

    // Story 4.2 Task 1: Handle wkbMultiPoint - decompose into multiple [POI] sections
    if (eType == wkbMultiPoint) {
        OGRMultiPoint* poMulti = poGeom->toMultiPoint();
        if (poMulti == nullptr) {
            CPLError(CE_Failure, CPLE_AppDefined,
                     "WritePOI: failed to cast geometry to MultiPoint");
            return false;
        }

        int nParts = poMulti->getNumGeometries();
        int nWritten = 0;  // M1 Fix: Count written parts
        CPLDebug("OGR_POLISHMAP", "WritePOI: Decomposing MultiPoint with %d parts", nParts);

        for (int i = 0; i < nParts; i++) {
            OGRGeometry* poPartGeom = poMulti->getGeometryRef(i);
            if (poPartGeom == nullptr) {
                CPLDebug("OGR_POLISHMAP", "WritePOI: Skipping NULL part %d/%d", i + 1, nParts);
                continue;
            }

            OGRPoint* poPart = poPartGeom->toPoint();
            if (poPart == nullptr) {
                CPLDebug("OGR_POLISHMAP", "WritePOI: Skipping non-Point part %d/%d", i + 1, nParts);
                continue;
            }

            if (!WriteSinglePOI(poPart, poFeature)) {
                CPLError(CE_Failure, CPLE_AppDefined,
                         "WritePOI: Failed to write part %d/%d of MultiPoint",
                         i + 1, nParts);
                return false;
            }
            nWritten++;
        }

        // M1 Fix: Warn if all parts were skipped
        if (nWritten == 0 && nParts > 0) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "WritePOI: All %d parts of MultiPoint were empty/invalid - nothing written",
                     nParts);
        }

        CPLDebug("OGR_POLISHMAP", "WritePOI: Wrote %d/%d valid parts from MultiPoint",
                 nWritten, nParts);
        return true;
    }

    // Handle simple Point geometry
    if (eType == wkbPoint) {
        OGRPoint* poPoint = poGeom->toPoint();
        if (poPoint == nullptr) {
            CPLError(CE_Failure, CPLE_AppDefined,
                     "WritePOI: failed to cast geometry to Point");
            return false;
        }
        return WriteSinglePOI(poPoint, poFeature);
    }

    // Unsupported geometry type
    CPLError(CE_Failure, CPLE_AppDefined,
             "WritePOI: feature geometry is not Point or MultiPoint (type=%d)",
             static_cast<int>(poGeom->getGeometryType()));
    return false;
}

/************************************************************************/
/*                       WriteSinglePOLYLINE()                           */
/*                                                                      */
/* Story 4.2 Task 4.2: Write a single LineString as [POLYLINE] section.  */
/* Extracted from WritePOLYLINE() to enable MultiLineString decomposition*/
/* Story 3.1: Uses buffered writing (NFR2 performance)                   */
/* Format:                                                               */
/*   [POLYLINE]                                                          */
/*   Type=<type_code>                                                    */
/*   Label=<label>       (optional, UTF-8 → CP1252)                      */
/*   Data0=(lat1,lon1),(lat2,lon2),...  (ALL points on ONE line)         */
/*   EndLevel=<level>    (optional)                                      */
/*   Levels=<range>      (optional)                                      */
/*   [END]                                                               */
/************************************************************************/

bool PolishMapWriter::WriteSinglePOLYLINE(OGRLineString* poLine, OGRFeature* poFeature)
{
    if (poLine == nullptr || poFeature == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WriteSinglePOLYLINE: NULL pointer");
        return false;
    }

    // Validate minimum 2 points for valid POLYLINE
    int nNumPoints = poLine->getNumPoints();
    if (nNumPoints < 2) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WriteSinglePOLYLINE: LineString has less than 2 points (%d)",
                 nNumPoints);
        return false;
    }

    // Write [POLYLINE] section marker
    if (!BufferedWrite("[POLYLINE]\n")) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "WriteSinglePOLYLINE: failed to write [POLYLINE] marker");
        return false;
    }

    // Extract and write Type field (required)
    const char* pszType = poFeature->GetFieldAsString("Type");
    if (pszType != nullptr && pszType[0] != '\0') {
        if (!BufferedWrite(FormatString("Type=%s\n", pszType).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write Type");
            return false;
        }
    } else {
        // Default POLYLINE type
        if (!BufferedWrite(FormatString("Type=%s\n", DEFAULT_POLYLINE_TYPE).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write default Type");
            return false;
        }
    }

    // Extract and write Label field (optional)
    const char* pszLabel = poFeature->GetFieldAsString("Label");
    if (pszLabel != nullptr && pszLabel[0] != '\0') {
        std::string osLabelCP1252 = RecodeToCP1252(pszLabel);
        if (!BufferedWrite(FormatString("Label=%s\n", osLabelCP1252.c_str()).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write Label");
            return false;
        }
    }

    // Write Data0 coordinates (ALL points on ONE line)
    // CRITICAL: Polish Map format uses (lat, lon) order, NOT (lon, lat)!
    std::string osData0 = "Data0=";
    osData0.reserve(static_cast<size_t>(nNumPoints) * 30);

    for (int i = 0; i < nNumPoints; i++) {
        double dfLat = poLine->getY(i);
        double dfLon = poLine->getX(i);

        if (i > 0) {
            osData0 += ",";
        }
        osData0 += FormatString("(%.6f,%.6f)", dfLat, dfLon);
    }
    osData0 += "\n";

    if (!BufferedWrite(osData0.c_str())) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write Data0");
        return false;
    }

    // Extract and write EndLevel field (optional)
    int nEndLevelIdx = poFeature->GetFieldIndex("EndLevel");
    if (nEndLevelIdx >= 0 && poFeature->IsFieldSetAndNotNull(nEndLevelIdx)) {
        int nEndLevel = poFeature->GetFieldAsInteger("EndLevel");
        if (!BufferedWrite(FormatString("EndLevel=%d\n", nEndLevel).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write EndLevel");
            return false;
        }
    }

    // Extract and write Levels field (optional)
    const char* pszLevels = poFeature->GetFieldAsString("Levels");
    if (pszLevels != nullptr && pszLevels[0] != '\0') {
        if (!BufferedWrite(FormatString("Levels=%s\n", pszLevels).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write Levels");
            return false;
        }
    }

    // Write [END] marker
    if (!BufferedWrite("[END]\n\n")) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write [END]");
        return false;
    }

    CPLDebug("OGR_POLISHMAP", "WriteSinglePOLYLINE: Type=%s, Label=%s, %d points",
             pszType ? pszType : "(default)",
             pszLabel ? pszLabel : "(null)",
             nNumPoints);

    return true;
}

/************************************************************************/
/*                          WritePOLYLINE()                              */
/*                                                                      */
/* Story 2.4 Task 1: Write POLYLINE feature to output file.              */
/* Story 4.2: Updated to handle wkbMultiLineString decomposition (AC2).  */
/* Story 3.1: Uses buffered writing (NFR2 performance)                   */
/************************************************************************/

bool PolishMapWriter::WritePOLYLINE(OGRFeature* poFeature)
{
    // Validate input
    if (poFeature == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOLYLINE: NULL feature pointer");
        return false;
    }

    if (m_fpOutput == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOLYLINE: file handle is null");
        return false;
    }

    // Ensure header is written before first feature
    if (!m_bHeaderWritten) {
        std::map<std::string, std::string> aoDefaultMetadata;
        aoDefaultMetadata["Name"] = "Untitled";
        aoDefaultMetadata["CodePage"] = "1252";
        if (!WriteHeader(aoDefaultMetadata)) {
            CPLError(CE_Failure, CPLE_FileIO,
                     "WritePOLYLINE: failed to write header");
            return false;
        }
    }

    // Extract geometry
    OGRGeometry* poGeom = poFeature->GetGeometryRef();
    if (poGeom == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOLYLINE: feature has no geometry");
        return false;
    }

    OGRwkbGeometryType eType = wkbFlatten(poGeom->getGeometryType());

    // Story 4.2 Task 2: Handle wkbMultiLineString - decompose into multiple [POLYLINE] sections
    if (eType == wkbMultiLineString) {
        OGRMultiLineString* poMulti = poGeom->toMultiLineString();
        if (poMulti == nullptr) {
            CPLError(CE_Failure, CPLE_AppDefined,
                     "WritePOLYLINE: failed to cast geometry to MultiLineString");
            return false;
        }

        int nParts = poMulti->getNumGeometries();
        int nWritten = 0;  // M1 Fix: Count written parts
        CPLDebug("OGR_POLISHMAP", "WritePOLYLINE: Decomposing MultiLineString with %d parts", nParts);

        for (int i = 0; i < nParts; i++) {
            OGRGeometry* poPartGeom = poMulti->getGeometryRef(i);
            if (poPartGeom == nullptr) {
                CPLDebug("OGR_POLISHMAP", "WritePOLYLINE: Skipping NULL part %d/%d", i + 1, nParts);
                continue;
            }

            OGRLineString* poPart = poPartGeom->toLineString();
            if (poPart == nullptr) {
                CPLDebug("OGR_POLISHMAP", "WritePOLYLINE: Skipping non-LineString part %d/%d", i + 1, nParts);
                continue;
            }

            // Skip degenerate parts (< 2 points)
            if (poPart->getNumPoints() < 2) {
                CPLDebug("OGR_POLISHMAP", "WritePOLYLINE: Skipping degenerate part %d/%d (%d points)",
                         i + 1, nParts, poPart->getNumPoints());
                continue;
            }

            if (!WriteSinglePOLYLINE(poPart, poFeature)) {
                CPLError(CE_Failure, CPLE_AppDefined,
                         "WritePOLYLINE: Failed to write part %d/%d of MultiLineString",
                         i + 1, nParts);
                return false;
            }
            nWritten++;
        }

        // M1 Fix: Warn if all parts were skipped
        if (nWritten == 0 && nParts > 0) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "WritePOLYLINE: All %d parts of MultiLineString were empty/degenerate - nothing written",
                     nParts);
        }

        CPLDebug("OGR_POLISHMAP", "WritePOLYLINE: Wrote %d/%d valid parts from MultiLineString",
                 nWritten, nParts);
        return true;
    }

    // Handle simple LineString geometry
    if (eType == wkbLineString) {
        OGRLineString* poLine = poGeom->toLineString();
        if (poLine == nullptr) {
            CPLError(CE_Failure, CPLE_AppDefined,
                     "WritePOLYLINE: failed to cast geometry to LineString");
            return false;
        }
        return WriteSinglePOLYLINE(poLine, poFeature);
    }

    // Unsupported geometry type
    CPLError(CE_Failure, CPLE_AppDefined,
             "WritePOLYLINE: feature geometry is not LineString or MultiLineString (type=%d)",
             static_cast<int>(poGeom->getGeometryType()));
    return false;
}

/************************************************************************/
/*                       WriteSinglePOLYGON()                            */
/*                                                                      */
/* Story 4.2 Task 4.3: Write a single Polygon as [POLYGON] section.      */
/* Extracted from WritePOLYGON() to enable MultiPolygon decomposition.   */
/* Story 3.1: Uses buffered writing (NFR2 performance)                   */
/* Format:                                                               */
/*   [POLYGON]                                                           */
/*   Type=<type_code>                                                    */
/*   Label=<label>       (optional, UTF-8 → CP1252)                      */
/*   Data0=(lat1,lon1),(lat2,lon2),...,(lat1,lon1)  (closed ring)        */
/*   EndLevel=<level>    (optional)                                      */
/*   [END]                                                               */
/************************************************************************/

bool PolishMapWriter::WriteSinglePOLYGON(OGRPolygon* poPolygon, OGRFeature* poFeature)
{
    if (poPolygon == nullptr || poFeature == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WriteSinglePOLYGON: NULL pointer");
        return false;
    }

    // Extract exterior ring
    OGRLinearRing* poRing = poPolygon->getExteriorRing();
    if (poRing == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WriteSinglePOLYGON: Polygon has no exterior ring");
        return false;
    }

    // M3 Fix: Warn if interior rings (holes) are present - they will be ignored
    int nInteriorRings = poPolygon->getNumInteriorRings();
    if (nInteriorRings > 0) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "WriteSinglePOLYGON: Polygon has %d interior ring(s) (holes) which will be ignored - "
                 "Polish Map format does not support polygon holes",
                 nInteriorRings);
    }

    // Validate minimum 3 points for valid POLYGON
    int nNumPoints = poRing->getNumPoints();
    if (nNumPoints < 3) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WriteSinglePOLYGON: Polygon exterior ring has less than 3 points (%d)",
                 nNumPoints);
        return false;
    }

    // Validate polygon is not degenerate (all points at same location)
    bool bIsDegenerate = true;
    double dfRefLat = poRing->getY(0);
    double dfRefLon = poRing->getX(0);
    for (int i = 1; i < nNumPoints && bIsDegenerate; i++) {
        if (fabs(poRing->getY(i) - dfRefLat) > RING_CLOSURE_TOLERANCE ||
            fabs(poRing->getX(i) - dfRefLon) > RING_CLOSURE_TOLERANCE) {
            bIsDegenerate = false;
        }
    }
    if (bIsDegenerate) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WriteSinglePOLYGON: Polygon is degenerate (all %d points at same location)",
                 nNumPoints);
        return false;
    }

    // Warn about very large polygons
    if (nNumPoints > LARGE_POLYGON_WARNING_THRESHOLD) {
        CPLDebug("OGR_POLISHMAP",
                 "WriteSinglePOLYGON: Large polygon with %d points (performance warning)",
                 nNumPoints);
    }

    // Check if ring needs auto-closing
    double dfFirstLat = poRing->getY(0);
    double dfFirstLon = poRing->getX(0);
    double dfLastLat = poRing->getY(nNumPoints - 1);
    double dfLastLon = poRing->getX(nNumPoints - 1);
    bool bNeedsClosing = (fabs(dfFirstLat - dfLastLat) > RING_CLOSURE_TOLERANCE ||
                          fabs(dfFirstLon - dfLastLon) > RING_CLOSURE_TOLERANCE);

    if (bNeedsClosing) {
        CPLDebug("OGR_POLISHMAP", "Auto-closing POLYGON ring for writing");
    }

    // Write [POLYGON] section marker
    if (!BufferedWrite("[POLYGON]\n")) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "WriteSinglePOLYGON: failed to write [POLYGON] marker");
        return false;
    }

    // Extract and write Type field (required)
    const char* pszType = poFeature->GetFieldAsString("Type");
    if (pszType != nullptr && pszType[0] != '\0') {
        if (!BufferedWrite(FormatString("Type=%s\n", pszType).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYGON: failed to write Type");
            return false;
        }
    } else {
        // Default POLYGON type
        if (!BufferedWrite(FormatString("Type=%s\n", DEFAULT_POLYGON_TYPE).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYGON: failed to write default Type");
            return false;
        }
    }

    // Extract and write Label field (optional)
    const char* pszLabel = poFeature->GetFieldAsString("Label");
    if (pszLabel != nullptr && pszLabel[0] != '\0') {
        std::string osLabelCP1252 = RecodeToCP1252(pszLabel);
        if (!BufferedWrite(FormatString("Label=%s\n", osLabelCP1252.c_str()).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYGON: failed to write Label");
            return false;
        }
    }

    // Write Data0 coordinates (ALL points on ONE line, closed ring)
    // CRITICAL: Polish Map format uses (lat, lon) order, NOT (lon, lat)!
    std::string osData0 = "Data0=";
    osData0.reserve(static_cast<size_t>(nNumPoints + 1) * 30);

    for (int i = 0; i < nNumPoints; i++) {
        double dfLat = poRing->getY(i);
        double dfLon = poRing->getX(i);

        if (i > 0) {
            osData0 += ",";
        }
        osData0 += FormatString("(%.6f,%.6f)", dfLat, dfLon);
    }

    // Auto-close ring if needed (duplicate first point)
    if (bNeedsClosing) {
        osData0 += FormatString(",(%.6f,%.6f)", dfFirstLat, dfFirstLon);
    }
    osData0 += "\n";

    if (!BufferedWrite(osData0.c_str())) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYGON: failed to write Data0");
        return false;
    }

    // Extract and write EndLevel field (optional)
    int nEndLevelIdx = poFeature->GetFieldIndex("EndLevel");
    if (nEndLevelIdx >= 0 && poFeature->IsFieldSetAndNotNull(nEndLevelIdx)) {
        int nEndLevel = poFeature->GetFieldAsInteger("EndLevel");
        if (!BufferedWrite(FormatString("EndLevel=%d\n", nEndLevel).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYGON: failed to write EndLevel");
            return false;
        }
    }

    // Write [END] marker
    if (!BufferedWrite("[END]\n\n")) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYGON: failed to write [END]");
        return false;
    }

    int nWrittenPoints = bNeedsClosing ? nNumPoints + 1 : nNumPoints;
    CPLDebug("OGR_POLISHMAP", "WriteSinglePOLYGON: Type=%s, Label=%s, %d points%s",
             pszType ? pszType : "(default)",
             pszLabel ? pszLabel : "(null)",
             nWrittenPoints,
             bNeedsClosing ? " (auto-closed)" : "");

    return true;
}

/************************************************************************/
/*                          WritePOLYGON()                               */
/*                                                                      */
/* Story 2.5 Task 1: Write POLYGON feature to output file.               */
/* Story 4.2: Updated to handle wkbMultiPolygon decomposition (AC1).     */
/* Story 3.1: Uses buffered writing (NFR2 performance)                   */
/************************************************************************/

bool PolishMapWriter::WritePOLYGON(OGRFeature* poFeature)
{
    // Validate input
    if (poFeature == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOLYGON: NULL feature pointer");
        return false;
    }

    if (m_fpOutput == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOLYGON: file handle is null");
        return false;
    }

    // Ensure header is written before first feature
    if (!m_bHeaderWritten) {
        std::map<std::string, std::string> aoDefaultMetadata;
        aoDefaultMetadata["Name"] = "Untitled";
        aoDefaultMetadata["CodePage"] = "1252";
        if (!WriteHeader(aoDefaultMetadata)) {
            CPLError(CE_Failure, CPLE_FileIO,
                     "WritePOLYGON: failed to write header");
            return false;
        }
    }

    // Extract geometry
    OGRGeometry* poGeom = poFeature->GetGeometryRef();
    if (poGeom == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "WritePOLYGON: feature has no geometry");
        return false;
    }

    OGRwkbGeometryType eType = wkbFlatten(poGeom->getGeometryType());

    // Story 4.2 Task 3: Handle wkbMultiPolygon - decompose into multiple [POLYGON] sections
    if (eType == wkbMultiPolygon) {
        OGRMultiPolygon* poMulti = poGeom->toMultiPolygon();
        if (poMulti == nullptr) {
            CPLError(CE_Failure, CPLE_AppDefined,
                     "WritePOLYGON: failed to cast geometry to MultiPolygon");
            return false;
        }

        int nParts = poMulti->getNumGeometries();
        int nWritten = 0;
        CPLDebug("OGR_POLISHMAP", "WritePOLYGON: Decomposing MultiPolygon with %d parts", nParts);

        for (int i = 0; i < nParts; i++) {
            OGRGeometry* poPartGeom = poMulti->getGeometryRef(i);
            if (poPartGeom == nullptr) {
                CPLDebug("OGR_POLISHMAP", "WritePOLYGON: Skipping NULL part %d/%d", i + 1, nParts);
                continue;
            }

            OGRPolygon* poPart = poPartGeom->toPolygon();
            if (poPart == nullptr) {
                CPLDebug("OGR_POLISHMAP", "WritePOLYGON: Skipping non-Polygon part %d/%d", i + 1, nParts);
                continue;
            }

            // Story 4.2: Skip empty/degenerate parts - don't fail entire operation
            OGRLinearRing* poRing = poPart->getExteriorRing();
            if (poRing == nullptr || poRing->getNumPoints() < 3) {
                CPLDebug("OGR_POLISHMAP", "WritePOLYGON: Skipping degenerate part %d/%d (< 3 points)",
                         i + 1, nParts);
                continue;
            }

            if (!WriteSinglePOLYGON(poPart, poFeature)) {
                CPLError(CE_Failure, CPLE_AppDefined,
                         "WritePOLYGON: Failed to write part %d/%d of MultiPolygon",
                         i + 1, nParts);
                return false;
            }
            nWritten++;
        }

        // H4 Fix: Warn if all parts were skipped
        if (nWritten == 0 && nParts > 0) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "WritePOLYGON: All %d parts of MultiPolygon were empty/degenerate - nothing written",
                     nParts);
        }

        CPLDebug("OGR_POLISHMAP", "WritePOLYGON: Wrote %d/%d valid parts from MultiPolygon",
                 nWritten, nParts);
        return true;
    }

    // Handle simple Polygon geometry
    if (eType == wkbPolygon) {
        OGRPolygon* poPolygon = poGeom->toPolygon();
        if (poPolygon == nullptr) {
            CPLError(CE_Failure, CPLE_AppDefined,
                     "WritePOLYGON: failed to cast geometry to Polygon");
            return false;
        }
        return WriteSinglePOLYGON(poPolygon, poFeature);
    }

    // Unsupported geometry type
    CPLError(CE_Failure, CPLE_AppDefined,
             "WritePOLYGON: feature geometry is not Polygon or MultiPolygon (type=%d)",
             static_cast<int>(poGeom->getGeometryType()));
    return false;
}
