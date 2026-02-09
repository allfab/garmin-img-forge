/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Field mapping manager for configurable field names (Story 4.4)
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

#ifndef POLISHMAPFIELDMAPPER_H_INCLUDED
#define POLISHMAPFIELDMAPPER_H_INCLUDED

#include "polishmapyamlparser.h"
#include <map>
#include <string>

/************************************************************************/
/*                      PolishMapFieldMapper                            */
/************************************************************************/

/**
 * @class PolishMapFieldMapper
 * @brief Manages field name mapping with YAML config and hardcoded fallback.
 *
 * Provides configurable field mapping from source dataset fields to
 * Polish Map canonical fields. Supports two mapping strategies:
 *
 * 1. **YAML Config** (priority): User-provided mapping file
 * 2. **Hardcoded Aliases** (fallback): Built-in aliases from polishmapfields.h
 *
 * This enables flexible ogr2ogr conversions from any source format
 * (BDTOPO, OSM, custom Shapefiles) without code changes.
 *
 * @section workflow Usage Workflow
 * @code
 * // 1. Create mapper
 * PolishMapFieldMapper mapper;
 *
 * // 2. Optionally load YAML config (if user provides -co FIELD_MAPPING=...)
 * if (pszConfigPath) {
 *     mapper.LoadConfig(pszConfigPath);
 * }
 *
 * // 3. Map field names during conversion
 * std::string osTarget = mapper.MapFieldName("NAME");  // → "Label"
 * @endcode
 *
 * @see PolishMapYAMLParser
 * @see ResolveFieldAlias() in polishmapfields.h
 */
class PolishMapFieldMapper {
private:
    PolishMapYAMLParser m_oParser;  ///< YAML config parser
    bool m_bHasConfig;               ///< true if YAML config loaded

public:
    /**
     * @brief Constructor.
     */
    PolishMapFieldMapper();

    /**
     * @brief Destructor.
     */
    ~PolishMapFieldMapper();

    /**
     * @brief Load YAML config from file.
     *
     * Loads field mappings from YAML config file using PolishMapYAMLParser.
     * If successful, YAML mappings take priority over hardcoded aliases.
     *
     * @param pszConfigPath Path to YAML config file.
     * @return true if loaded successfully, false on error.
     *
     * @note Errors are reported via CPLError().
     * @note If this is not called, MapFieldName() uses hardcoded aliases only.
     *
     * @see PolishMapYAMLParser::LoadConfig()
     */
    bool LoadConfig(const char* pszConfigPath);

    /**
     * @brief Map source field name to Polish Map canonical name.
     *
     * Resolves field mapping using this priority:
     * 1. YAML config (if loaded via LoadConfig())
     * 2. Hardcoded aliases (from ResolveFieldAlias())
     * 3. Empty string (if no mapping found)
     *
     * @param pszSourceField Source field name to map.
     * @return Canonical Polish Map field name, or empty string if unmapped.
     *
     * @note Matching is case-insensitive.
     * @note Returns empty string if field is not recognized.
     *
     * @section examples Examples
     * @code
     * // With YAML config loaded:
     * mapper.MapFieldName("NAME");        // → "Label" (from YAML)
     * mapper.MapFieldName("Country");     // → "CountryName" (from YAML)
     *
     * // Without config (hardcoded aliases):
     * mapper.MapFieldName("NAME");        // → "Label" (hardcoded)
     * mapper.MapFieldName("UNKNOWN");     // → "" (not found)
     * @endcode
     */
    std::string MapFieldName(const char* pszSourceField) const;

    /**
     * @brief Check if YAML config was loaded.
     *
     * @return true if LoadConfig() succeeded, false if using hardcoded aliases only.
     */
    bool HasConfig() const;

    /**
     * @brief Get all field mappings from YAML config.
     *
     * Returns the map of source → target field names from YAML config.
     * If no config loaded, returns empty map.
     *
     * @return Map of source field names (uppercase) → Polish Map field names.
     *
     * @note This only returns YAML mappings, not hardcoded aliases.
     */
    const std::map<std::string, std::string>& GetMappings() const;
};

#endif /* POLISHMAPFIELDMAPPER_H_INCLUDED */
