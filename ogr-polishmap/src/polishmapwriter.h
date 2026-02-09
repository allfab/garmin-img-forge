/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Polish Map format writer - header file
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

#ifndef POLISHMAPWRITER_H_INCLUDED
#define POLISHMAPWRITER_H_INCLUDED

#include "cpl_port.h"
#include "cpl_string.h"
#include "cpl_vsi.h"
#include "ogrsf_frmts.h"
#include "polishmapfields.h"
#include <string>
#include <map>

/************************************************************************/
/*                          PolishMapWriter                              */
/************************************************************************/

/**
 * @def WRITER_BUFFER_SIZE
 * @brief Size of the internal write buffer (64KB).
 *
 * Used for buffered I/O to reduce system calls and improve performance.
 * The buffer is flushed when full or when Flush() is called.
 */
static constexpr size_t WRITER_BUFFER_SIZE = 65536;  // 64KB buffer

/**
 * @class PolishMapWriter
 * @brief Writer class for Polish Map format output.
 *
 * Handles writing of Polish Map files with buffered I/O for performance.
 * Supports writing the [IMG ID] header section and feature sections
 * (POI, POLYLINE, POLYGON) with proper encoding conversion.
 *
 * @section buffering Buffered Writing
 * The writer uses a 64KB internal buffer to accumulate writes and reduce
 * system calls. Data is automatically flushed when the buffer is full,
 * or manually via Flush(). The destructor does NOT auto-flush; call
 * Flush() before closing the file handle.
 *
 * @section encoding Character Encoding
 * Labels are automatically converted from UTF-8 to CP1252 (Windows Western
 * European) encoding, which is the default for Polish Map files.
 *
 * @section ownership File Handle Ownership
 * The writer borrows the file handle; it does NOT close the file on
 * destruction. The caller must manage the file handle lifecycle.
 *
 * @see OGRPolishMapDataSource
 * @see OGRPolishMapLayer::ICreateFeature()
 */
class PolishMapWriter {
public:
    /**
     * @brief Construct writer with output file handle.
     * @param fpOutput VSILFILE handle opened for writing (borrowed, NOT owned)
     * @note The file handle is borrowed - caller retains ownership and must
     *       close it after writer is destroyed.
     */
    explicit PolishMapWriter(VSILFILE* fpOutput);

    /**
     * @brief Destructor - does NOT close file (borrowed handle)
     */
    ~PolishMapWriter();

    // Disable copy
    PolishMapWriter(const PolishMapWriter&) = delete;
    PolishMapWriter& operator=(const PolishMapWriter&) = delete;

    /**
     * @brief Write [IMG ID] header section with minimal required fields.
     * @param osName Map name (will be used as Name= field)
     * @param osCodePage Code page for encoding (default: "1252" for CP1252)
     * @return true if successful, false on write error
     *
     * Output format:
     * [IMG ID]
     * Name=<osName>
     * CodePage=<osCodePage>
     * [END]
     */
    bool WriteHeader(const std::string& osName, const std::string& osCodePage = "1252");

    /**
     * @brief Write [IMG ID] header section with full metadata.
     * @param aoMetadata Map of key=value pairs for header fields
     * @return true if successful, false on write error
     *
     * Story 2.2: Extended signature accepting metadata map.
     * Handles default values, field ordering, and UTF-8 to CP1252 conversion.
     * Story 2.2.8: Also checks for HEADER_TEMPLATE key in metadata.
     */
    bool WriteHeader(const std::map<std::string, std::string>& aoMetadata);

    /**
     * @brief Write [IMG ID] header section copied from template file.
     * @param pszTemplatePath Path to .mp file whose header will be copied
     * @return true if successful, false on error
     *
     * Story 2.2.8 Task 3: Template-based header generation.
     * Parses the template file's [IMG ID] section and writes it to output.
     * Preserves all 15 standard fields plus custom fields (aoOtherFields).
     *
     * Error handling:
     * - Template file not found → CE_Failure, return false
     * - Template has invalid [IMG ID] section → CE_Failure, return false
     */
    bool WriteHeaderFromTemplate(const char* pszTemplatePath);

    /**
     * @brief Flush any pending writes to file.
     * @return true if successful, false on error
     */
    bool Flush();

    /**
     * @brief Write a POI feature to the output file.
     * @param poFeature OGRFeature with Point geometry and attributes
     * @return true if successful, false on error
     *
     * Story 2.3: Writes [POI] section with:
     * - Type field (required, defaults to 0x0000)
     * - Label field (optional, UTF-8 → CP1252 converted)
     * - Data0 field (required, coordinates as (lat,lon) with 6 decimals)
     * - EndLevel field (optional)
     * - [END] marker (Polish Map standard for all sections)
     */
    bool WritePOI(OGRFeature* poFeature);

    /**
     * @brief Write a POLYLINE feature to the output file.
     * @param poFeature OGRFeature with LineString geometry and attributes
     * @return true if successful, false on error
     *
     * Story 2.4: Writes [POLYLINE] section with:
     * - Type field (required, defaults to 0x0001)
     * - Label field (optional, UTF-8 → CP1252 converted)
     * - Data0 field (required, ALL coordinates on ONE line as (lat,lon),(lat,lon),...)
     * - EndLevel field (optional)
     * - Levels field (optional)
     * - [END] marker (Polish Map standard for all sections)
     *
     * CRITICAL: LineString must have at least 2 points to be valid.
     */
    bool WritePOLYLINE(OGRFeature* poFeature);

    /**
     * @brief Write a POLYGON feature to the output file.
     * @param poFeature OGRFeature with Polygon geometry and attributes
     * @return true if successful, false on error
     *
     * Story 2.5: Writes [POLYGON] section with:
     * - Type field (required, defaults to 0x0001)
     * - Label field (optional, UTF-8 → CP1252 converted)
     * - Data0 field (required, ALL coordinates on ONE line as (lat,lon),(lat,lon),...
     *               with closed ring - first point = last point)
     * - EndLevel field (optional)
     * - [END] marker (Polish Map standard for all sections)
     *
     * CRITICAL: Polygon must have at least 3 points (triangle minimum).
     * Auto-closes open rings by duplicating the first point.
     */
    bool WritePOLYGON(OGRFeature* poFeature);

    /**
     * @brief Check if header has been written.
     * @return true if WriteHeader() has been called successfully
     */
    bool IsHeaderWritten() const { return m_bHeaderWritten; }

    /**
     * @brief Set field mapping for reading feature attributes.
     * @param aoMapping Map from canonical Polish Map fields to source field names
     *
     * Story 4.4 Task 5: Enables writer to read values from source fields that
     * have been mapped to Polish Map fields via CreateField().
     *
     * Example: {"Label" -> "NAME", "Type" -> "MP_TYPE"}
     * When writing, writer will read "NAME" field instead of "Label".
     */
    void SetFieldMapping(const std::map<std::string, std::string>& aoMapping);

private:
    VSILFILE* m_fpOutput;      // Borrowed file handle (NOT owned)
    bool m_bHeaderWritten;     // Track if header section was written

    // Story 3.1: Buffered writing members (Architecture: Buffered I/O Pattern)
    std::string m_osWriteBuffer;  // Accumulation buffer for writes (NFR2: 10 MB < 3s)

    // Story 4.4 Task 5: Field mapping (canonical Polish Map field -> source field name)
    std::map<std::string, std::string> m_oFieldMapping;

    /**
     * @brief Write data to internal buffer, flush if necessary.
     * @param pszData String data to write
     * @return true if successful, false on error
     *
     * Story 3.1: Buffered writing to reduce syscalls.
     * Accumulates data until buffer reaches WRITER_BUFFER_SIZE, then flushes.
     */
    bool BufferedWrite(const char* pszData);

    /**
     * @brief Flush internal write buffer to file.
     * @return true if successful, false on error
     */
    bool FlushBuffer();

    /**
     * @brief Convert UTF-8 string to CP1252 encoding.
     * @param osUTF8Value UTF-8 encoded string
     * @return CP1252 encoded string, or original string on conversion failure
     *
     * Story 2.2 Task 4: Uses CPLRecode for conversion.
     * On failure, logs CE_Warning and returns original value as fallback.
     */
    std::string RecodeToCP1252(const std::string& osUTF8Value);

    /**
     * @brief Helper to write a single header field with error logging.
     * @param pszKey Field name (e.g., "ID", "Name")
     * @param osValue Field value
     * @param bRecodeCP1252 If true, convert value from UTF-8 to CP1252
     * @return true if successful, false on error
     *
     * M4/M5 fix: Factorizes repeated code and adds consistent error logging.
     * Only writes if osValue is non-empty.
     */
    bool WriteHeaderField(const char* pszKey, const std::string& osValue, bool bRecodeCP1252 = false);

    /**
     * @brief Write extended attributes for a feature.
     * @param poFeature Feature containing attributes
     * @param nLayerFlag Layer bitmask (LAYER_POI, LAYER_POLYLINE, LAYER_POLYGON)
     * @return true if successful, false on error
     *
     * Iterates over extended fields applicable to the layer type and writes
     * any set, non-null fields as Key=Value lines. Skips fields already
     * written explicitly (Type, Label, EndLevel, Levels).
     */
    bool WriteExtendedAttributes(OGRFeature* poFeature, unsigned int nLayerFlag);

    // Story 4.2: Helper methods for single geometry writing (multi-geometry decomposition)

    /**
     * @brief Write a single Point geometry as [POI] section.
     * @param poPoint Point geometry to write
     * @param poFeature Feature containing attributes (Type, Label, EndLevel)
     * @return true if successful, false on error
     *
     * Story 4.2 Task 4.1: Extracted from WritePOI() to enable MultiPoint decomposition.
     * Writes [POI] section with Type, Label, Data0, EndLevel fields.
     */
    bool WriteSinglePOI(OGRPoint* poPoint, OGRFeature* poFeature);

    /**
     * @brief Write a single LineString geometry as [POLYLINE] section.
     * @param poLine LineString geometry to write
     * @param poFeature Feature containing attributes (Type, Label, EndLevel, Levels)
     * @return true if successful, false on error
     *
     * Story 4.2 Task 4.2: Extracted from WritePOLYLINE() to enable MultiLineString decomposition.
     * Writes [POLYLINE] section with Type, Label, Data0, EndLevel, Levels fields.
     */
    bool WriteSinglePOLYLINE(OGRLineString* poLine, OGRFeature* poFeature);

    /**
     * @brief Write a single Polygon geometry as [POLYGON] section.
     * @param poPolygon Polygon geometry to write
     * @param poFeature Feature containing attributes (Type, Label, EndLevel)
     * @return true if successful, false on error
     *
     * Story 4.2 Task 4.3: Extracted from WritePOLYGON() to enable MultiPolygon decomposition.
     * Writes [POLYGON] section with Type, Label, Data0, EndLevel fields.
     * Auto-closes open rings by duplicating first point.
     */
    bool WriteSinglePOLYGON(OGRPolygon* poPolygon, OGRFeature* poFeature);

    /**
     * @brief Resolve field name using mapping (canonical -> source).
     * @param pszCanonicalField Canonical Polish Map field name (e.g., "Label")
     * @return Source field name if mapping exists, else canonical name
     *
     * Story 4.4 Task 5: Helper to read values from correctly mapped source fields.
     * Example: GetFieldName("Label") returns "NAME" if mapping exists, else "Label".
     */
    const char* GetFieldName(const char* pszCanonicalField) const;
};

#endif /* POLISHMAPWRITER_H_INCLUDED */
