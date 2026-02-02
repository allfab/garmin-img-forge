/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Declaration of OGRPolishMapDataSource class
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

#ifndef OGRPOLISHMAPDATASOURCE_H_INCLUDED
#define OGRPOLISHMAPDATASOURCE_H_INCLUDED

#include "gdal_priv.h"
#include "ogrsf_frmts.h"
#include "polishmapparser.h"
#include <vector>
#include <memory>
#include <map>
#include <string>

class OGRPolishMapLayer;
class PolishMapWriter;

/************************************************************************/
/*                       OGRPolishMapDataSource                         */
/************************************************************************/

class OGRPolishMapDataSource final : public GDALDataset {
private:
    std::vector<std::unique_ptr<OGRPolishMapLayer>> m_apoLayers;
    CPLString m_osFilename;
    PolishMapHeaderData m_oHeaderData;  // Story 1.2: Header metadata storage
    std::unique_ptr<PolishMapParser> m_poParser;  // Story 1.4: Parser for feature reading

    // Story 2.1: Write mode support
    bool m_bUpdate;                              // false = read (Open), true = write (Create)
    VSILFILE* m_fpOutput;                        // Output file handle (write mode only)
    std::unique_ptr<PolishMapWriter> m_poWriter; // Writer instance (write mode only)

    // Story 2.2: Metadata storage for Polish Map header fields
    std::map<std::string, std::string> m_aoMetadata;

    // Story 1.3: Initialize the 3 layers (POI, POLYLINE, POLYGON)
    void CreateLayers();

    // Story 2.1: Create layers for write mode (no parser needed)
    void CreateLayersForWriteMode();

public:
    OGRPolishMapDataSource();
    ~OGRPolishMapDataSource() override;

    // GDALDataset pure virtual methods
    int GetLayerCount() override;

    /** @brief Returns layer by index.
     *  @param nLayer Layer index (0-2 valid)
     *  @return OGRLayer pointer or nullptr if index invalid.
     *  @note Ownership remains with dataset. Caller must NOT delete. */
    OGRLayer* GetLayer(int nLayer) override;

    int TestCapability(const char* pszCap) override;

    // Story 1.2: Header metadata access
    void SetHeaderData(const PolishMapHeaderData& oData) { m_oHeaderData = oData; }
    const PolishMapHeaderData& GetHeaderData() const { return m_oHeaderData; }

    // Story 1.4: Parser access
    void SetParser(std::unique_ptr<PolishMapParser> poParser);
    PolishMapParser* GetParser() { return m_poParser.get(); }

    // Story 2.1: Write mode factory method
    static OGRPolishMapDataSource* Create(const char* pszFilename);

    // Story 2.1: Check if in write mode
    bool IsUpdateMode() const { return m_bUpdate; }

    // Story 2.2: Metadata API override for Polish Map header fields
    CPLErr SetMetadataItem(const char* pszName, const char* pszValue,
                           const char* pszDomain = nullptr) override;
    const char* GetMetadataItem(const char* pszName,
                                const char* pszDomain = nullptr) override;

    /** @brief Access Polish Map metadata map (for writer).
     *  @return Const reference to internal metadata map (key=value pairs).
     *  @note Used internally by PolishMapWriter to generate [IMG ID] section.
     *  @since Story 2.2 */
    const std::map<std::string, std::string>& GetPolishMapMetadata() const { return m_aoMetadata; }
};

#endif /* OGRPOLISHMAPDATASOURCE_H_INCLUDED */
