/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Simple YAML parser for field mapping configuration (Story 4.4)
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

#ifndef POLISHMAPYAMLPARSER_H_INCLUDED
#define POLISHMAPYAMLPARSER_H_INCLUDED

#include "cpl_error.h"
#include "cpl_vsi.h"
#include <map>
#include <string>

/************************************************************************/
/*                      PolishMapYAMLParser                             */
/************************************************************************/

/**
 * @class PolishMapYAMLParser
 * @brief Minimal YAML parser for field mapping configuration.
 *
 * Parses a simple YAML format for field mapping:
 * ```yaml
 * field_mapping:
 *   SOURCE_FIELD: TARGET_FIELD
 *   NAME: Label
 *   MP_TYPE: Type
 * ```
 *
 * Features:
 * - Supports simple key-value pairs under `field_mapping:` section
 * - Ignores comments (lines starting with #)
 * - Case-insensitive source field matching
 * - Validates target fields against Polish Map schema
 * - No external YAML library dependency
 *
 * Limitations:
 * - No nested structures beyond `field_mapping:` section
 * - No arrays, multiline strings, or advanced YAML features
 * - Simple line-by-line parsing only
 *
 * @section example Usage Example
 * @code
 * PolishMapYAMLParser parser;
 * if (parser.LoadConfig("/path/to/config.yaml")) {
 *     auto mappings = parser.GetMappings();
 *     // Use mappings...
 * }
 * @endcode
 */
class PolishMapYAMLParser {
private:
    std::map<std::string, std::string> m_oMappings;  ///< Source → Target mappings
    bool m_bLoaded;                                   ///< Config successfully loaded

public:
    /**
     * @brief Constructor.
     */
    PolishMapYAMLParser();

    /**
     * @brief Destructor.
     */
    ~PolishMapYAMLParser();

    /**
     * @brief Load YAML config from file.
     *
     * Parses the YAML file and extracts field mappings from the
     * `field_mapping:` section. Validates target fields against
     * Polish Map schema (polishmapfields.h).
     *
     * @param pszConfigPath Path to YAML config file.
     * @return true if loaded successfully, false on error.
     *
     * @note Errors are reported via CPLError().
     * @note Source field names are converted to uppercase for case-insensitive matching.
     */
    bool LoadConfig(const char* pszConfigPath);

    /**
     * @brief Get all field mappings.
     *
     * Returns the map of source fields → Polish Map fields.
     * Source field names are uppercase.
     *
     * @return Map of source → target field names.
     */
    const std::map<std::string, std::string>& GetMappings() const;

    /**
     * @brief Check if config was loaded successfully.
     *
     * @return true if LoadConfig() succeeded, false otherwise.
     */
    bool IsLoaded() const;

private:
    /**
     * @brief Parse a single mapping line.
     *
     * Parses a line in format: `  SOURCE: TARGET`
     * Extracts source and target field names, validates target.
     *
     * @param pszLine Line to parse.
     * @param osSource Output: source field name (uppercase).
     * @param osTarget Output: target field name (canonical).
     * @return true if parsed successfully, false on syntax error.
     */
    bool ParseMappingLine(const char* pszLine, std::string& osSource, std::string& osTarget);

    /**
     * @brief Validate target field name.
     *
     * Checks if target field exists in Polish Map schema (g_aoPolishMapFields).
     *
     * @param pszTarget Target field name to validate.
     * @return true if valid Polish Map field, false otherwise.
     */
    bool ValidateTargetField(const char* pszTarget);

    /**
     * @brief Trim whitespace from string.
     *
     * @param osStr String to trim (modified in place).
     */
    void TrimString(std::string& osStr);
};

#endif /* POLISHMAPYAMLPARSER_H_INCLUDED */
