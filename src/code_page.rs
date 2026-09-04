//! Windows code page identifiers.
//!
//! Code page numbers are not part of the Encoding Standard; they come from
//! Microsoft's [Code Page Identifiers] table, and are what `GetACP`, a
//! `.NET` `Encoding.CodePage`, an ODBC driver or an old database column will
//! hand you.  The mapping below pairs each number with the encoding this crate
//! uses for it.
//!
//! A number resolves to the charset Microsoft assigns it, not to whatever the
//! WHATWG Encoding Standard would make of the equivalent label: 28591 is
//! [`ISO_8859_1`](crate::ISO_8859_1) and 20127 is
//! [`US_ASCII`](crate::US_ASCII), rather than windows-1252 as the labels
//! `iso-8859-1` and `ascii` would give through
//! [`Encoding::for_whatwg_label`](crate::Encoding::for_whatwg_label).
//!
//! A number for a charset this crate does not have is absent, rather than
//! pointed at a superset or at [`REPLACEMENT`]: 50227 is ISO-2022-CN and 52936
//! is HZ-GB-2312 in Microsoft's registry, whatever the standard makes of the
//! matching labels, and neither is something this crate can decode.
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
// Every alias below sits behind a table feature, so a build with none is left
// with no caller for this.
#[allow(dead_code)]
const fn alias(number: u32, encoding: &'static Encoding) -> CodePage {
    CodePage {
        number,
        encoding,
        canonical: false,
    }
}

/// Sorted by number, for binary search.
///
/// A slice rather than an array because its length depends on which table
/// groups are compiled in.
pub(crate) static CODE_PAGES: &[CodePage] = &[
    #[cfg(feature = "single-byte")]
    alias(708, &e::ISO_8859_6_INIT), // Arabic (ASMO 708)
    #[cfg(feature = "single-byte")]
    cp(866, &e::IBM866_INIT),
    #[cfg(feature = "single-byte")]
    cp(874, &e::WINDOWS_874_INIT),
    #[cfg(feature = "shift-jis")]
    // 932 is the codepage the standard's Shift_JIS reproduces byte for byte.
    cp(932, &e::WINDOWS_31J_INIT),
    #[cfg(feature = "gb18030")]
    cp(936, &e::GBK_INIT), // named gb2312, but it is GBK
    #[cfg(feature = "euc-kr")]
    cp(949, &e::EUC_KR_INIT), // Unified Hangul Code
    // 950 is Big5, and so lives with the Big5 table itself rather than here:
    // the standard's index of that name is Big5 plus HKSCS, which disagrees
    // with Microsoft's 950 at 250 cells where Big5 itself disagrees at 11.
    cp(1200, &e::UTF_16LE_INIT),
    cp(1201, &e::UTF_16BE_INIT),
    #[cfg(feature = "single-byte")]
    cp(1250, &e::WINDOWS_1250_INIT),
    #[cfg(feature = "single-byte")]
    cp(1251, &e::WINDOWS_1251_INIT),
    #[cfg(feature = "single-byte")]
    cp(1252, &e::WINDOWS_1252_INIT),
    #[cfg(feature = "single-byte")]
    cp(1253, &e::WINDOWS_1253_INIT),
    #[cfg(feature = "single-byte")]
    cp(1254, &e::WINDOWS_1254_INIT),
    #[cfg(feature = "single-byte")]
    cp(1255, &e::WINDOWS_1255_INIT),
    #[cfg(feature = "single-byte")]
    cp(1256, &e::WINDOWS_1256_INIT),
    #[cfg(feature = "single-byte")]
    cp(1257, &e::WINDOWS_1257_INIT),
    #[cfg(feature = "single-byte")]
    cp(1258, &e::WINDOWS_1258_INIT),
    #[cfg(feature = "single-byte")]
    cp(10000, &e::MACINTOSH_INIT),
    #[cfg(feature = "single-byte")]
    cp(10007, &e::X_MAC_CYRILLIC_INIT),
    #[cfg(feature = "single-byte")]
    alias(10017, &e::X_MAC_CYRILLIC_INIT), // Mac Ukrainian
    #[cfg(feature = "single-byte")]
    #[cfg(feature = "single-byte")]
    cp(20866, &e::KOI8_R_INIT),
    #[cfg(feature = "euc-jp")]
    alias(20932, &e::EUC_JP_INIT), // JIS X 0208-1990 and 0212-1990
    #[cfg(feature = "gb18030")]
    alias(20936, &e::GBK_INIT), // GB2312-80
    #[cfg(feature = "euc-kr")]
    alias(20949, &e::EUC_KR_INIT), // Korean Wansung
    #[cfg(feature = "single-byte")]
    cp(21866, &e::KOI8_U_INIT),
    #[cfg(feature = "single-byte")]
    #[cfg(feature = "single-byte")]
    cp(28592, &e::ISO_8859_2_INIT),
    #[cfg(feature = "single-byte")]
    cp(28593, &e::ISO_8859_3_INIT),
    #[cfg(feature = "single-byte")]
    cp(28594, &e::ISO_8859_4_INIT),
    #[cfg(feature = "single-byte")]
    cp(28595, &e::ISO_8859_5_INIT),
    #[cfg(feature = "single-byte")]
    cp(28596, &e::ISO_8859_6_INIT),
    #[cfg(feature = "single-byte")]
    cp(28597, &e::ISO_8859_7_INIT),
    #[cfg(feature = "single-byte")]
    cp(28598, &e::ISO_8859_8_INIT),
    #[cfg(feature = "single-byte")]
    #[cfg(feature = "single-byte")]
    cp(28603, &e::ISO_8859_13_INIT),
    #[cfg(feature = "single-byte")]
    cp(28605, &e::ISO_8859_15_INIT),
    #[cfg(feature = "single-byte")]
    cp(38598, &e::ISO_8859_8_I_INIT),
    #[cfg(feature = "iso-2022-jp")]
    cp(50220, &e::ISO_2022_JP_INIT),
    #[cfg(feature = "iso-2022-jp")]
    alias(50221, &e::ISO_2022_JP_INIT), // allows half-width katakana
    #[cfg(feature = "iso-2022-jp")]
    alias(50222, &e::ISO_2022_JP_INIT), // allows SO/SI
    #[cfg(feature = "euc-jp")]
    cp(51932, &e::EUC_JP_INIT),
    #[cfg(feature = "gb18030")]
    alias(51936, &e::GBK_INIT), // EUC-CN
    #[cfg(feature = "euc-kr")]
    alias(51949, &e::EUC_KR_INIT),
    #[cfg(feature = "gb18030")]
    cp(54936, &e::GB18030_INIT),
    cp(65001, &e::UTF_8_INIT),
];
