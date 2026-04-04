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

#ifndef GARMINIMGFILESYSTEM_H_INCLUDED
#define GARMINIMGFILESYSTEM_H_INCLUDED

#include "cpl_vsi.h"
#include "cpl_error.h"

#include <cstdint>
#include <map>
#include <string>
#include <vector>
#include <set>

/************************************************************************/
/*                      IMGSubfile                                      */
/************************************************************************/

struct IMGSubfile {
    std::string osFilename;   // e.g., "MAPNAME"
    std::string osExtension;  // e.g., "TRE"
    std::string osFullName;   // e.g., "MAPNAME.TRE"
    uint32_t nSize = 0;
    std::vector<uint8_t> abyData;
};

/************************************************************************/
/*                      IMGHeaderInfo                                   */
/************************************************************************/

struct IMGHeaderInfo {
    uint16_t nBlockSizeExp1 = 0;
    uint16_t nBlockSizeExp2 = 0;
    uint32_t nBlockSize = 0;
    std::string osDescription;
    bool bValid = false;
};

/************************************************************************/
/*                     GarminIMGFilesystem                              */
/************************************************************************/

class GarminIMGFilesystem {
public:
    GarminIMGFilesystem();
    ~GarminIMGFilesystem();

    bool Parse(const char* pszFilename);

    const IMGHeaderInfo& GetHeaderInfo() const { return m_oHeaderInfo; }

    // Get all subfiles
    const std::map<std::string, IMGSubfile>& GetSubfiles() const {
        return m_aoSubfiles;
    }

    // Get subfile data by full name (e.g., "MAPNAME.TRE")
    const std::vector<uint8_t>* GetSubfileData(const std::string& osFullName) const;

    // Get unique tile names (filenames without extensions)
    std::vector<std::string> GetTileNames() const;

    // Check if multi-tile gmapsupp
    bool IsMultiTile() const;

private:
    VSILFILE* m_fpInput = nullptr;
    IMGHeaderInfo m_oHeaderInfo;
    std::map<std::string, IMGSubfile> m_aoSubfiles;

    bool ParseHeader();
    bool ParseDirectory();
    bool ReadSubfileData();
};

#endif /* GARMINIMGFILESYSTEM_H_INCLUDED */
