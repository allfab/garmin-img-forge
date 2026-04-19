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
#include "polishmapfields.h"
#include "polishmapparser.h"  // Story 2.2.8: For template parsing
#include "cpl_error.h"
#include "cpl_conv.h"
#include <cstdarg>  // Story 3.1: For FormatString() helper
#include <cstring>

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
    , m_paoMetadata(nullptr)
    , m_oFieldMapping()  // Story 4.4: Initialize empty mapping
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
/*                      GetDefaultHeaderData()                           */
/*                                                                      */
/* Story 2.2.4: Return intelligent default header values based on       */
/* cGPSmapper best practices and specification recommendations.         */
/************************************************************************/

static std::map<std::string, std::string> GetDefaultHeaderData()
{
    std::map<std::string, std::string> aoDefaults;

    // Basic fields
    aoDefaults["ID"] = "1";                    // Required field (spec requirement)
    aoDefaults["Name"] = "Untitled";           // Default map name
    aoDefaults["CodePage"] = "1252";           // CP1252 (Windows Western European)
    aoDefaults["Datum"] = "W84";               // WGS 84 (most common)

    // Critical fields (Story 2.2.4)
    aoDefaults["LBLcoding"] = "9";             // 9 = 8-bit encoding (smallest maps)
    aoDefaults["Preprocess"] = "F";            // F = Full generalization (best compatibility)
    aoDefaults["TreeSize"] = "3000";           // 3000 = Countryside maps (balanced)
    aoDefaults["RgnLimit"] = "1024";           // 1024 = Maximum elements per region

    // Important fields (Story 2.2.4)
    aoDefaults["Transparent"] = "N";           // N = No transparency (default)
    aoDefaults["SimplifyLevel"] = "2";         // 2 = Moderate simplification (balanced)
    aoDefaults["Marine"] = "N";                // N = Not a marine map (default)
    aoDefaults["LeftSideTraffic"] = "N";       // N = Right-side traffic (most common)

    // Note: Levels and Level0-N are NOT defaulted - they require explicit user input
    // based on the specific map's zoom level requirements

    return aoDefaults;
}

/************************************************************************/
/*                           WriteHeader()                               */
/*                                                                      */
/* Story 2.1 Task 3.2: Write minimal [IMG ID] header section.           */
/* Story 2.2 Review: Refactored to delegate to WriteHeader(map) to      */
/* avoid code duplication.                                               */
/* Story 2.2.4: Now uses intelligent defaults from GetDefaultHeaderData() */
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
/*                        SetFieldMapping()                              */
/*                                                                      */
/* Story 4.4 Task 5: Set field mapping for reading feature attributes.  */
/* Enables reading from source fields that were mapped via CreateField().*/
/************************************************************************/

void PolishMapWriter::SetFieldMapping(const std::map<std::string, std::string>& aoMapping)
{
    m_oFieldMapping = aoMapping;
    CPLDebug("OGR_POLISHMAP", "Writer: Field mapping set with %zu entries",
             m_oFieldMapping.size());
}

/************************************************************************/
/*                          SetMetadata()                                */
/*                                                                      */
/* Store pointer to datasource metadata for use in safety net header    */
/* writing within WritePOI/WritePOLYLINE/WritePOLYGON.                  */
/************************************************************************/

void PolishMapWriter::SetMetadata(const std::map<std::string, std::string>* paoMetadata)
{
    m_paoMetadata = paoMetadata;
}

/************************************************************************/
/*                          GetFieldName()                               */
/*                                                                      */
/* Story 4.4 Task 5: Resolve field name using mapping.                  */
/* Returns source field name if mapping exists, else canonical name.     */
/************************************************************************/

const char* PolishMapWriter::GetFieldName(const char* pszCanonicalField) const
{
    auto it = m_oFieldMapping.find(pszCanonicalField);
    if (it != m_oFieldMapping.end()) {
        // Mapped field - return source field name
        return it->second.c_str();
    }
    // No mapping - return canonical field name (backward compatibility)
    return pszCanonicalField;
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

    // Story 2.2.8 Task 4: Check for HEADER_TEMPLATE option (priority 1)
    auto itTemplate = aoMetadata.find("HEADER_TEMPLATE");
    if (itTemplate != aoMetadata.end() && !itTemplate->second.empty()) {
        // Template-based generation (AC4: template takes precedence)
        return WriteHeaderFromTemplate(itTemplate->second.c_str());
    }

    // Fallback to metadata-based or defaults-based generation (priority 2/3)

    // Story 2.2.4: Merge user metadata with intelligent defaults
    // User-provided values override defaults
    std::map<std::string, std::string> aoMergedMetadata = GetDefaultHeaderData();
    for (const auto& kv : aoMetadata) {
        aoMergedMetadata[kv.first] = kv.second;  // Override defaults with user values
    }

    // Extract Name and CodePage for special handling
    std::string osName = "Untitled";
    std::string osCodePage = "1252";

    auto itName = aoMergedMetadata.find("Name");
    if (itName != aoMergedMetadata.end()) {
        osName = RecodeToCP1252(itName->second);  // UTF-8 → CP1252
    }

    auto itCodePage = aoMergedMetadata.find("CodePage");
    if (itCodePage != aoMergedMetadata.end()) {
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

    // Write other metadata fields (ID, Elevation, etc.)
    // Story 2.2 Extension: Ordered set of known fields to write in logical order
    // Order follows cGPSmapper spec recommendations: critical fields first, then important
    static const char* const apszKnownFields[] = {
        // Basic fields
        "ID",              // Map ID (REQUIRED per cGPSmapper spec)
        "Elevation",       // Elevation unit (M/F)
        "Datum",           // Coordinate system (W84, etc.)
        // Critical fields (Story 2.2.1)
        "LBLcoding",       // Label encoding (6/9/10)
        "Preprocess",      // Preprocessing mode (G/F/P/N)
        "Levels",          // Number of zoom levels (1-10)
        // Level definitions (written in order after Levels)
        "Level0", "Level1", "Level2", "Level3", "Level4",
        "Level5", "Level6", "Level7", "Level8", "Level9",
        "TreeSize",        // Map tree size (100-15000)
        "RgnLimit",        // Region element limit (50-1024)
        // Important fields (Story 2.2.2)
        "Transparent",     // Transparency (Y/N/S)
        "SimplifyLevel",   // Simplification level (0-4)
        "Marine",          // Marine map (Y/N)
        "LeftSideTraffic", // Left-side traffic (Y/N)
        "Routing",         // Routing enabled (Y/N) — required by mkgmap when RoadID present
        nullptr
    };

    for (int i = 0; apszKnownFields[i] != nullptr; i++) {
        auto it = aoMergedMetadata.find(apszKnownFields[i]);
        if (it != aoMergedMetadata.end()) {
            std::string osValue = RecodeToCP1252(it->second);
            if (!BufferedWrite(FormatString("%s=%s\n", it->first.c_str(), osValue.c_str()).c_str())) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "PolishMapWriter::WriteHeader() - failed to write %s",
                         it->first.c_str());
                return false;
            }
        }
    }

    // Story 2.2.5: Write any remaining custom fields (not in known list)
    // This preserves aoOtherFields from parser for round-trip compatibility
    for (const auto& kv : aoMergedMetadata) {
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

    CPLDebug("OGR_POLISHMAP", "WriteHeader: Name=%s, CodePage=%s, ID=%s, %d fields total",
             osName.c_str(), osCodePage.c_str(),
             aoMergedMetadata["ID"].c_str(),
             static_cast<int>(aoMergedMetadata.size()));

    return true;
}

/************************************************************************/
/*                        WriteHeaderField()                            */
/*                                                                      */
/* M4/M5 fix: Helper to write a single header field with error logging. */
/************************************************************************/

bool PolishMapWriter::WriteHeaderField(const char* pszKey, const std::string& osValue, bool bRecodeCP1252)
{
    if (osValue.empty()) {
        return true;  // Nothing to write, not an error
    }

    std::string osEncodedValue = bRecodeCP1252 ? RecodeToCP1252(osValue) : osValue;
    std::string osLine = FormatString("%s=%s\n", pszKey, osEncodedValue.c_str());

    if (!BufferedWrite(osLine.c_str())) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeaderField() - failed to write %s", pszKey);
        return false;
    }

    return true;
}

/************************************************************************/
/*                      WriteHeaderFromTemplate()                        */
/*                                                                      */
/* Story 2.2.8 Task 3: Copy header from template file.                  */
/* Parses template file's [IMG ID] section and writes all fields.       */
/************************************************************************/

bool PolishMapWriter::WriteHeaderFromTemplate(const char* pszTemplatePath)
{
    if (m_fpOutput == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "PolishMapWriter::WriteHeaderFromTemplate() - file handle is null");
        return false;
    }

    if (m_bHeaderWritten) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "PolishMapWriter::WriteHeaderFromTemplate() - header already written");
        return true;  // Not a fatal error
    }

    // Story 2.2.8 Task 3.2: Validate template file exists (AC2)
    VSIStatBufL sStat;
    if (VSIStatL(pszTemplatePath, &sStat) != 0) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "HEADER_TEMPLATE file not found: %s", pszTemplatePath);
        return false;
    }

    // Story 2.2.8 Task 3.3: Parse template file
    // M1 note: Template is parsed twice (once in Create() for validation, once here for data).
    // This is an acceptable trade-off: fail-fast validation prevents invalid datasets from
    // being created, while keeping validation and writing concerns separated (SOLID principle).
    // Header files are small (<10KB typically), so performance impact is negligible.
    PolishMapParser oParser(pszTemplatePath);
    if (!oParser.IsOpen()) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "Failed to open HEADER_TEMPLATE file: %s", pszTemplatePath);
        return false;
    }

    // Story 2.2.8 Task 3.4: Parse header (AC3)
    if (!oParser.ParseHeader()) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "HEADER_TEMPLATE file has invalid [IMG ID] section: %s",
                 pszTemplatePath);
        return false;
    }

    // Story 2.2.8 Task 3.5: Get header data
    const PolishMapHeaderData& oHeader = oParser.GetHeaderData();

    // Story 2.2.8 Task 3.6: Write [IMG ID] section
    if (!BufferedWrite("[IMG ID]\n")) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeaderFromTemplate() - failed to write [IMG ID]");
        return false;
    }

    // M4/M5 fix: Write 15 standard fields using helper function
    // Order: ID, Name, Elevation, Datum, LBLcoding, Preprocess, Levels, TreeSize, etc.

    if (!WriteHeaderField("ID", oHeader.osID)) return false;
    if (!WriteHeaderField("Name", oHeader.osName, true)) return false;  // CP1252 encoding
    if (!WriteHeaderField("Elevation", oHeader.osElevation)) return false;
    if (!WriteHeaderField("Datum", oHeader.osDatum)) return false;
    if (!WriteHeaderField("LBLcoding", oHeader.osLBLcoding)) return false;
    if (!WriteHeaderField("Preprocess", oHeader.osPreprocess)) return false;
    if (!WriteHeaderField("Levels", oHeader.osLevels)) return false;

    // M7 fix: Write Level0-N definitions from template aoLevelDefs
    for (size_t i = 0; i < oHeader.aoLevelDefs.size(); i++) {
        std::string osLevelKey = FormatString("Level%zu", i);
        if (!WriteHeaderField(osLevelKey.c_str(), oHeader.aoLevelDefs[i])) return false;
    }

    if (!WriteHeaderField("TreeSize", oHeader.osTreeSize)) return false;
    if (!WriteHeaderField("RgnLimit", oHeader.osRgnLimit)) return false;
    if (!WriteHeaderField("Transparent", oHeader.osTransparent)) return false;
    if (!WriteHeaderField("SimplifyLevel", oHeader.osSimplifyLevel)) return false;
    if (!WriteHeaderField("Marine", oHeader.osMarine)) return false;
    if (!WriteHeaderField("LeftSideTraffic", oHeader.osLeftSideTraffic)) return false;

    // Story 2.2.8 Task 3.7: Write custom fields (aoOtherFields) for AC6
    for (const auto& pair : oHeader.aoOtherFields) {
        if (!WriteHeaderField(pair.first.c_str(), pair.second)) return false;
    }

    // CodePage always last before [END]
    if (!WriteHeaderField("CodePage", oHeader.osCodePage)) return false;

    // Write [END] marker
    if (!BufferedWrite("[END]\n")) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeaderFromTemplate() - failed to write [END]");
        return false;
    }

    m_bHeaderWritten = true;

    CPLDebug("OGR_POLISHMAP", "WriteHeaderFromTemplate: Copied header from %s (Name=%s)",
             pszTemplatePath, oHeader.osName.c_str());

    return true;
}

/************************************************************************/
/*                    WriteExtendedAttributes()                          */
/*                                                                      */
/* Write extended attributes for a feature based on layer type.          */
/* Skips fields already written explicitly (Type, Label, EndLevel,      */
/* Levels) and Data0 (coordinates).                                     */
/************************************************************************/

bool PolishMapWriter::WriteExtendedAttributes(OGRFeature* poFeature,
                                               unsigned int nLayerFlag)
{
    auto aoFields = GetFieldsForLayer(nLayerFlag);
    for (const auto* pDef : aoFields) {
        // Skip fields already written explicitly
        if (EQUAL(pDef->pszName, "Type") || EQUAL(pDef->pszName, "Label") ||
            EQUAL(pDef->pszName, "EndLevel") || EQUAL(pDef->pszName, "Levels")) {
            continue;
        }

        // Story 4.4 Task 5: Use field mapping to read from correct source field
        const char* pszFieldName = GetFieldName(pDef->pszName);
        int nFieldIdx = poFeature->GetFieldIndex(pszFieldName);
        if (nFieldIdx < 0 || !poFeature->IsFieldSetAndNotNull(nFieldIdx)) {
            continue;
        }

        if (pDef->eType == OFTInteger) {
            int nValue = poFeature->GetFieldAsInteger(nFieldIdx);
            if (!BufferedWrite(FormatString("%s=%d\n", pDef->pszName, nValue).c_str())) {
                return false;
            }
        } else {
            const char* pszValue = poFeature->GetFieldAsString(nFieldIdx);
            if (pszValue != nullptr && pszValue[0] != '\0') {
                std::string osCP1252 = RecodeToCP1252(pszValue);
                if (!BufferedWrite(FormatString("%s=%s\n", pDef->pszName, osCP1252.c_str()).c_str())) {
                    return false;
                }
            }
        }
    }
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

    // Story 4.4 Task 5: Use field mapping to read from correct source field
    // Extract and write Type field (required)
    const char* pszTypeFieldName = GetFieldName("Type");
    const char* pszType = poFeature->GetFieldAsString(pszTypeFieldName);
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
    const char* pszLabelFieldName = GetFieldName("Label");
    const char* pszLabel = poFeature->GetFieldAsString(pszLabelFieldName);
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
    const char* pszEndLevelFieldName = GetFieldName("EndLevel");
    int nEndLevelIdx = poFeature->GetFieldIndex(pszEndLevelFieldName);
    if (nEndLevelIdx >= 0 && poFeature->IsFieldSetAndNotNull(nEndLevelIdx)) {
        int nEndLevel = poFeature->GetFieldAsInteger(pszEndLevelFieldName);
        if (!BufferedWrite(FormatString("EndLevel=%d\n", nEndLevel).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOI: failed to write EndLevel");
            return false;
        }
    }

    // Write extended attributes
    if (!WriteExtendedAttributes(poFeature, LAYER_POI)) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOI: failed to write extended attributes");
        return false;
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
        if (m_paoMetadata != nullptr) {
            if (!WriteHeader(*m_paoMetadata)) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "WritePOI: failed to write header");
                return false;
            }
        } else {
            std::map<std::string, std::string> aoDefaultMetadata;
            aoDefaultMetadata["Name"] = "Untitled";
            aoDefaultMetadata["CodePage"] = "1252";
            if (!WriteHeader(aoDefaultMetadata)) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "WritePOI: failed to write header");
                return false;
            }
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

    // Story 4.4 Task 5: Use field mapping to read from correct source field
    // Extract and write Type field (required)
    const char* pszTypeFieldName = GetFieldName("Type");
    const char* pszType = poFeature->GetFieldAsString(pszTypeFieldName);
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
    const char* pszLabelFieldName = GetFieldName("Label");
    const char* pszLabel = poFeature->GetFieldAsString(pszLabelFieldName);
    if (pszLabel != nullptr && pszLabel[0] != '\0') {
        std::string osLabelCP1252 = RecodeToCP1252(pszLabel);
        if (!BufferedWrite(FormatString("Label=%s\n", osLabelCP1252.c_str()).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write Label");
            return false;
        }
    }

    // Write Data0 coordinates (ALL points on ONE line).
    // CRITICAL: Polish Map format uses (lat, lon) order, NOT (lon, lat)!
    //
    // Tech-spec #2 Task 3: when the feature carries additional geometry fields
    // (Data1=, Data2=, ...) — only possible when MULTI_GEOM_FIELDS=YES at
    // dataset creation and the primary geometry is a single LineString (not a
    // MultiLineString being decomposed by the caller) — we iterate over all
    // non-empty geom fields of the feature and emit Data<i>= for each.
    auto formatPolylineDataLine = [](int nIndex, const OGRLineString* poLS) -> std::string {
        std::string osLine = FormatString("Data%d=", nIndex);
        int nPts = poLS->getNumPoints();
        osLine.reserve(static_cast<size_t>(nPts) * 30 + 8);
        for (int i = 0; i < nPts; i++) {
            if (i > 0) osLine += ",";
            osLine += FormatString("(%.6f,%.6f)", poLS->getY(i), poLS->getX(i));
        }
        osLine += "\n";
        return osLine;
    };

    // Emit Data0 from the passed primary LineString (always).
    if (!BufferedWrite(formatPolylineDataLine(0, poLine).c_str())) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write Data0");
        return false;
    }

    // Emit Data1..DataK from additional OGR geometry fields, only when the
    // feature's primary geometry is a single LineString (avoids emitting extra
    // geoms once per sub-part in a MultiLineString decomposition path).
    const OGRGeometry* poPrimary = poFeature->GetGeometryRef();
    const bool bIsSinglePrimary =
        poPrimary != nullptr &&
        wkbFlatten(poPrimary->getGeometryType()) == wkbLineString;
    if (bIsSinglePrimary) {
        int nGeomCount = poFeature->GetGeomFieldCount();
        for (int g = 1; g < nGeomCount; g++) {
            const OGRGeometry* poGeom = poFeature->GetGeomFieldRef(g);
            if (poGeom == nullptr || poGeom->IsEmpty()) continue;
            if (wkbFlatten(poGeom->getGeometryType()) != wkbLineString) {
                CPLError(CE_Warning, CPLE_AppDefined,
                         "WriteSinglePOLYLINE: additional geom field %d is not "
                         "a LineString, skipped", g);
                continue;
            }
            const OGRLineString* poAddLS =
                static_cast<const OGRLineString*>(poGeom);
            if (poAddLS->getNumPoints() < 2) continue;
            if (!BufferedWrite(formatPolylineDataLine(g, poAddLS).c_str())) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "WriteSinglePOLYLINE: failed to write Data%d", g);
                return false;
            }
        }
    }

    // Extract and write EndLevel field (optional)
    const char* pszEndLevelFieldName = GetFieldName("EndLevel");
    int nEndLevelIdx = poFeature->GetFieldIndex(pszEndLevelFieldName);
    if (nEndLevelIdx >= 0 && poFeature->IsFieldSetAndNotNull(nEndLevelIdx)) {
        int nEndLevel = poFeature->GetFieldAsInteger(pszEndLevelFieldName);
        if (!BufferedWrite(FormatString("EndLevel=%d\n", nEndLevel).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write EndLevel");
            return false;
        }
    }

    // Extract and write Levels field (optional)
    const char* pszLevelsFieldName = GetFieldName("Levels");
    const char* pszLevels = poFeature->GetFieldAsString(pszLevelsFieldName);
    if (pszLevels != nullptr && pszLevels[0] != '\0') {
        if (!BufferedWrite(FormatString("Levels=%s\n", pszLevels).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write Levels");
            return false;
        }
    }

    // Write extended attributes
    if (!WriteExtendedAttributes(poFeature, LAYER_POLYLINE)) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYLINE: failed to write extended attributes");
        return false;
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
        if (m_paoMetadata != nullptr) {
            if (!WriteHeader(*m_paoMetadata)) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "WritePOLYLINE: failed to write header");
                return false;
            }
        } else {
            std::map<std::string, std::string> aoDefaultMetadata;
            aoDefaultMetadata["Name"] = "Untitled";
            aoDefaultMetadata["CodePage"] = "1252";
            if (!WriteHeader(aoDefaultMetadata)) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "WritePOLYLINE: failed to write header");
                return false;
            }
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

    // Story 4.4 Task 5: Use field mapping to read from correct source field
    // Extract and write Type field (required)
    const char* pszTypeFieldName = GetFieldName("Type");
    const char* pszType = poFeature->GetFieldAsString(pszTypeFieldName);
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
    const char* pszLabelFieldName = GetFieldName("Label");
    const char* pszLabel = poFeature->GetFieldAsString(pszLabelFieldName);
    if (pszLabel != nullptr && pszLabel[0] != '\0') {
        std::string osLabelCP1252 = RecodeToCP1252(pszLabel);
        if (!BufferedWrite(FormatString("Label=%s\n", osLabelCP1252.c_str()).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYGON: failed to write Label");
            return false;
        }
    }

    // Write Data0 coordinates (ALL points on ONE line, closed ring).
    // CRITICAL: Polish Map format uses (lat, lon) order, NOT (lon, lat)!
    //
    // Tech-spec #2 Task 3: when MULTI_GEOM_FIELDS=YES and the feature carries
    // additional geometry fields (Data1=, Data2=, ...), we iterate over all
    // non-empty geom fields of the feature and emit Data<i>= for each.
    // Only applied when the primary geom is a single Polygon (not a
    // MultiPolygon being decomposed by the caller).
    auto formatPolygonDataLine = [](int nIndex,
                                    const OGRLinearRing* poR,
                                    bool bClose,
                                    double dfFirstLat_,
                                    double dfFirstLon_) -> std::string {
        std::string osLine = FormatString("Data%d=", nIndex);
        int nPts = poR->getNumPoints();
        osLine.reserve(static_cast<size_t>(nPts + 1) * 30 + 8);
        for (int i = 0; i < nPts; i++) {
            if (i > 0) osLine += ",";
            osLine += FormatString("(%.6f,%.6f)", poR->getY(i), poR->getX(i));
        }
        if (bClose) {
            osLine += FormatString(",(%.6f,%.6f)", dfFirstLat_, dfFirstLon_);
        }
        osLine += "\n";
        return osLine;
    };

    // Emit Data0 from the passed primary polygon's exterior ring.
    if (!BufferedWrite(formatPolygonDataLine(0, poRing, bNeedsClosing,
                                             dfFirstLat, dfFirstLon).c_str())) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYGON: failed to write Data0");
        return false;
    }

    // Emit Data1..DataK from additional OGR geometry fields, only when the
    // feature's primary geometry is a single Polygon.
    const OGRGeometry* poPrimary = poFeature->GetGeometryRef();
    const bool bIsSinglePrimary =
        poPrimary != nullptr &&
        wkbFlatten(poPrimary->getGeometryType()) == wkbPolygon;
    if (bIsSinglePrimary) {
        int nGeomCount = poFeature->GetGeomFieldCount();
        for (int g = 1; g < nGeomCount; g++) {
            const OGRGeometry* poGeom = poFeature->GetGeomFieldRef(g);
            if (poGeom == nullptr || poGeom->IsEmpty()) continue;
            if (wkbFlatten(poGeom->getGeometryType()) != wkbPolygon) {
                CPLError(CE_Warning, CPLE_AppDefined,
                         "WriteSinglePOLYGON: additional geom field %d is not "
                         "a Polygon, skipped", g);
                continue;
            }
            const OGRPolygon* poAddPoly =
                static_cast<const OGRPolygon*>(poGeom);
            const OGRLinearRing* poAddRing = poAddPoly->getExteriorRing();
            if (poAddRing == nullptr) continue;
            int nAddPts = poAddRing->getNumPoints();
            if (nAddPts < 3) continue;
            double dfAddFirstLat = poAddRing->getY(0);
            double dfAddFirstLon = poAddRing->getX(0);
            double dfAddLastLat  = poAddRing->getY(nAddPts - 1);
            double dfAddLastLon  = poAddRing->getX(nAddPts - 1);
            bool bAddNeedsClosing =
                (fabs(dfAddFirstLat - dfAddLastLat) > RING_CLOSURE_TOLERANCE ||
                 fabs(dfAddFirstLon - dfAddLastLon) > RING_CLOSURE_TOLERANCE);
            if (!BufferedWrite(formatPolygonDataLine(g, poAddRing,
                                                    bAddNeedsClosing,
                                                    dfAddFirstLat,
                                                    dfAddFirstLon).c_str())) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "WriteSinglePOLYGON: failed to write Data%d", g);
                return false;
            }
        }
    }

    // Extract and write EndLevel field (optional)
    const char* pszEndLevelFieldName = GetFieldName("EndLevel");
    int nEndLevelIdx = poFeature->GetFieldIndex(pszEndLevelFieldName);
    if (nEndLevelIdx >= 0 && poFeature->IsFieldSetAndNotNull(nEndLevelIdx)) {
        int nEndLevel = poFeature->GetFieldAsInteger(pszEndLevelFieldName);
        if (!BufferedWrite(FormatString("EndLevel=%d\n", nEndLevel).c_str())) {
            CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYGON: failed to write EndLevel");
            return false;
        }
    }

    // Write extended attributes
    if (!WriteExtendedAttributes(poFeature, LAYER_POLYGON)) {
        CPLError(CE_Failure, CPLE_FileIO, "WriteSinglePOLYGON: failed to write extended attributes");
        return false;
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
        if (m_paoMetadata != nullptr) {
            if (!WriteHeader(*m_paoMetadata)) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "WritePOLYGON: failed to write header");
                return false;
            }
        } else {
            std::map<std::string, std::string> aoDefaultMetadata;
            aoDefaultMetadata["Name"] = "Untitled";
            aoDefaultMetadata["CodePage"] = "1252";
            if (!WriteHeader(aoDefaultMetadata)) {
                CPLError(CE_Failure, CPLE_FileIO,
                         "WritePOLYGON: failed to write header");
                return false;
            }
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
