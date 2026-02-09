/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Field mapping manager implementation
 * Author:   mpforge project
 *
 ******************************************************************************/

#include "polishmapfieldmapper.h"
#include "polishmapfields.h"
#include "cpl_string.h"

/************************************************************************/
/*                       PolishMapFieldMapper()                         */
/************************************************************************/

PolishMapFieldMapper::PolishMapFieldMapper()
    : m_bHasConfig(false)
{
}

/************************************************************************/
/*                      ~PolishMapFieldMapper()                         */
/************************************************************************/

PolishMapFieldMapper::~PolishMapFieldMapper()
{
}

/************************************************************************/
/*                          LoadConfig()                                */
/************************************************************************/

bool PolishMapFieldMapper::LoadConfig(const char* pszConfigPath)
{
    if (!pszConfigPath || pszConfigPath[0] == '\0') {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "Empty config path provided to field mapper");
        return false;
    }

    if (m_oParser.LoadConfig(pszConfigPath)) {
        m_bHasConfig = true;
        CPLDebug("PolishMap", "Field mapper loaded config: %s", pszConfigPath);
        return true;
    }

    return false;
}

/************************************************************************/
/*                         MapFieldName()                               */
/************************************************************************/

std::string PolishMapFieldMapper::MapFieldName(const char* pszSourceField) const
{
    if (!pszSourceField || pszSourceField[0] == '\0') {
        return std::string();
    }

    // Priority 1: YAML config (if loaded)
    if (m_bHasConfig) {
        // Convert source field to uppercase for lookup
        std::string osUpper(pszSourceField);
        for (auto& c : osUpper) {
            c = static_cast<char>(toupper(static_cast<unsigned char>(c)));
        }

        const auto& mappings = m_oParser.GetMappings();
        auto it = mappings.find(osUpper);
        if (it != mappings.end()) {
            return it->second;
        }
    }

    // Priority 2: Hardcoded aliases (fallback)
    std::string osCanonical = ResolveFieldAlias(pszSourceField);
    if (!osCanonical.empty()) {
        return osCanonical;
    }

    // Priority 3: Not found
    return std::string();
}

/************************************************************************/
/*                           HasConfig()                                */
/************************************************************************/

bool PolishMapFieldMapper::HasConfig() const
{
    return m_bHasConfig;
}

/************************************************************************/
/*                         GetMappings()                                */
/************************************************************************/

const std::map<std::string, std::string>& PolishMapFieldMapper::GetMappings() const
{
    return m_oParser.GetMappings();
}
