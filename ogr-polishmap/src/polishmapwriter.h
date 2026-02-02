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
#include <string>
#include <map>

/************************************************************************/
/*                          PolishMapWriter                              */
/*                                                                      */
/* Story 2.1: Skeleton writer for Polish Map format output.             */
/* Handles file I/O using GDAL VSI abstraction layer.                   */
/************************************************************************/

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
     */
    bool WriteHeader(const std::map<std::string, std::string>& aoMetadata);

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

private:
    VSILFILE* m_fpOutput;      // Borrowed file handle (NOT owned)
    bool m_bHeaderWritten;     // Track if header section was written

    /**
     * @brief Convert UTF-8 string to CP1252 encoding.
     * @param osUTF8Value UTF-8 encoded string
     * @return CP1252 encoded string, or original string on conversion failure
     *
     * Story 2.2 Task 4: Uses CPLRecode for conversion.
     * On failure, logs CE_Warning and returns original value as fallback.
     */
    std::string RecodeToCP1252(const std::string& osUTF8Value);
};

#endif /* POLISHMAPWRITER_H_INCLUDED */
