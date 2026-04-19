/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Polish Map format parser - header file
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

#ifndef POLISHMAPPARSER_H_INCLUDED
#define POLISHMAPPARSER_H_INCLUDED

#include "cpl_port.h"
#include "cpl_string.h"
#include <map>
#include <string>
#include <vector>

/************************************************************************/
/*                            SectionType                               */
/************************************************************************/

/**
 * @enum SectionType
 * @brief Enumeration of Polish Map section types.
 *
 * Used by the parser to identify and handle different section types
 * in Polish Map files. Each section type corresponds to a geometry type.
 */
enum class SectionType {
    POI,        /**< Point of Interest - [POI] or [RGN10] sections (wkbPoint) */
    Polyline,   /**< Linear feature - [POLYLINE] sections (wkbLineString) */
    Polygon     /**< Area feature - [POLYGON] sections (wkbPolygon) */
};

/************************************************************************/
/*                        PolishMapSection                              */
/************************************************************************/

/**
 * @struct PolishMapSection
 * @brief Unified Intermediate Representation (IR) for all section types.
 *
 * This structure serves as a container for parsed section data from
 * Polish Map files. It handles POI, POLYLINE, and POLYGON sections
 * with a unified interface for the DRY ParseNextSection() method.
 *
 * @section lifecycle Memory Lifecycle
 * The structure is designed for reuse: parse a section, convert to
 * OGRFeature, then call Clear() to release memory before parsing the next.
 * This pattern minimizes memory usage for large files.
 *
 * @see PolishMapParser::ParseNextSection()
 */
struct PolishMapSection {
    SectionType eType;                               // Section type
    std::string osType;                              // Garmin type code "0x2C00", "0x16", etc.
    std::string osLabel;                             // UTF-8 after conversion
    std::vector<std::pair<double, double>> aoCoords; // Coordinates (1 for POI, N for POLY*)
    int nEndLevel;                                   // 0-9, -1 if absent
    std::string osLevels;                            // "0-3" or empty
    std::map<std::string, std::string> aoOtherFields;// Additional fields

    // Tech-spec #2 Task 4: additional coordinate sets for Data1=..Data9= lines
    // (POLYLINE/POLYGON only). Keyed by index N. aoCoords remains the bucket 0
    // storage (alias), preserving backward compatibility for all existing code
    // paths that read aoCoords.
    std::map<int, std::vector<std::pair<double, double>>> aoAdditionalCoordsSets;

    // Default constructor
    PolishMapSection() : eType(SectionType::POI), nEndLevel(-1) {}

    // Constructor with type
    explicit PolishMapSection(SectionType type) : eType(type), nEndLevel(-1) {}

    // Clear all data
    // Story 3.1: Added shrink_to_fit() for memory optimization (Architecture: Memory Management)
    // Pattern: Parse section → Populate IR → Convert to OGRFeature → Clear IR → Next section
    void Clear() {
        osType.clear();
        osLabel.clear();
        aoCoords.clear();
        aoCoords.shrink_to_fit();  // Release memory (NFR3: < 2x file size)
        nEndLevel = -1;
        osLevels.clear();
        aoOtherFields.clear();
        aoAdditionalCoordsSets.clear();
    }

    // Get minimum point count for valid geometry
    int GetMinPointCount() const {
        switch (eType) {
            case SectionType::POI: return 1;
            case SectionType::Polyline: return 2;
            case SectionType::Polygon: return 3;
        }
        return 1;
    }

    // Get section marker string
    const char* GetSectionMarker() const {
        switch (eType) {
            case SectionType::POI: return "[POI]";
            case SectionType::Polyline: return "[POLYLINE]";
            case SectionType::Polygon: return "[POLYGON]";
        }
        return "[UNKNOWN]";
    }

    // Get section type name for logging
    const char* GetTypeName() const {
        switch (eType) {
            case SectionType::POI: return "POI";
            case SectionType::Polyline: return "POLYLINE";
            case SectionType::Polygon: return "POLYGON";
        }
        return "UNKNOWN";
    }
};

/************************************************************************/
/*                        PolishMapHeaderData                           */
/*                                                                      */
/* Intermediate Representation (IR) structure for [IMG ID] header data. */
/* Architecture: Minimal IR, cleared after each section processed.      */
/************************************************************************/

struct PolishMapHeaderData {
    // Basic fields (existing)
    std::string osName;           // Map name
    std::string osID;             // Map ID (REQUIRED field per cGPSmapper spec)
    std::string osCodePage;       // Encoding (default: 1252)
    std::string osDatum;          // Coordinate system (default: WGS 84)
    std::string osElevation;      // Elevation unit (M/F)

    // Critical fields (Story 1.2 Extension)
    std::string osLBLcoding;      // Label encoding: 6/9/10 (default: 9 = 8-bit)
    std::string osPreprocess;     // Preprocessing mode: G/F/P/N (default: F)
    std::string osLevels;         // Number of zoom levels: 1-10 (e.g., "2")
    std::vector<std::string> aoLevelDefs;  // Level definitions: Level0, Level1, ... (e.g., ["24", "18"])
    std::string osTreeSize;       // Map tree size: 100-15000 (default: 3000)
    std::string osRgnLimit;       // Region element limit: 50-1024 (default: 1024)

    // Important fields (Story 1.2 Extension)
    std::string osTransparent;    // Transparency: Y/N/S (default: N)
    std::string osSimplifyLevel;  // Simplification level: 0-4 (default: 2)
    std::string osMarine;         // Marine map: Y/N (default: N)
    std::string osLeftSideTraffic;// Left-side traffic: Y/N (default: N)

    std::map<std::string, std::string> aoOtherFields;  // All other unrecognized key=value pairs

    // Default values
    PolishMapHeaderData() : osCodePage("1252"), osDatum("WGS 84") {}

    // Clear all data
    // Story 3.1: Added shrink_to_fit() for memory optimization (NFR3)
    // Story 1.2: Extended to clear new fields
    void Clear() {
        osName.clear();
        osName.shrink_to_fit();
        osID.clear();
        osID.shrink_to_fit();
        osCodePage = "1252";
        osDatum = "WGS 84";
        osElevation.clear();
        osElevation.shrink_to_fit();

        // Clear critical fields
        osLBLcoding.clear();
        osPreprocess.clear();
        osLevels.clear();
        aoLevelDefs.clear();
        aoLevelDefs.shrink_to_fit();
        osTreeSize.clear();
        osRgnLimit.clear();

        // Clear important fields
        osTransparent.clear();
        osSimplifyLevel.clear();
        osMarine.clear();
        osLeftSideTraffic.clear();

        aoOtherFields.clear();
    }
};

/************************************************************************/
/*                        PolishMapPOISection                           */
/************************************************************************/

/**
 * @struct PolishMapPOISection
 * @brief Intermediate Representation (IR) for [POI] section data.
 *
 * Minimalist IR structure for parsing a single POI section at a time.
 * Used for backward compatibility with legacy parsing code.
 *
 * @see PolishMapSection for the unified IR structure.
 * @see PolishMapParser::ParseNextPOI()
 */
struct PolishMapPOISection {
    std::string osType;                    // "0x2C00"
    std::string osLabel;                   // UTF-8 après conversion
    std::pair<double, double> oCoords;     // (lat, lon)
    int nEndLevel;                         // 0-9, -1 si absent
    std::string osLevels;                  // "0-3" ou vide
    std::map<std::string, std::string> aoOtherFields;  // Data1, Data2, etc.

    // Default values
    PolishMapPOISection() : oCoords(0.0, 0.0), nEndLevel(-1) {}

    // Clear all data
    void Clear() {
        osType.clear();
        osLabel.clear();
        oCoords = std::make_pair(0.0, 0.0);
        nEndLevel = -1;
        osLevels.clear();
        aoOtherFields.clear();
    }
};

/************************************************************************/
/*                      PolishMapPolylineSection                        */
/************************************************************************/

/**
 * @struct PolishMapPolylineSection
 * @brief Intermediate Representation (IR) for [POLYLINE] section data.
 *
 * Minimalist IR structure for parsing a single POLYLINE section at a time.
 * Stores coordinate list for linear features (roads, trails, boundaries).
 *
 * @see PolishMapSection for the unified IR structure.
 * @see PolishMapParser::ParseNextPolyline()
 */
struct PolishMapPolylineSection {
    std::string osType;                              // "0x16"
    std::string osLabel;                             // UTF-8 après conversion
    std::vector<std::pair<double, double>> aoCoords; // [(lat1, lon1), (lat2, lon2), ...]
    int nEndLevel;                                   // 0-9, -1 si absent
    std::string osLevels;                            // "0-3" ou vide
    std::map<std::string, std::string> aoOtherFields;// Champs additionnels

    // Tech-spec #2 Task 4: Data1=..Data9= coordinate sets (N>=1). aoCoords
    // remains the N=0 bucket; aoAdditionalCoordsSets is only populated when
    // the parser encounters DataN>0 lines.
    std::map<int, std::vector<std::pair<double, double>>> aoAdditionalCoordsSets;

    // Default values
    PolishMapPolylineSection() : nEndLevel(-1) {}

    // Clear all data
    // Story 3.1: Added shrink_to_fit() for memory optimization (NFR3)
    void Clear() {
        osType.clear();
        osLabel.clear();
        aoCoords.clear();
        aoCoords.shrink_to_fit();  // Release memory
        nEndLevel = -1;
        osLevels.clear();
        aoOtherFields.clear();
        aoAdditionalCoordsSets.clear();
    }
};

/************************************************************************/
/*                      PolishMapPolygonSection                         */
/************************************************************************/

/**
 * @struct PolishMapPolygonSection
 * @brief Intermediate Representation (IR) for [POLYGON] section data.
 *
 * Minimalist IR structure for parsing a single POLYGON section at a time.
 * Stores coordinate list for area features (forests, lakes, urban areas).
 *
 * @see PolishMapSection for the unified IR structure.
 * @see PolishMapParser::ParseNextPolygon()
 */
struct PolishMapPolygonSection {
    std::string osType;                              // "0x4C"
    std::string osLabel;                             // UTF-8 après conversion
    std::vector<std::pair<double, double>> aoCoords; // [(lat1, lon1), (lat2, lon2), ...]
    int nEndLevel;                                   // 0-9, -1 si absent
    std::string osLevels;                            // "0-3" ou vide
    std::map<std::string, std::string> aoOtherFields;// Champs additionnels

    // Tech-spec #2 Task 4: Data1=..Data9= coordinate sets (N>=1). See
    // PolishMapPolylineSection for semantics.
    std::map<int, std::vector<std::pair<double, double>>> aoAdditionalCoordsSets;

    // Default values
    PolishMapPolygonSection() : nEndLevel(-1) {}

    // Clear all data
    // Story 3.1: Added shrink_to_fit() for memory optimization (NFR3)
    void Clear() {
        osType.clear();
        osLabel.clear();
        aoCoords.clear();
        aoCoords.shrink_to_fit();  // Release memory
        nEndLevel = -1;
        osLevels.clear();
        aoOtherFields.clear();
        aoAdditionalCoordsSets.clear();
    }
};

/************************************************************************/
/*                         PolishMapParser                              */
/*                                                                      */
/* Hybrid parser for Polish Map format files:                           */
/* - Level 1: Section detection via [SECTION_NAME] markers              */
/* - Level 2: Key=value parsing inside sections                         */
/* - State machine for section transitions                              */
/*                                                                      */
/* Story 3.1: Performance optimization notes (NFR1, NFR3)                */
/* - CPLReadLineL() uses internal buffering (GDAL optimized)             */
/* - No additional buffer needed - measured 0.455s for 10 MB (< 2s NFR1) */
/* - Memory optimized via shrink_to_fit() on IR structures               */
/*                                                                      */
/* Story 3.2: Three-Level Error Strategy (NFR9, NFR10, NFR11)            */
/*                                                                      */
/* @section error_strategy Error Handling Strategy                       */
/*                                                                      */
/* The parser implements a three-level error strategy for robustness:    */
/*                                                                      */
/* 1. CRITICAL ERRORS (Fail + Return NULL):                              */
/*    - Missing [IMG ID] header section                                  */
/*    - Empty or binary/corrupted file                                   */
/*    - Action: CPLError(CE_Failure, CPLE_OpenFailed) + return false     */
/*                                                                      */
/* 2. RECOVERABLE ERRORS (Skip + Continue):                              */
/*    - Malformed section (missing required fields like Type/Data0)      */
/*    - Invalid geometry (coordinates outside WGS84 range)               */
/*    - Invalid Type code format (non-hex/non-numeric)                   */
/*    - Action: CPLError(CE_Warning, CPLE_AppDefined) + skip section     */
/*                                                                      */
/* 3. MINOR ISSUES (Default + Log):                                      */
/*    - Missing optional fields (Label, EndLevel)                        */
/*    - Missing [END] marker at EOF                                      */
/*    - Action: CPLDebug("OGR_POLISHMAP") + use default value            */
/*                                                                      */
/* All error messages include context: filename, line number, section.   */
/* This strategy ensures NFR9 (0 crashes) and NFR11 (graceful degrade).  */
/************************************************************************/

/**
 * @class PolishMapParser
 * @brief Parser for Polish Map format files.
 *
 * Implements a hybrid parser for Polish Map (.mp) files with:
 * - Level 1: Section detection via bracketed markers ([POI], [POLYLINE], etc.)
 * - Level 2: Key=value parsing inside sections
 * - State machine for section transitions and error recovery
 *
 * @section performance Performance Characteristics
 * - Uses GDAL's CPLReadLineL() for efficient buffered reading
 * - Memory-optimized IR structures with shrink_to_fit() cleanup
 * - Measured: ~0.455s for 10 MB file (NFR1 target: <2s)
 *
 * @section errors Error Handling Strategy
 * The parser implements a three-level error strategy:
 * 1. **CRITICAL**: Missing header, corrupted file → Fail with CE_Failure
 * 2. **RECOVERABLE**: Malformed section, invalid coords → Skip with CE_Warning
 * 3. **MINOR**: Missing optional fields → Use defaults with CPLDebug()
 *
 * @see PolishMapSection
 * @see PolishMapHeaderData
 * @see OGRPolishMapDataSource
 */
class PolishMapParser {
public:
    /**
     * @brief Construct a parser for the specified file.
     *
     * Opens the file using GDAL's VSI file abstraction layer.
     * Check IsOpen() after construction to verify success.
     *
     * @param pszFilePath Path to the Polish Map file to parse.
     */
    explicit PolishMapParser(const char* pszFilePath);

    /**
     * @brief Destructor.
     *
     * Closes the file handle if open.
     */
    ~PolishMapParser();

    /** @brief Copy constructor (deleted). */
    PolishMapParser(const PolishMapParser&) = delete;

    /** @brief Copy assignment operator (deleted). */
    PolishMapParser& operator=(const PolishMapParser&) = delete;

    /**
     * @brief Parse the [IMG ID] header section.
     *
     * Reads and parses the file header, extracting metadata fields like
     * Name, ID, CodePage, Datum, etc. Must be called before parsing
     * any feature sections.
     *
     * @return true on success, false on failure.
     *
     * @note On failure, CPLError() is called with error details.
     * @note Stores the file position after header for ResetSectionReading().
     */
    bool ParseHeader();

    /**
     * @brief Get the parsed header data.
     *
     * @return Const reference to the header data structure.
     *
     * @note Only valid after successful ParseHeader() call.
     */
    const PolishMapHeaderData& GetHeaderData() const { return m_oHeaderData; }

    /**
     * @brief Check if the file was successfully opened.
     *
     * @return true if the file is open and ready, false otherwise.
     */
    bool IsOpen() const { return m_fpFile != nullptr; }

    /**
     * @brief Parse the next section of the specified type.
     *
     * Scans from the current position for the next section matching
     * the target type and parses its contents into the output structure.
     *
     * @param eTargetType Type of section to find (POI, Polyline, or Polygon).
     * @param oSection Output structure to receive parsed data.
     * @return true if a section was found and parsed, false if no more sections.
     *
     * @note Skips sections that don't match the target type.
     * @note Call oSection.Clear() before reusing the structure.
     */
    bool ParseNextSection(SectionType eTargetType, PolishMapSection& oSection);

    /**
     * @brief Reset reading position to start of data sections.
     *
     * Resets the file position to just after the header, allowing
     * feature sections to be re-read from the beginning.
     */
    void ResetSectionReading();

    /**
     * @brief Parse the next [POI] section.
     *
     * Wrapper for backward compatibility. Equivalent to calling
     * ParseNextSection(SectionType::POI, ...) with conversion.
     *
     * @param oSection Output structure to receive parsed POI data.
     * @return true if a POI was found and parsed, false if no more POIs.
     */
    bool ParseNextPOI(PolishMapPOISection& oSection);

    /**
     * @brief Reset reading position for POI sections.
     *
     * Alias for ResetSectionReading() - all layers share the same position.
     */
    void ResetPOIReading() { ResetSectionReading(); }

    /**
     * @brief Parse the next [POLYLINE] section.
     *
     * Wrapper for backward compatibility.
     *
     * @param oSection Output structure to receive parsed POLYLINE data.
     * @return true if a POLYLINE was found and parsed, false if no more.
     */
    bool ParseNextPolyline(PolishMapPolylineSection& oSection);

    /**
     * @brief Reset reading position for POLYLINE sections.
     *
     * Alias for ResetSectionReading() - all layers share the same position.
     */
    void ResetPolylineReading() { ResetSectionReading(); }

    /**
     * @brief Parse the next [POLYGON] section.
     *
     * Wrapper for backward compatibility.
     *
     * @param oSection Output structure to receive parsed POLYGON data.
     * @return true if a POLYGON was found and parsed, false if no more.
     */
    bool ParseNextPolygon(PolishMapPolygonSection& oSection);

    /**
     * @brief Reset reading position for POLYGON sections.
     *
     * Alias for ResetSectionReading() - all layers share the same position.
     */
    void ResetPolygonReading() { ResetSectionReading(); }

    /**
     * @brief Get the current line number in the file.
     *
     * Useful for error messages and debugging.
     *
     * @return Current line number (1-based).
     */
    int GetCurrentLine() const { return m_nCurrentLine; }

private:
    CPLString m_osFilePath;           /**< Path to the input file */
    VSILFILE* m_fpFile;               /**< VSI file handle */
    PolishMapHeaderData m_oHeaderData;/**< Parsed header metadata */
    vsi_l_offset m_nAfterHeaderPos;   /**< File position after header */
    int m_nCurrentLine;               /**< Current line number (1-based) */

    /**
     * @brief Read the next line from the file.
     *
     * @param osLine Output string to receive the line content.
     * @return true if a line was read, false on EOF or error.
     */
    bool ReadLine(CPLString& osLine);

    /**
     * @brief Parse a key=value pair from a line.
     *
     * @param osLine Input line to parse.
     * @param osKey Output key name.
     * @param osValue Output value string.
     * @return true if successfully parsed, false if not a key=value line.
     */
    bool ParseKeyValue(const CPLString& osLine, CPLString& osKey, CPLString& osValue);

    /**
     * @brief Convert a string from CP1252 to UTF-8.
     *
     * Uses the CodePage from header metadata to determine source encoding.
     *
     * @param osValue Input string in source encoding.
     * @return UTF-8 encoded string.
     */
    CPLString RecodeToUTF8(const CPLString& osValue);

    /**
     * @brief Parse single coordinate pair from Data0 value.
     *
     * Parses "(lat,lon)" format into separate double values.
     *
     * @param osValue Input coordinate string.
     * @param dfLat Output latitude value.
     * @param dfLon Output longitude value.
     * @return true if successfully parsed, false on error.
     */
    bool ParseCoordinates(const CPLString& osValue, double& dfLat, double& dfLon);

    /**
     * @brief Parse coordinate list from Data0 value.
     *
     * Parses multi-point format "(lat1,lon1),(lat2,lon2),..." or
     * simple "lat1,lon1,lat2,lon2,..." into a vector of coordinate pairs.
     *
     * @param osValue Input coordinate list string.
     * @param aoCoords Output vector of (lat, lon) pairs.
     * @return Number of points parsed (0 on error or empty input).
     */
    int ParseCoordinateList(const CPLString& osValue,
                            std::vector<std::pair<double, double>>& aoCoords);

    /**
     * @brief Parse Level0-N definitions from header.
     *
     * Story 1.2 Extension: Parses multi-value Level0, Level1, ..., LevelN fields
     * based on the Levels count. Extracts Level definitions from aoOtherFields
     * and stores them in aoLevelDefs vector for structured access.
     *
     * Must be called after ParseHeader() has populated aoOtherFields.
     * Removes Level0-N entries from aoOtherFields after extraction.
     */
    void ParseLevelDefinitions();
};

#endif /* POLISHMAPPARSER_H_INCLUDED */
