/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Orchestrator for writing complete Garmin IMG files
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#ifndef GARMINIMGWRITER_H_INCLUDED
#define GARMINIMGWRITER_H_INCLUDED

#include "garminimglblwriter.h"
#include "garminimgtrewriter.h"
#include "garminimgrgnwriter.h"
#include "garminimgnetwriter.h"
#include "garminimgnodwriter.h"
#include "garminimgbitwriter.h"
#include "garminimgbitreader.h"
#include "garminimgrgnparser.h"

#include <cstdint>
#include <memory>
#include <string>
#include <vector>

/************************************************************************/
/*                      WriterFeature                                   */
/************************************************************************/

struct WriterFeature {
    int nLayerType = 0;  // 0=POI, 1=POLYLINE, 2=POLYGON, 3=ROAD, 4=NODE
    uint16_t nType = 0;
    uint16_t nSubType = 0;
    std::string osLabel;
    std::vector<RGNPoint> aoPoints;

    // Road attributes
    int nRoadClass = 0;
    int nSpeed = 0;
    bool bOneWay = false;
    bool bToll = false;
    uint8_t nAccessFlags = 0;
    double dfLengthM = 0.0;
    bool bDirectionIndicator = false;
};

/************************************************************************/
/*                     GarminIMGWriter                                  */
/************************************************************************/

class GarminIMGWriter {
public:
    GarminIMGWriter();
    ~GarminIMGWriter();

    void SetMapName(const std::string& osName) { m_osMapName = osName; }
    void SetMapID(uint32_t nID) { m_nMapID = nID; }
    void SetLabelFormat(uint8_t nFormat) { m_nLabelFormat = nFormat; }
    void SetCodepage(uint16_t nCodepage) { m_nCodepage = nCodepage; }
    void SetTransparent(bool b) { m_bTransparent = b; }
    void SetDrawPriority(int n) { m_nDrawPriority = n; }
    void SetTYPFile(const std::string& osPath) { m_osTypFile = osPath; }

    void AddFeature(const WriterFeature& oFeature);
    bool Flush(const std::string& osFilename);

private:
    std::string m_osMapName;
    uint32_t m_nMapID = 1;
    uint8_t m_nLabelFormat = 6;
    uint16_t m_nCodepage = 1252;
    bool m_bTransparent = false;
    int m_nDrawPriority = 25;
    std::string m_osTypFile;

    std::vector<WriterFeature> m_aoFeatures;

    bool AssembleIMG(const std::string& osFilename,
                     const std::vector<uint8_t>& abyTRE,
                     const std::vector<uint8_t>& abyRGN,
                     const std::vector<uint8_t>& abyLBL,
                     const std::vector<uint8_t>& abyNET,
                     const std::vector<uint8_t>& abyNOD);
};

#endif /* GARMINIMGWRITER_H_INCLUDED */
