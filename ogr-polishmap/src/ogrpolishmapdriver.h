/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Driver registration and identification for Polish Map format
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

#ifndef OGRPOLISHMAPDRIVER_H_INCLUDED
#define OGRPOLISHMAPDRIVER_H_INCLUDED

#include "gdal_priv.h"

/************************************************************************/
/*                        OGRPolishMapDriver                            */
/************************************************************************/

/**
 * @class OGRPolishMapDriver
 * @brief GDAL/OGR driver for Polish Map (.mp) format files.
 *
 * This driver enables reading and writing of Polish Map format files,
 * which are text-based vector files used for creating Garmin GPS maps.
 * The driver supports three geometry types: POI (Point), POLYLINE (LineString),
 * and POLYGON (Polygon).
 *
 * @section capabilities Driver Capabilities
 * - Read: Yes (GA_ReadOnly)
 * - Write: Yes (via Create())
 * - Supports VirtualIO: Yes (VSI file abstraction)
 * - Supports Georeferencing: Yes (WGS84/EPSG:4326)
 *
 * @section usage Usage Example
 * @code
 * // Reading
 * GDALDataset* ds = (GDALDataset*) GDALOpenEx("file.mp", GDAL_OF_VECTOR, NULL, NULL, NULL);
 *
 * // Writing
 * GDALDriver* driver = GetGDALDriverManager()->GetDriverByName("PolishMap");
 * GDALDataset* ds = driver->Create("output.mp", 0, 0, 0, GDT_Unknown, NULL);
 * @endcode
 *
 * @see OGRPolishMapDataSource
 * @see OGRPolishMapLayer
 */
class OGRPolishMapDriver final : public GDALDriver {
public:
    /**
     * @brief Default constructor.
     *
     * Initializes the driver with metadata including short name ("PolishMap"),
     * long name, and supported capabilities (create, virtual I/O).
     */
    OGRPolishMapDriver();

    /**
     * @brief Destructor.
     */
    ~OGRPolishMapDriver() override;

    /**
     * @brief Identify if a file is a Polish Map format file.
     *
     * Checks if the given file is a valid Polish Map file by examining
     * the file extension (.mp) and/or content signature ([IMG ID] header).
     *
     * @param poOpenInfo GDAL open info structure containing file path and header bytes.
     * @return TRUE if the file appears to be a Polish Map file, FALSE otherwise.
     *
     * @note This method performs a quick check without fully parsing the file.
     *       It examines the first 1024 bytes for the [IMG ID] section marker.
     */
    static int Identify(GDALOpenInfo* poOpenInfo);

    /**
     * @brief Open a Polish Map file for reading.
     *
     * Opens the specified Polish Map file and creates a GDALDataset object.
     * The file is parsed to extract header metadata and prepare layers for
     * feature reading.
     *
     * @param poOpenInfo GDAL open info structure containing file path and access mode.
     * @return Pointer to GDALDataset on success, nullptr on failure.
     *         Caller is responsible for destroying the returned dataset with GDALClose().
     *
     * @note On failure, CPLError() is called with appropriate error message.
     * @note Only GA_ReadOnly access is supported; GA_Update returns nullptr.
     *
     * @see Identify()
     * @see Create()
     */
    static GDALDataset* Open(GDALOpenInfo* poOpenInfo);

    /**
     * @brief Create a new Polish Map file for writing.
     *
     * Creates a new Polish Map file with empty POI, POLYLINE, and POLYGON layers.
     * Features can then be added to these layers using ICreateFeature().
     *
     * @param pszName Output file path for the new Polish Map file.
     * @param nXSize Not used (must be 0 for vector drivers).
     * @param nYSize Not used (must be 0 for vector drivers).
     * @param nBands Not used (must be 0 for vector drivers).
     * @param eType Not used (should be GDT_Unknown for vector drivers).
     * @param papszOptions Creation options (NAME, CODEPAGE, ID).
     * @return Pointer to new GDALDataset on success, nullptr on failure.
     *         Caller is responsible for destroying the returned dataset with GDALClose().
     *
     * @note Supported creation options:
     *       - NAME: Map name for [IMG ID] header (default: filename)
     *       - CODEPAGE: Character encoding (default: "1252")
     *       - ID: Map identifier (default: auto-generated)
     *
     * @see Open()
     */
    static GDALDataset* Create(const char* pszName, int nXSize, int nYSize,
                               int nBands, GDALDataType eType, char** papszOptions);
};

// Visibility macro for exported symbols
#if defined(__GNUC__) || defined(__clang__)
#  define OGR_POLISHMAP_EXPORT __attribute__((visibility("default")))
#elif defined(_MSC_VER)
#  define OGR_POLISHMAP_EXPORT __declspec(dllexport)
#else
#  define OGR_POLISHMAP_EXPORT
#endif

/**
 * @name Registration Functions
 * @brief C-style functions for driver registration and plugin loading.
 * @{
 */

extern "C" {
    /**
     * @brief Register the OGR PolishMap driver with GDAL.
     *
     * This function creates and registers the PolishMap driver with the
     * GDAL driver manager. It sets up driver metadata including short name,
     * long name, help topic, and function pointers for Open, Create, and Identify.
     *
     * @note Called automatically during GDAL initialization or plugin loading.
     * @note Safe to call multiple times; subsequent calls are no-ops.
     */
    OGR_POLISHMAP_EXPORT void RegisterOGRPolishMap();

    /**
     * @brief Standard GDAL plugin entry point.
     *
     * This function is the standard entry point for GDAL plugins.
     * It simply calls RegisterOGRPolishMap() to register the driver.
     *
     * @note This function is required for the driver to work as a plugin.
     * @note The function name must be exactly "GDALRegisterMe" for GDAL to find it.
     */
    OGR_POLISHMAP_EXPORT void GDALRegisterMe();
}

/** @} */ // end of Registration Functions group

#endif /* OGRPOLISHMAPDRIVER_H_INCLUDED */
