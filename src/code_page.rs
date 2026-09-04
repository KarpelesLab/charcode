//! Windows code page identifiers.
//!
//! Code page numbers are not part of the Encoding Standard; they come from
//! Microsoft's [Code Page Identifiers] table, and are what `GetACP`, a
//! `.NET` `Encoding.CodePage`, an ODBC driver or an old database column will
//! hand you.  The mapping below pairs each number with the encoding this crate
//! uses for it.
//!
//! Where a number names an encoding the standard folds into a superset, the
//! superset is what you get, exactly as it is for the equivalent label: code
//! page 28591 (ISO-8859-1) resolves to [`WINDOWS_1252`], and 28599
//! (ISO-8859-9) to [`WINDOWS_1254`].  Numbers for encodings the standard
//! neutralizes — 50225, 50227, 50229 and 52936 — resolve to [`REPLACEMENT`].
//!
//! [Code Page Identifiers]: https://learn.microsoft.com/en-us/windows/win32/intl/code-page-identifiers

use crate::Encoding;
use crate::encodings as e;

pub(crate) struct CodePage {
    pub(crate) number: u32,
    pub(crate) encoding: &'static Encoding,
    /// Whether this is the number [`Encoding::windows_code_page`] reports.  An
    /// encoding reachable through several numbers has exactly one.
    pub(crate) canonical: bool,
}

const fn cp(number: u32, encoding: &'static Encoding) -> CodePage {
    CodePage {
        number,
        encoding,
        canonical: true,
    }
}

/// An additional number for an encoding that already has a canonical one.
const fn alias(number: u32, encoding: &'static Encoding) -> CodePage {
    CodePage {
        number,
        encoding,
        canonical: false,
    }
}

/// Sorted by number, for binary search.
pub(crate) static CODE_PAGES: [CodePage; 51] = [
    alias(708, &e::ISO_8859_6_INIT), // Arabic (ASMO 708)
    cp(866, &e::IBM866_INIT),
    cp(874, &e::WINDOWS_874_INIT),
    cp(932, &e::SHIFT_JIS_INIT),
    cp(936, &e::GBK_INIT),    // named gb2312, but it is GBK
    cp(949, &e::EUC_KR_INIT), // Unified Hangul Code
    cp(950, &e::BIG5_INIT),
    cp(1200, &e::UTF_16LE_INIT),
    cp(1201, &e::UTF_16BE_INIT),
    cp(1250, &e::WINDOWS_1250_INIT),
    cp(1251, &e::WINDOWS_1251_INIT),
    cp(1252, &e::WINDOWS_1252_INIT),
    cp(1253, &e::WINDOWS_1253_INIT),
    cp(1254, &e::WINDOWS_1254_INIT),
    cp(1255, &e::WINDOWS_1255_INIT),
    cp(1256, &e::WINDOWS_1256_INIT),
    cp(1257, &e::WINDOWS_1257_INIT),
    cp(1258, &e::WINDOWS_1258_INIT),
    cp(10000, &e::MACINTOSH_INIT),
    cp(10007, &e::X_MAC_CYRILLIC_INIT),
    alias(10017, &e::X_MAC_CYRILLIC_INIT), // Mac Ukrainian
    alias(20127, &e::WINDOWS_1252_INIT),   // US-ASCII
    cp(20866, &e::KOI8_R_INIT),
    alias(20932, &e::EUC_JP_INIT), // JIS X 0208-1990 and 0212-1990
    alias(20936, &e::GBK_INIT),    // GB2312-80
    alias(20949, &e::EUC_KR_INIT), // Korean Wansung
    cp(21866, &e::KOI8_U_INIT),
    alias(28591, &e::WINDOWS_1252_INIT), // ISO-8859-1
    cp(28592, &e::ISO_8859_2_INIT),
    cp(28593, &e::ISO_8859_3_INIT),
    cp(28594, &e::ISO_8859_4_INIT),
    cp(28595, &e::ISO_8859_5_INIT),
    cp(28596, &e::ISO_8859_6_INIT),
    cp(28597, &e::ISO_8859_7_INIT),
    cp(28598, &e::ISO_8859_8_INIT),
    alias(28599, &e::WINDOWS_1254_INIT), // ISO-8859-9
    cp(28603, &e::ISO_8859_13_INIT),
    cp(28605, &e::ISO_8859_15_INIT),
    cp(38598, &e::ISO_8859_8_I_INIT),
    cp(50220, &e::ISO_2022_JP_INIT),
    alias(50221, &e::ISO_2022_JP_INIT), // allows half-width katakana
    alias(50222, &e::ISO_2022_JP_INIT), // allows SO/SI
    cp(50225, &e::REPLACEMENT_INIT),    // ISO-2022-KR
    alias(50227, &e::REPLACEMENT_INIT), // ISO-2022-CN
    alias(50229, &e::REPLACEMENT_INIT), // ISO-2022-CN-EXT
    cp(51932, &e::EUC_JP_INIT),
    alias(51936, &e::GBK_INIT), // EUC-CN
    alias(51949, &e::EUC_KR_INIT),
    alias(52936, &e::REPLACEMENT_INIT), // HZ-GB-2312
    cp(54936, &e::GB18030_INIT),
    cp(65001, &e::UTF_8_INIT),
];
