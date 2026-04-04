/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  TYP to QML conversion for QGIS symbology
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#ifndef TYP2QML_H_INCLUDED
#define TYP2QML_H_INCLUDED

#include "garminimgtypparser.h"

#include <string>

bool ConvertTypToQML(GarminIMGTYPParser& parser,
                     const std::string& osOutputDir,
                     const std::string& osBaseName,
                     const std::string& osPalette = "day");

#endif /* TYP2QML_H_INCLUDED */
