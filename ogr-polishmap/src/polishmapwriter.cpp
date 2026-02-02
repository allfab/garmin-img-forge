/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Polish Map format writer - implementation
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

#include "polishmapwriter.h"
#include "cpl_error.h"
#include "cpl_conv.h"

/************************************************************************/
/*                          PolishMapWriter()                            */
/************************************************************************/

PolishMapWriter::PolishMapWriter(VSILFILE* fpOutput)
    : m_fpOutput(fpOutput)
    , m_bHeaderWritten(false)
{
    // File handle is borrowed - we don't own it
}

/************************************************************************/
/*                         ~PolishMapWriter()                            */
/************************************************************************/

PolishMapWriter::~PolishMapWriter()
{
    // Do NOT close file - it's a borrowed handle
    // Owner (OGRPolishMapDataSource) is responsible for closing
}

/************************************************************************/
/*                           WriteHeader()                               */
/*                                                                      */
/* Story 2.1 Task 3.2: Write minimal [IMG ID] header section.           */
/* Output:                                                              */
/*   [IMG ID]                                                           */
/*   Name=<name>                                                        */
/*   CodePage=<codepage>                                                */
/*   [END]                                                              */
/************************************************************************/

bool PolishMapWriter::WriteHeader(const std::string& osName, const std::string& osCodePage)
{
    if (m_fpOutput == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "PolishMapWriter::WriteHeader() - file handle is null");
        return false;
    }

    if (m_bHeaderWritten) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "PolishMapWriter::WriteHeader() - header already written");
        return true;  // Not a fatal error
    }

    // Write [IMG ID] section
    if (VSIFPrintfL(m_fpOutput, "[IMG ID]\n") < 0) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeader() - failed to write [IMG ID]");
        return false;
    }

    // Write Name field
    if (VSIFPrintfL(m_fpOutput, "Name=%s\n", osName.c_str()) < 0) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeader() - failed to write Name");
        return false;
    }

    // Write CodePage field
    if (VSIFPrintfL(m_fpOutput, "CodePage=%s\n", osCodePage.c_str()) < 0) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeader() - failed to write CodePage");
        return false;
    }

    // Write [END] marker
    if (VSIFPrintfL(m_fpOutput, "[END]\n") < 0) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "PolishMapWriter::WriteHeader() - failed to write [END]");
        return false;
    }

    m_bHeaderWritten = true;

    CPLDebug("OGR_POLISHMAP", "WriteHeader: Name=%s, CodePage=%s",
             osName.c_str(), osCodePage.c_str());

    return true;
}

/************************************************************************/
/*                              Flush()                                  */
/************************************************************************/

bool PolishMapWriter::Flush()
{
    if (m_fpOutput == nullptr) {
        return false;
    }

    return VSIFFlushL(m_fpOutput) == 0;
}
