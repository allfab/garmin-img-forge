/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Parser for Garmin IMG FAT-like filesystem container
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

#include "garminimgfilesystem.h"
#include "cpl_conv.h"
#include "cpl_error.h"

#include <algorithm>
#include <cstring>
#include <set>

/************************************************************************/
/*                     GarminIMGFilesystem()                            */
/************************************************************************/

GarminIMGFilesystem::GarminIMGFilesystem() {
}

/************************************************************************/
/*                    ~GarminIMGFilesystem()                            */
/************************************************************************/

GarminIMGFilesystem::~GarminIMGFilesystem() {
    if (m_fpInput) {
        VSIFCloseL(m_fpInput);
        m_fpInput = nullptr;
    }
}

/************************************************************************/
/*                             Parse()                                  */
/************************************************************************/

bool GarminIMGFilesystem::Parse(const char* pszFilename) {
    m_fpInput = VSIFOpenL(pszFilename, "rb");
    if (!m_fpInput) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "GarminIMG: Cannot open file: %s", pszFilename);
        return false;
    }

    if (!ParseHeader()) {
        return false;
    }

    if (!ParseDirectory()) {
        return false;
    }

    if (!ReadSubfileData()) {
        return false;
    }

    return true;
}

/************************************************************************/
/*                          ParseHeader()                               */
/************************************************************************/

bool GarminIMGFilesystem::ParseHeader() {
    uint8_t abyHeader[512];

    VSIFSeekL(m_fpInput, 0, SEEK_SET);
    if (VSIFReadL(abyHeader, 1, 512, m_fpInput) != 512) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "GarminIMG: Cannot read IMG header (need 512 bytes)");
        return false;
    }

    // Check magic "DSKIMG\0" at offset 0x10
    if (memcmp(abyHeader + 0x10, "DSKIMG\0", 7) != 0) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG: Missing DSKIMG magic at offset 0x10");
        return false;
    }

    // Check "GARMIN\0" at offset 0x41
    if (memcmp(abyHeader + 0x41, "GARMIN\0", 7) != 0) {
        CPLDebug("OGR_GARMINIMG", "Missing GARMIN signature at 0x41 (non-fatal)");
    }

    // Block size: 2^(e1 + e2)
    m_oHeaderInfo.nBlockSizeExp1 = abyHeader[0x61];
    m_oHeaderInfo.nBlockSizeExp2 = abyHeader[0x62];
    m_oHeaderInfo.nBlockSize = 1u << (m_oHeaderInfo.nBlockSizeExp1 +
                                       m_oHeaderInfo.nBlockSizeExp2);

    if (m_oHeaderInfo.nBlockSize < 512 || m_oHeaderInfo.nBlockSize > 65536) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG: Invalid block size: %u (exp1=%u, exp2=%u)",
                 m_oHeaderInfo.nBlockSize,
                 m_oHeaderInfo.nBlockSizeExp1,
                 m_oHeaderInfo.nBlockSizeExp2);
        return false;
    }

    // Check partition signature
    if (abyHeader[0x1FE] != 0x55 || abyHeader[0x1FF] != 0xAA) {
        CPLDebug("OGR_GARMINIMG", "Missing partition signature 0x55AA (non-fatal)");
    }

    // Extract description from header
    char szDesc[21];
    memcpy(szDesc, abyHeader + 0x49, 20);
    szDesc[20] = '\0';
    m_oHeaderInfo.osDescription = szDesc;

    m_oHeaderInfo.bValid = true;

    CPLDebug("OGR_GARMINIMG", "IMG header: block_size=%u (%u+%u), desc='%s'",
             m_oHeaderInfo.nBlockSize,
             m_oHeaderInfo.nBlockSizeExp1,
             m_oHeaderInfo.nBlockSizeExp2,
             m_oHeaderInfo.osDescription.c_str());

    return true;
}

/************************************************************************/
/*                        ParseDirectory()                              */
/************************************************************************/

bool GarminIMGFilesystem::ParseDirectory() {
    const uint32_t nBlockSize = m_oHeaderInfo.nBlockSize;

    // Get file size to know when to stop
    VSIFSeekL(m_fpInput, 0, SEEK_END);
    vsi_l_offset nFileSize = VSIFTellL(m_fpInput);

    // Temporary storage for multi-part files
    struct DirEntry {
        std::string osFilename;
        std::string osExtension;
        uint32_t nSize = 0;
        uint16_t nPart = 0;
        std::vector<uint16_t> anBlocks;
    };

    // key = "FILENAME.EXT", value = list of parts
    std::map<std::string, std::vector<DirEntry>> aoEntries;

    // Directory entries are always 512 bytes each, starting at offset 0x200.
    // The directory occupies all 512-byte slots from 0x200 up to the first
    // data block. We scan until we've seen enough consecutive empty entries
    // or we hit the data area.
    //
    // Entry types:
    //   flag 0x00 = empty/padding (NOT necessarily end of directory)
    //   flag 0x01 = regular subfile
    //   flag 0x03 = header blocks marker
    //   other     = skip
    //
    // Header blocks entries (flag 0x01 with all-space filename+extension)
    // are also skipped — they reference the IMG header itself.

    vsi_l_offset nOffset = 0x200;
    uint8_t abyEntry[512];

    // Limit directory scanning: stop after first data block or
    // after 2048 entries (1 MB of directory — more than any IMG)
    int nConsecutiveEmpty = 0;
    const int nMaxEntries = 2048;
    int nEntriesScanned = 0;

    while (nOffset + 512 <= nFileSize && nEntriesScanned < nMaxEntries) {
        VSIFSeekL(m_fpInput, nOffset, SEEK_SET);
        if (VSIFReadL(abyEntry, 1, 512, m_fpInput) != 512) {
            break;
        }

        uint8_t nFlag = abyEntry[0x00];
        nEntriesScanned++;

        if (nFlag == 0x00) {
            // Empty/padding entry — continue scanning, don't stop
            nConsecutiveEmpty++;
            // If we see many consecutive empties after finding some entries,
            // we've likely passed the directory
            if (nConsecutiveEmpty > 4 && !aoEntries.empty()) {
                break;
            }
            nOffset += 512;
            continue;
        }

        nConsecutiveEmpty = 0;

        if (nFlag == 0x03) {
            // Header blocks entry, skip
            nOffset += 512;
            continue;
        }

        if (nFlag != 0x01) {
            // Unknown flag, skip
            nOffset += 512;
            continue;
        }

        // Extract filename (8 chars at 0x01, space-padded)
        char szFilename[9];
        memcpy(szFilename, abyEntry + 0x01, 8);
        szFilename[8] = '\0';
        // Trim trailing spaces
        for (int i = 7; i >= 0; i--) {
            if (szFilename[i] == ' ') szFilename[i] = '\0';
            else break;
        }

        // Extract extension (3 chars at 0x09)
        char szExt[4];
        memcpy(szExt, abyEntry + 0x09, 3);
        szExt[3] = '\0';
        for (int i = 2; i >= 0; i--) {
            if (szExt[i] == ' ') szExt[i] = '\0';
            else break;
        }

        // Skip header blocks entries (flag 0x01 but filename+extension empty)
        if (szFilename[0] == '\0' || szExt[0] == '\0') {
            nOffset += 512;
            continue;
        }

        std::string osFullName = std::string(szFilename) + "." + szExt;

        // File size (LE32 at 0x0C, only valid for part 0)
        uint32_t nSize = static_cast<uint32_t>(abyEntry[0x0C]) |
                         (static_cast<uint32_t>(abyEntry[0x0D]) << 8) |
                         (static_cast<uint32_t>(abyEntry[0x0E]) << 16) |
                         (static_cast<uint32_t>(abyEntry[0x0F]) << 24);

        // Part number (LE16 at 0x11)
        uint16_t nPart = static_cast<uint16_t>(abyEntry[0x11]) |
                         (static_cast<uint16_t>(abyEntry[0x12]) << 8);

        // Block numbers (240 × LE16 at 0x20)
        DirEntry oEntry;
        oEntry.osFilename = szFilename;
        oEntry.osExtension = szExt;
        oEntry.nSize = nSize;
        oEntry.nPart = nPart;

        for (int i = 0; i < 240; i++) {
            uint16_t nBlock = static_cast<uint16_t>(abyEntry[0x20 + i * 2]) |
                              (static_cast<uint16_t>(abyEntry[0x20 + i * 2 + 1]) << 8);
            if (nBlock == 0xFFFF) break;
            oEntry.anBlocks.push_back(nBlock);
        }

        aoEntries[osFullName].push_back(std::move(oEntry));
        nOffset += 512;
    }

    // Merge multi-part entries and store
    for (auto& [osFullName, aoParts] : aoEntries) {
        // Sort by part number
        std::sort(aoParts.begin(), aoParts.end(),
                  [](const DirEntry& a, const DirEntry& b) {
                      return a.nPart < b.nPart;
                  });

        IMGSubfile oSubfile;
        oSubfile.osFullName = osFullName;
        oSubfile.osFilename = aoParts[0].osFilename;
        oSubfile.osExtension = aoParts[0].osExtension;
        oSubfile.nSize = aoParts[0].nSize;  // Size from part 0

        // Collect all blocks from all parts
        std::vector<uint16_t> anAllBlocks;
        for (const auto& oPart : aoParts) {
            anAllBlocks.insert(anAllBlocks.end(),
                               oPart.anBlocks.begin(),
                               oPart.anBlocks.end());
        }

        // Pre-allocate data
        oSubfile.abyData.reserve(oSubfile.nSize);

        // Read block data
        for (uint16_t nBlock : anAllBlocks) {
            vsi_l_offset nBlockOffset =
                static_cast<vsi_l_offset>(nBlock) * nBlockSize;

            // How many bytes to read from this block
            if (oSubfile.abyData.size() >= oSubfile.nSize) break;
            uint32_t nRemaining = oSubfile.nSize -
                                  static_cast<uint32_t>(oSubfile.abyData.size());
            uint32_t nToRead = std::min(nBlockSize, nRemaining);

            if (nToRead == 0) break;

            std::vector<uint8_t> abyBlock(nToRead);
            VSIFSeekL(m_fpInput, nBlockOffset, SEEK_SET);
            if (VSIFReadL(abyBlock.data(), 1, nToRead, m_fpInput) != nToRead) {
                CPLError(CE_Warning, CPLE_FileIO,
                         "GarminIMG: Short read on block %u for %s",
                         nBlock, osFullName.c_str());
                break;
            }

            oSubfile.abyData.insert(oSubfile.abyData.end(),
                                    abyBlock.begin(), abyBlock.end());
        }

        CPLDebug("OGR_GARMINIMG", "Subfile: %s, size=%u, blocks=%zu, read=%zu",
                 osFullName.c_str(), oSubfile.nSize,
                 anAllBlocks.size(), oSubfile.abyData.size());

        m_aoSubfiles[osFullName] = std::move(oSubfile);
    }

    return !m_aoSubfiles.empty();
}

/************************************************************************/
/*                        ReadSubfileData()                             */
/************************************************************************/

bool GarminIMGFilesystem::ReadSubfileData() {
    // Data already read during ParseDirectory
    return true;
}

/************************************************************************/
/*                        GetSubfileData()                              */
/************************************************************************/

const std::vector<uint8_t>*
GarminIMGFilesystem::GetSubfileData(const std::string& osFullName) const {
    auto it = m_aoSubfiles.find(osFullName);
    if (it != m_aoSubfiles.end()) {
        return &(it->second.abyData);
    }
    return nullptr;
}

/************************************************************************/
/*                         GetTileNames()                               */
/************************************************************************/

std::vector<std::string> GarminIMGFilesystem::GetTileNames() const {
    std::set<std::string> aoTileNames;

    for (const auto& [osFullName, oSubfile] : m_aoSubfiles) {
        // Only consider subfiles with TRE extension as tile markers
        if (oSubfile.osExtension == "TRE") {
            aoTileNames.insert(oSubfile.osFilename);
        }
    }

    return std::vector<std::string>(aoTileNames.begin(), aoTileNames.end());
}

/************************************************************************/
/*                         IsMultiTile()                                */
/************************************************************************/

bool GarminIMGFilesystem::IsMultiTile() const {
    return GetTileNames().size() > 1;
}
