/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Simple YAML parser implementation for field mapping
 * Author:   mpforge project
 *
 ******************************************************************************/

#include "polishmapyamlparser.h"
#include "polishmapfields.h"
#include "cpl_string.h"
#include <algorithm>
#include <cctype>
#include <memory>

/************************************************************************/
/*                        PolishMapYAMLParser()                         */
/************************************************************************/

PolishMapYAMLParser::PolishMapYAMLParser()
    : m_bLoaded(false)
{
}

/************************************************************************/
/*                       ~PolishMapYAMLParser()                         */
/************************************************************************/

PolishMapYAMLParser::~PolishMapYAMLParser()
{
}

/************************************************************************/
/*                            LoadConfig()                              */
/************************************************************************/

bool PolishMapYAMLParser::LoadConfig(const char* pszConfigPath)
{
    if (!pszConfigPath || pszConfigPath[0] == '\0') {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "Empty config path provided");
        return false;
    }

    // Open file with RAII wrapper to ensure closure
    struct VSILFileCloser {
        void operator()(VSILFILE* fp) const {
            if (fp) VSIFCloseL(fp);
        }
    };
    std::unique_ptr<VSILFILE, VSILFileCloser> fp(VSIFOpenL(pszConfigPath, "r"));

    if (!fp) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "Failed to open field mapping config: %s", pszConfigPath);
        return false;
    }

    bool bInFieldMappingSection = false;
    int nLineNum = 0;
    const char* pszLine = nullptr;

    m_oMappings.clear();

    // Read file line by line
    while ((pszLine = CPLReadLineL(fp.get())) != nullptr) {
        nLineNum++;

        std::string osLine(pszLine);
        TrimString(osLine);

        // Skip empty lines and comments
        if (osLine.empty() || osLine[0] == '#') {
            continue;
        }

        // Check for field_mapping section
        if (osLine == "field_mapping:") {
            bInFieldMappingSection = true;
            continue;
        }

        // Parse mappings if inside field_mapping section
        if (bInFieldMappingSection) {
            // Check if line starts with whitespace (indicates mapping entry)
            if (!osLine.empty() && (pszLine[0] == ' ' || pszLine[0] == '\t')) {
                std::string osSource, osTarget;
                if (ParseMappingLine(osLine.c_str(), osSource, osTarget)) {
                    // Convert source to uppercase for case-insensitive matching
                    for (auto& c : osSource) {
                        c = static_cast<char>(toupper(static_cast<unsigned char>(c)));
                    }

                    // Validate target field
                    if (!ValidateTargetField(osTarget.c_str())) {
                        CPLError(CE_Warning, CPLE_AppDefined,
                                 "Line %d: Invalid Polish Map field '%s', ignoring mapping",
                                 nLineNum, osTarget.c_str());
                        continue;
                    }

                    // Store mapping
                    m_oMappings[osSource] = osTarget;
                    CPLDebug("PolishMap", "Field mapping: %s → %s",
                             osSource.c_str(), osTarget.c_str());
                } else {
                    CPLError(CE_Warning, CPLE_AppDefined,
                             "Line %d: Syntax error in mapping, expected 'SOURCE: TARGET'",
                             nLineNum);
                }
            } else {
                // Non-indented line exits field_mapping section
                bInFieldMappingSection = false;
            }
        }
    }

    // File automatically closed by unique_ptr destructor

    if (m_oMappings.empty()) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "No field mappings found in config: %s", pszConfigPath);
    }

    m_bLoaded = true;
    CPLDebug("PolishMap", "Loaded %d field mappings from %s",
             static_cast<int>(m_oMappings.size()), pszConfigPath);

    return true;
}

/************************************************************************/
/*                          GetMappings()                               */
/************************************************************************/

const std::map<std::string, std::string>& PolishMapYAMLParser::GetMappings() const
{
    return m_oMappings;
}

/************************************************************************/
/*                           IsLoaded()                                 */
/************************************************************************/

bool PolishMapYAMLParser::IsLoaded() const
{
    return m_bLoaded;
}

/************************************************************************/
/*                        ParseMappingLine()                            */
/************************************************************************/

bool PolishMapYAMLParser::ParseMappingLine(const char* pszLine,
                                           std::string& osSource,
                                           std::string& osTarget)
{
    if (!pszLine || pszLine[0] == '\0') {
        return false;
    }

    std::string osLine(pszLine);
    TrimString(osLine);

    // Find colon separator
    size_t nColonPos = osLine.find(':');
    if (nColonPos == std::string::npos) {
        return false;
    }

    // Extract source and target
    osSource = osLine.substr(0, nColonPos);
    osTarget = osLine.substr(nColonPos + 1);

    // Remove inline comments from target (everything after #)
    size_t nCommentPos = osTarget.find('#');
    if (nCommentPos != std::string::npos) {
        osTarget = osTarget.substr(0, nCommentPos);
    }

    TrimString(osSource);
    TrimString(osTarget);

    // Check for empty parts
    if (osSource.empty() || osTarget.empty()) {
        return false;
    }

    return true;
}

/************************************************************************/
/*                       ValidateTargetField()                          */
/************************************************************************/

bool PolishMapYAMLParser::ValidateTargetField(const char* pszTarget)
{
    if (!pszTarget || pszTarget[0] == '\0') {
        return false;
    }

    // Check if target exists in Polish Map fields
    for (int i = 0; i < g_nPolishMapFieldCount; i++) {
        if (EQUAL(pszTarget, g_aoPolishMapFields[i].pszName)) {
            return true;
        }
    }

    return false;
}

/************************************************************************/
/*                          TrimString()                                */
/************************************************************************/

void PolishMapYAMLParser::TrimString(std::string& osStr)
{
    // Trim leading whitespace
    osStr.erase(osStr.begin(),
                std::find_if(osStr.begin(), osStr.end(), [](unsigned char ch) {
                    return !std::isspace(ch);
                }));

    // Trim trailing whitespace
    osStr.erase(std::find_if(osStr.rbegin(), osStr.rend(), [](unsigned char ch) {
                    return !std::isspace(ch);
                }).base(),
                osStr.end());
}
