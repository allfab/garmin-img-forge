/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Orchestrator for writing complete Garmin IMG files
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#include "garminimgwriter.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include "cpl_vsi.h"

#include <algorithm>
#include <cmath>
#include <cstring>

GarminIMGWriter::GarminIMGWriter() {
}

GarminIMGWriter::~GarminIMGWriter() {
}

void GarminIMGWriter::AddFeature(const WriterFeature& oFeature) {
    m_aoFeatures.push_back(oFeature);
}

bool GarminIMGWriter::Flush(const std::string& osFilename) {
    if (m_aoFeatures.empty()) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "GarminIMG Writer: No features to write");
    }

    // Step 1: Build LBL (labels)
    GarminIMGLBLWriter oLBL;
    oLBL.SetEncodingFormat(m_nLabelFormat);
    oLBL.SetCodepage(m_nCodepage);

    std::vector<uint32_t> anLabelOffsets;
    for (const auto& oFeat : m_aoFeatures) {
        anLabelOffsets.push_back(oLBL.AddLabel(oFeat.osLabel));
    }

    auto abyLBL = oLBL.Build();

    // Step 2: Build RGN (geometries)
    GarminIMGRGNWriter oRGN;

    // Calculate bounds
    double dfMinLon = 180.0, dfMaxLon = -180.0;
    double dfMinLat = 90.0, dfMaxLat = -90.0;

    for (const auto& oFeat : m_aoFeatures) {
        for (const auto& oPt : oFeat.aoPoints) {
            dfMinLon = std::min(dfMinLon, oPt.dfLon);
            dfMaxLon = std::max(dfMaxLon, oPt.dfLon);
            dfMinLat = std::min(dfMinLat, oPt.dfLat);
            dfMaxLat = std::max(dfMaxLat, oPt.dfLat);
        }
    }

    if (dfMinLon > dfMaxLon) {
        dfMinLon = dfMaxLon = 0.0;
        dfMinLat = dfMaxLat = 0.0;
    }

    // Subdivision center
    int32_t nCenterLon = GarminIMGBitReader::DegreesToMapUnits(
        (dfMinLon + dfMaxLon) / 2.0);
    int32_t nCenterLat = GarminIMGBitReader::DegreesToMapUnits(
        (dfMinLat + dfMaxLat) / 2.0);

    int nResolution = 24;
    int nShift = 24 - nResolution;

    uint8_t nContentFlags = 0;

    for (size_t i = 0; i < m_aoFeatures.size(); i++) {
        const auto& oFeat = m_aoFeatures[i];
        uint32_t nLabelOff = anLabelOffsets[i];

        if (oFeat.nLayerType == 0) {
            // POI
            nContentFlags |= 0x10;
            int16_t nDeltaLon = static_cast<int16_t>(
                (GarminIMGBitReader::DegreesToMapUnits(oFeat.aoPoints[0].dfLon) - nCenterLon) >> nShift);
            int16_t nDeltaLat = static_cast<int16_t>(
                (GarminIMGBitReader::DegreesToMapUnits(oFeat.aoPoints[0].dfLat) - nCenterLat) >> nShift);

            bool bHasSub = (oFeat.nSubType != 0);
            oRGN.WritePOI(static_cast<uint8_t>(oFeat.nType), nLabelOff,
                          nDeltaLon, nDeltaLat,
                          static_cast<uint8_t>(oFeat.nSubType), bHasSub);
        } else if (oFeat.nLayerType == 1 || oFeat.nLayerType == 2) {
            // POLYLINE or POLYGON
            if (oFeat.nLayerType == 1) nContentFlags |= 0x40;
            else nContentFlags |= 0x80;

            if (oFeat.aoPoints.size() < 2) continue;

            int16_t nFirstDeltaLon = static_cast<int16_t>(
                (GarminIMGBitReader::DegreesToMapUnits(oFeat.aoPoints[0].dfLon) - nCenterLon) >> nShift);
            int16_t nFirstDeltaLat = static_cast<int16_t>(
                (GarminIMGBitReader::DegreesToMapUnits(oFeat.aoPoints[0].dfLat) - nCenterLat) >> nShift);

            // Build bitstream for remaining points
            GarminIMGBitWriter oBits;

            // Calculate deltas for base determination
            std::vector<int32_t> anDeltaX, anDeltaY;
            int32_t nPrevLon = GarminIMGBitReader::DegreesToMapUnits(oFeat.aoPoints[0].dfLon);
            int32_t nPrevLat = GarminIMGBitReader::DegreesToMapUnits(oFeat.aoPoints[0].dfLat);

            for (size_t j = 1; j < oFeat.aoPoints.size(); j++) {
                int32_t nLon = GarminIMGBitReader::DegreesToMapUnits(oFeat.aoPoints[j].dfLon);
                int32_t nLat = GarminIMGBitReader::DegreesToMapUnits(oFeat.aoPoints[j].dfLat);
                anDeltaX.push_back((nLon - nPrevLon) >> nShift);
                anDeltaY.push_back((nLat - nPrevLat) >> nShift);
                nPrevLon = nLon;
                nPrevLat = nLat;
            }

            // Determine base sizes
            int nMaxAbsX = 0, nMaxAbsY = 0;
            for (auto dx : anDeltaX) nMaxAbsX = std::max(nMaxAbsX, std::abs(static_cast<int>(dx)));
            for (auto dy : anDeltaY) nMaxAbsY = std::max(nMaxAbsY, std::abs(static_cast<int>(dy)));

            int nXBitsNeeded = GarminIMGBitWriter::BitsNeeded(nMaxAbsX);
            int nYBitsNeeded = GarminIMGBitWriter::BitsNeeded(nMaxAbsY);

            int nXBase = std::max(0, GarminIMGBitReader::Bits2Base(nXBitsNeeded));
            int nYBase = std::max(0, GarminIMGBitReader::Bits2Base(nYBitsNeeded));

            int nXBits = GarminIMGBitReader::Base2Bits(nXBase) + 1;  // +1 for sign
            int nYBits = GarminIMGBitReader::Base2Bits(nYBase) + 1;

            // Bitstream header (16 bits)
            uint16_t nHeader = static_cast<uint16_t>(nXBase & 0x0F) |
                               (static_cast<uint16_t>(nYBase & 0x0F) << 4);
            // Not same sign → need sign bits
            oBits.PutN(nHeader, 16);

            // Encode deltas
            for (size_t j = 0; j < anDeltaX.size(); j++) {
                oBits.SPutN(anDeltaX[j], nXBits);
                oBits.SPutN(anDeltaY[j], nYBits);
            }

            auto abyBitstream = oBits.GetBuffer();

            if (oFeat.nLayerType == 1) {
                oRGN.WritePolyline(static_cast<uint8_t>(oFeat.nType), nLabelOff,
                                   nFirstDeltaLon, nFirstDeltaLat,
                                   abyBitstream, oFeat.bDirectionIndicator);
            } else {
                oRGN.WritePolygon(static_cast<uint8_t>(oFeat.nType), nLabelOff,
                                  nFirstDeltaLon, nFirstDeltaLat,
                                  abyBitstream);
            }
        }
    }

    auto abyRGN = oRGN.Build();

    // Step 3: Build NET + NOD (for road features)
    GarminIMGNETWriter oNET;
    GarminIMGNODWriter oNOD;
    bool bHasRouting = false;

    for (size_t i = 0; i < m_aoFeatures.size(); i++) {
        const auto& oFeat = m_aoFeatures[i];
        if (oFeat.nLayerType == 3) {
            std::vector<uint32_t> anLabels = { anLabelOffsets[i] };
            oNET.AddRoad(anLabels, 0, oFeat.nRoadClass, oFeat.nSpeed,
                         oFeat.bOneWay, oFeat.bToll, oFeat.nAccessFlags,
                         oFeat.dfLengthM);
            bHasRouting = true;
        } else if (oFeat.nLayerType == 4 && !oFeat.aoPoints.empty()) {
            oNOD.AddNode(oFeat.aoPoints[0].dfLat, oFeat.aoPoints[0].dfLon, {});
            bHasRouting = true;
        }
    }

    std::vector<uint8_t> abyNET, abyNOD;
    if (bHasRouting) {
        abyNET = oNET.Build();
        abyNOD = oNOD.Build();
    }

    // Step 4: Build TRE (index spatial)
    GarminIMGTREWriter oTRE;
    oTRE.SetBounds(dfMaxLat, dfMaxLon, dfMinLat, dfMinLon);
    oTRE.SetMapID(m_nMapID);
    oTRE.SetMapProperties(bHasRouting, m_bTransparent, m_nDrawPriority);
    oTRE.AddLevel(nResolution, false);

    TREWriterSubdiv oSubdiv;
    oSubdiv.nRGNOffset = 0;
    oSubdiv.nContentFlags = nContentFlags;
    oSubdiv.nCenterLon = nCenterLon;
    oSubdiv.nCenterLat = nCenterLat;

    int32_t nHalfW = GarminIMGBitReader::DegreesToMapUnits((dfMaxLon - dfMinLon) / 2.0);
    int32_t nHalfH = GarminIMGBitReader::DegreesToMapUnits((dfMaxLat - dfMinLat) / 2.0);
    oSubdiv.nWidth = static_cast<uint16_t>(std::min(static_cast<int32_t>(0x7FFF), std::abs(nHalfW)));
    oSubdiv.nHeight = static_cast<uint16_t>(std::min(static_cast<int32_t>(0x7FFF), std::abs(nHalfH)));
    oSubdiv.bLastSubdiv = true;

    oTRE.AddSubdivision(oSubdiv);

    uint32_t nLastRGNPos = oRGN.GetCurrentOffset();
    auto abyTRE = oTRE.Build(nLastRGNPos);

    // Step 5: Assemble IMG file
    return AssembleIMG(osFilename, abyTRE, abyRGN, abyLBL, abyNET, abyNOD);
}

/************************************************************************/
/*                        AssembleIMG()                                 */
/************************************************************************/

bool GarminIMGWriter::AssembleIMG(
    const std::string& osFilename,
    const std::vector<uint8_t>& abyTRE,
    const std::vector<uint8_t>& abyRGN,
    const std::vector<uint8_t>& abyLBL,
    const std::vector<uint8_t>& abyNET,
    const std::vector<uint8_t>& abyNOD) {

    VSILFILE* fp = VSIFOpenL(osFilename.c_str(), "wb");
    if (!fp) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "GarminIMG Writer: Cannot create file: %s", osFilename.c_str());
        return false;
    }

    // Block size = 512
    const uint32_t nBlockSize = 512;
    const uint8_t nExp1 = 9, nExp2 = 0;

    // Subfiles to write
    struct SubfileEntry {
        std::string osName;
        std::string osExt;
        const std::vector<uint8_t>* pabyData;
    };

    std::string osTileName = "MAPNAME0";
    if (m_osMapName.size() <= 8) {
        osTileName = m_osMapName;
    }
    while (osTileName.size() < 8) osTileName += ' ';

    std::vector<SubfileEntry> aoSubfiles;
    aoSubfiles.push_back({osTileName, "TRE", &abyTRE});
    aoSubfiles.push_back({osTileName, "RGN", &abyRGN});
    aoSubfiles.push_back({osTileName, "LBL", &abyLBL});
    if (!abyNET.empty()) {
        aoSubfiles.push_back({osTileName, "NET", &abyNET});
    }
    if (!abyNOD.empty()) {
        aoSubfiles.push_back({osTileName, "NOD", &abyNOD});
    }

    // Calculate block allocations
    // Header = 1 block, directory = enough blocks for entries
    int nDirEntries = 1 + static_cast<int>(aoSubfiles.size());  // +1 for header entry
    int nDirBlocks = (nDirEntries * 512 + nBlockSize - 1) / nBlockSize;

    int nDataStartBlock = 1 + nDirBlocks;
    std::vector<int> anStartBlocks;
    std::vector<int> anBlockCounts;

    int nCurrentBlock = nDataStartBlock;
    for (const auto& oSub : aoSubfiles) {
        anStartBlocks.push_back(nCurrentBlock);
        int nBlocks = (static_cast<int>(oSub.pabyData->size()) + nBlockSize - 1) / nBlockSize;
        if (nBlocks == 0) nBlocks = 1;
        anBlockCounts.push_back(nBlocks);
        nCurrentBlock += nBlocks;
    }

    // Write IMG header (block 0)
    uint8_t abyHeader[512];
    memset(abyHeader, 0, sizeof(abyHeader));

    // XOR byte at 0x00
    abyHeader[0x00] = 0x00;

    // Magic "DSKIMG\0" at 0x10
    memcpy(abyHeader + 0x10, "DSKIMG\0", 7);

    // Description at 0x49
    memset(abyHeader + 0x49, ' ', 20);
    size_t nDescLen = std::min(m_osMapName.size(), size_t(20));
    memcpy(abyHeader + 0x49, m_osMapName.c_str(), nDescLen);

    // "GARMIN\0" at 0x41
    memcpy(abyHeader + 0x41, "GARMIN\0", 7);

    // Block size exponents
    abyHeader[0x61] = nExp1;
    abyHeader[0x62] = nExp2;

    // Partition signature
    abyHeader[0x1FE] = 0x55;
    abyHeader[0x1FF] = 0xAA;

    VSIFWriteL(abyHeader, 1, sizeof(abyHeader), fp);

    // Write directory (starting at block 1 = offset 0x200)
    // Header entry (flag 0x03)
    {
        uint8_t abyDirEntry[512];
        memset(abyDirEntry, 0, sizeof(abyDirEntry));
        abyDirEntry[0x00] = 0x03;  // Header blocks flag
        memset(abyDirEntry + 0x01, ' ', 8);  // Filename
        memset(abyDirEntry + 0x09, ' ', 3);  // Extension
        // Block 0 reference
        abyDirEntry[0x20] = 0x00;
        abyDirEntry[0x21] = 0x00;
        // Fill rest of block list with 0xFFFF
        for (int i = 1; i < 240; i++) {
            abyDirEntry[0x20 + i * 2] = 0xFF;
            abyDirEntry[0x20 + i * 2 + 1] = 0xFF;
        }
        VSIFWriteL(abyDirEntry, 1, sizeof(abyDirEntry), fp);
    }

    // Subfile entries
    for (size_t i = 0; i < aoSubfiles.size(); i++) {
        uint8_t abyDirEntry[512];
        memset(abyDirEntry, 0, sizeof(abyDirEntry));

        abyDirEntry[0x00] = 0x01;  // Regular file

        // Filename (8 bytes, space-padded)
        memset(abyDirEntry + 0x01, ' ', 8);
        memcpy(abyDirEntry + 0x01, aoSubfiles[i].osName.c_str(),
               std::min(size_t(8), aoSubfiles[i].osName.size()));

        // Extension (3 bytes)
        memset(abyDirEntry + 0x09, ' ', 3);
        memcpy(abyDirEntry + 0x09, aoSubfiles[i].osExt.c_str(),
               std::min(size_t(3), aoSubfiles[i].osExt.size()));

        // File size LE32 at 0x0C
        uint32_t nSize = static_cast<uint32_t>(aoSubfiles[i].pabyData->size());
        abyDirEntry[0x0C] = nSize & 0xFF;
        abyDirEntry[0x0D] = (nSize >> 8) & 0xFF;
        abyDirEntry[0x0E] = (nSize >> 16) & 0xFF;
        abyDirEntry[0x0F] = (nSize >> 24) & 0xFF;

        // Part number 0
        abyDirEntry[0x11] = 0x00;
        abyDirEntry[0x12] = 0x00;

        // Block numbers
        for (int b = 0; b < anBlockCounts[i] && b < 240; b++) {
            uint16_t nBlock = static_cast<uint16_t>(anStartBlocks[i] + b);
            abyDirEntry[0x20 + b * 2] = nBlock & 0xFF;
            abyDirEntry[0x20 + b * 2 + 1] = (nBlock >> 8) & 0xFF;
        }
        // Fill rest with 0xFFFF
        for (int b = anBlockCounts[i]; b < 240; b++) {
            abyDirEntry[0x20 + b * 2] = 0xFF;
            abyDirEntry[0x20 + b * 2 + 1] = 0xFF;
        }

        VSIFWriteL(abyDirEntry, 1, sizeof(abyDirEntry), fp);
    }

    // Pad directory to fill remaining blocks
    vsi_l_offset nCurrentPos = VSIFTellL(fp);
    vsi_l_offset nDataStart = static_cast<vsi_l_offset>(nDataStartBlock) * nBlockSize;
    while (nCurrentPos < nDataStart) {
        uint8_t zero = 0;
        VSIFWriteL(&zero, 1, 1, fp);
        nCurrentPos++;
    }

    // Write subfile data
    for (size_t i = 0; i < aoSubfiles.size(); i++) {
        vsi_l_offset nExpectedOffset = static_cast<vsi_l_offset>(anStartBlocks[i]) * nBlockSize;
        VSIFSeekL(fp, nExpectedOffset, SEEK_SET);
        VSIFWriteL(aoSubfiles[i].pabyData->data(), 1,
                   aoSubfiles[i].pabyData->size(), fp);

        // Pad to block boundary
        uint32_t nPad = anBlockCounts[i] * nBlockSize -
                        static_cast<uint32_t>(aoSubfiles[i].pabyData->size());
        for (uint32_t j = 0; j < nPad; j++) {
            uint8_t zero = 0;
            VSIFWriteL(&zero, 1, 1, fp);
        }
    }

    VSIFCloseL(fp);

    CPLDebug("OGR_GARMINIMG", "Writer: Wrote IMG file %s (%d features, %zu subfiles)",
             osFilename.c_str(), static_cast<int>(m_aoFeatures.size()),
             aoSubfiles.size());

    return true;
}
