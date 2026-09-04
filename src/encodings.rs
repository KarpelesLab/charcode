//! The [`Encoding`] instances for every encoding in the standard.
//!
//! Each encoding exists once as a `const` (so that it can be named in other
//! constants, such as the label table) and once as a `&'static` reference, which is
//! the form the API hands out and compares by name.

use crate::Encoding;
use crate::tables::single_byte as sb;
use crate::variant::VariantEncoding;

// The Encoding

/// UTF-8, the encoding every new format should use.
pub const UTF_8_INIT: Encoding = Encoding::new("UTF-8", VariantEncoding::Utf8);
/// UTF-8, the encoding every new format should use.
pub static UTF_8: &Encoding = &UTF_8_INIT;

// Legacy single-byte encodings

/// IBM866.
pub const IBM866_INIT: Encoding = Encoding::new(
    "IBM866",
    VariantEncoding::SingleByte(
        &sb::IBM866_DECODE,
        &sb::IBM866_ENCODE_CODE_POINTS,
        &sb::IBM866_ENCODE_BYTES,
    ),
);
/// IBM866.
pub static IBM866: &Encoding = &IBM866_INIT;

/// ISO-8859-2.
pub const ISO_8859_2_INIT: Encoding = Encoding::new(
    "ISO-8859-2",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_2_DECODE,
        &sb::ISO_8859_2_ENCODE_CODE_POINTS,
        &sb::ISO_8859_2_ENCODE_BYTES,
    ),
);
/// ISO-8859-2.
pub static ISO_8859_2: &Encoding = &ISO_8859_2_INIT;

/// ISO-8859-3.
pub const ISO_8859_3_INIT: Encoding = Encoding::new(
    "ISO-8859-3",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_3_DECODE,
        &sb::ISO_8859_3_ENCODE_CODE_POINTS,
        &sb::ISO_8859_3_ENCODE_BYTES,
    ),
);
/// ISO-8859-3.
pub static ISO_8859_3: &Encoding = &ISO_8859_3_INIT;

/// ISO-8859-4.
pub const ISO_8859_4_INIT: Encoding = Encoding::new(
    "ISO-8859-4",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_4_DECODE,
        &sb::ISO_8859_4_ENCODE_CODE_POINTS,
        &sb::ISO_8859_4_ENCODE_BYTES,
    ),
);
/// ISO-8859-4.
pub static ISO_8859_4: &Encoding = &ISO_8859_4_INIT;

/// ISO-8859-5.
pub const ISO_8859_5_INIT: Encoding = Encoding::new(
    "ISO-8859-5",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_5_DECODE,
        &sb::ISO_8859_5_ENCODE_CODE_POINTS,
        &sb::ISO_8859_5_ENCODE_BYTES,
    ),
);
/// ISO-8859-5.
pub static ISO_8859_5: &Encoding = &ISO_8859_5_INIT;

/// ISO-8859-6.
pub const ISO_8859_6_INIT: Encoding = Encoding::new(
    "ISO-8859-6",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_6_DECODE,
        &sb::ISO_8859_6_ENCODE_CODE_POINTS,
        &sb::ISO_8859_6_ENCODE_BYTES,
    ),
);
/// ISO-8859-6.
pub static ISO_8859_6: &Encoding = &ISO_8859_6_INIT;

/// ISO-8859-7.
pub const ISO_8859_7_INIT: Encoding = Encoding::new(
    "ISO-8859-7",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_7_DECODE,
        &sb::ISO_8859_7_ENCODE_CODE_POINTS,
        &sb::ISO_8859_7_ENCODE_BYTES,
    ),
);
/// ISO-8859-7.
pub static ISO_8859_7: &Encoding = &ISO_8859_7_INIT;

/// ISO-8859-8.
pub const ISO_8859_8_INIT: Encoding = Encoding::new(
    "ISO-8859-8",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_8_DECODE,
        &sb::ISO_8859_8_ENCODE_CODE_POINTS,
        &sb::ISO_8859_8_ENCODE_BYTES,
    ),
);
/// ISO-8859-8.
pub static ISO_8859_8: &Encoding = &ISO_8859_8_INIT;

/// ISO-8859-8-I, identical to [`ISO_8859_8`] except in its name and implied text direction.
pub const ISO_8859_8_I_INIT: Encoding = Encoding::new(
    "ISO-8859-8-I",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_8_DECODE,
        &sb::ISO_8859_8_ENCODE_CODE_POINTS,
        &sb::ISO_8859_8_ENCODE_BYTES,
    ),
);
/// ISO-8859-8-I, identical to [`ISO_8859_8`] except in its name and implied text direction.
pub static ISO_8859_8_I: &Encoding = &ISO_8859_8_I_INIT;

/// ISO-8859-10.
pub const ISO_8859_10_INIT: Encoding = Encoding::new(
    "ISO-8859-10",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_10_DECODE,
        &sb::ISO_8859_10_ENCODE_CODE_POINTS,
        &sb::ISO_8859_10_ENCODE_BYTES,
    ),
);
/// ISO-8859-10.
pub static ISO_8859_10: &Encoding = &ISO_8859_10_INIT;

/// ISO-8859-13.
pub const ISO_8859_13_INIT: Encoding = Encoding::new(
    "ISO-8859-13",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_13_DECODE,
        &sb::ISO_8859_13_ENCODE_CODE_POINTS,
        &sb::ISO_8859_13_ENCODE_BYTES,
    ),
);
/// ISO-8859-13.
pub static ISO_8859_13: &Encoding = &ISO_8859_13_INIT;

/// ISO-8859-14.
pub const ISO_8859_14_INIT: Encoding = Encoding::new(
    "ISO-8859-14",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_14_DECODE,
        &sb::ISO_8859_14_ENCODE_CODE_POINTS,
        &sb::ISO_8859_14_ENCODE_BYTES,
    ),
);
/// ISO-8859-14.
pub static ISO_8859_14: &Encoding = &ISO_8859_14_INIT;

/// ISO-8859-15.
pub const ISO_8859_15_INIT: Encoding = Encoding::new(
    "ISO-8859-15",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_15_DECODE,
        &sb::ISO_8859_15_ENCODE_CODE_POINTS,
        &sb::ISO_8859_15_ENCODE_BYTES,
    ),
);
/// ISO-8859-15.
pub static ISO_8859_15: &Encoding = &ISO_8859_15_INIT;

/// ISO-8859-16.
pub const ISO_8859_16_INIT: Encoding = Encoding::new(
    "ISO-8859-16",
    VariantEncoding::SingleByte(
        &sb::ISO_8859_16_DECODE,
        &sb::ISO_8859_16_ENCODE_CODE_POINTS,
        &sb::ISO_8859_16_ENCODE_BYTES,
    ),
);
/// ISO-8859-16.
pub static ISO_8859_16: &Encoding = &ISO_8859_16_INIT;

/// KOI8-R.
pub const KOI8_R_INIT: Encoding = Encoding::new(
    "KOI8-R",
    VariantEncoding::SingleByte(
        &sb::KOI8_R_DECODE,
        &sb::KOI8_R_ENCODE_CODE_POINTS,
        &sb::KOI8_R_ENCODE_BYTES,
    ),
);
/// KOI8-R.
pub static KOI8_R: &Encoding = &KOI8_R_INIT;

/// KOI8-U.
pub const KOI8_U_INIT: Encoding = Encoding::new(
    "KOI8-U",
    VariantEncoding::SingleByte(
        &sb::KOI8_U_DECODE,
        &sb::KOI8_U_ENCODE_CODE_POINTS,
        &sb::KOI8_U_ENCODE_BYTES,
    ),
);
/// KOI8-U.
pub static KOI8_U: &Encoding = &KOI8_U_INIT;

/// macintosh.
pub const MACINTOSH_INIT: Encoding = Encoding::new(
    "macintosh",
    VariantEncoding::SingleByte(
        &sb::MACINTOSH_DECODE,
        &sb::MACINTOSH_ENCODE_CODE_POINTS,
        &sb::MACINTOSH_ENCODE_BYTES,
    ),
);
/// macintosh.
pub static MACINTOSH: &Encoding = &MACINTOSH_INIT;

/// windows-874.
pub const WINDOWS_874_INIT: Encoding = Encoding::new(
    "windows-874",
    VariantEncoding::SingleByte(
        &sb::WINDOWS_874_DECODE,
        &sb::WINDOWS_874_ENCODE_CODE_POINTS,
        &sb::WINDOWS_874_ENCODE_BYTES,
    ),
);
/// windows-874.
pub static WINDOWS_874: &Encoding = &WINDOWS_874_INIT;

/// windows-1250.
pub const WINDOWS_1250_INIT: Encoding = Encoding::new(
    "windows-1250",
    VariantEncoding::SingleByte(
        &sb::WINDOWS_1250_DECODE,
        &sb::WINDOWS_1250_ENCODE_CODE_POINTS,
        &sb::WINDOWS_1250_ENCODE_BYTES,
    ),
);
/// windows-1250.
pub static WINDOWS_1250: &Encoding = &WINDOWS_1250_INIT;

/// windows-1251.
pub const WINDOWS_1251_INIT: Encoding = Encoding::new(
    "windows-1251",
    VariantEncoding::SingleByte(
        &sb::WINDOWS_1251_DECODE,
        &sb::WINDOWS_1251_ENCODE_CODE_POINTS,
        &sb::WINDOWS_1251_ENCODE_BYTES,
    ),
);
/// windows-1251.
pub static WINDOWS_1251: &Encoding = &WINDOWS_1251_INIT;

/// windows-1252.
pub const WINDOWS_1252_INIT: Encoding = Encoding::new(
    "windows-1252",
    VariantEncoding::SingleByte(
        &sb::WINDOWS_1252_DECODE,
        &sb::WINDOWS_1252_ENCODE_CODE_POINTS,
        &sb::WINDOWS_1252_ENCODE_BYTES,
    ),
);
/// windows-1252.
pub static WINDOWS_1252: &Encoding = &WINDOWS_1252_INIT;

/// windows-1253.
pub const WINDOWS_1253_INIT: Encoding = Encoding::new(
    "windows-1253",
    VariantEncoding::SingleByte(
        &sb::WINDOWS_1253_DECODE,
        &sb::WINDOWS_1253_ENCODE_CODE_POINTS,
        &sb::WINDOWS_1253_ENCODE_BYTES,
    ),
);
/// windows-1253.
pub static WINDOWS_1253: &Encoding = &WINDOWS_1253_INIT;

/// windows-1254.
pub const WINDOWS_1254_INIT: Encoding = Encoding::new(
    "windows-1254",
    VariantEncoding::SingleByte(
        &sb::WINDOWS_1254_DECODE,
        &sb::WINDOWS_1254_ENCODE_CODE_POINTS,
        &sb::WINDOWS_1254_ENCODE_BYTES,
    ),
);
/// windows-1254.
pub static WINDOWS_1254: &Encoding = &WINDOWS_1254_INIT;

/// windows-1255.
pub const WINDOWS_1255_INIT: Encoding = Encoding::new(
    "windows-1255",
    VariantEncoding::SingleByte(
        &sb::WINDOWS_1255_DECODE,
        &sb::WINDOWS_1255_ENCODE_CODE_POINTS,
        &sb::WINDOWS_1255_ENCODE_BYTES,
    ),
);
/// windows-1255.
pub static WINDOWS_1255: &Encoding = &WINDOWS_1255_INIT;

/// windows-1256.
pub const WINDOWS_1256_INIT: Encoding = Encoding::new(
    "windows-1256",
    VariantEncoding::SingleByte(
        &sb::WINDOWS_1256_DECODE,
        &sb::WINDOWS_1256_ENCODE_CODE_POINTS,
        &sb::WINDOWS_1256_ENCODE_BYTES,
    ),
);
/// windows-1256.
pub static WINDOWS_1256: &Encoding = &WINDOWS_1256_INIT;

/// windows-1257.
pub const WINDOWS_1257_INIT: Encoding = Encoding::new(
    "windows-1257",
    VariantEncoding::SingleByte(
        &sb::WINDOWS_1257_DECODE,
        &sb::WINDOWS_1257_ENCODE_CODE_POINTS,
        &sb::WINDOWS_1257_ENCODE_BYTES,
    ),
);
/// windows-1257.
pub static WINDOWS_1257: &Encoding = &WINDOWS_1257_INIT;

/// windows-1258.
pub const WINDOWS_1258_INIT: Encoding = Encoding::new(
    "windows-1258",
    VariantEncoding::SingleByte(
        &sb::WINDOWS_1258_DECODE,
        &sb::WINDOWS_1258_ENCODE_CODE_POINTS,
        &sb::WINDOWS_1258_ENCODE_BYTES,
    ),
);
/// windows-1258.
pub static WINDOWS_1258: &Encoding = &WINDOWS_1258_INIT;

/// x-mac-cyrillic.
pub const X_MAC_CYRILLIC_INIT: Encoding = Encoding::new(
    "x-mac-cyrillic",
    VariantEncoding::SingleByte(
        &sb::X_MAC_CYRILLIC_DECODE,
        &sb::X_MAC_CYRILLIC_ENCODE_CODE_POINTS,
        &sb::X_MAC_CYRILLIC_ENCODE_BYTES,
    ),
);
/// x-mac-cyrillic.
pub static X_MAC_CYRILLIC: &Encoding = &X_MAC_CYRILLIC_INIT;

// Legacy multi-byte Chinese (simplified) encodings

/// GBK, which decodes as [`GB18030`] but whose encoder emits only two-byte sequences.
pub const GBK_INIT: Encoding = Encoding::new("GBK", VariantEncoding::Gb18030 { is_gbk: true });
/// GBK, which decodes as [`GB18030`] but whose encoder emits only two-byte sequences.
pub static GBK: &Encoding = &GBK_INIT;

/// gb18030, the Chinese national standard, which covers all of Unicode.
pub const GB18030_INIT: Encoding =
    Encoding::new("gb18030", VariantEncoding::Gb18030 { is_gbk: false });
/// gb18030, the Chinese national standard, which covers all of Unicode.
pub static GB18030: &Encoding = &GB18030_INIT;

// Legacy multi-byte Chinese (traditional) encodings

/// Big5, with the Hong Kong Supplementary Character Set extensions.
pub const BIG5_INIT: Encoding = Encoding::new("Big5", VariantEncoding::Big5);
/// Big5, with the Hong Kong Supplementary Character Set extensions.
pub static BIG5: &Encoding = &BIG5_INIT;

// Legacy multi-byte Japanese encodings

/// EUC-JP.  Its decoder also accepts JIS X 0212 via the 0x8F prefix.
pub const EUC_JP_INIT: Encoding = Encoding::new("EUC-JP", VariantEncoding::EucJp);
/// EUC-JP.  Its decoder also accepts JIS X 0212 via the 0x8F prefix.
pub static EUC_JP: &Encoding = &EUC_JP_INIT;

/// ISO-2022-JP, the only stateful encoding in the standard.
pub const ISO_2022_JP_INIT: Encoding = Encoding::new("ISO-2022-JP", VariantEncoding::Iso2022Jp);
/// ISO-2022-JP, the only stateful encoding in the standard.
pub static ISO_2022_JP: &Encoding = &ISO_2022_JP_INIT;

/// Shift_JIS, including the Windows end-user defined character range.
pub const SHIFT_JIS_INIT: Encoding = Encoding::new("Shift_JIS", VariantEncoding::ShiftJis);
/// Shift_JIS, including the Windows end-user defined character range.
pub static SHIFT_JIS: &Encoding = &SHIFT_JIS_INIT;

// Legacy multi-byte Korean encodings

/// EUC-KR, in practice the Unified Hangul Code (Windows codepage 949).
pub const EUC_KR_INIT: Encoding = Encoding::new("EUC-KR", VariantEncoding::EucKr);
/// EUC-KR, in practice the Unified Hangul Code (Windows codepage 949).
pub static EUC_KR: &Encoding = &EUC_KR_INIT;

// Legacy miscellaneous encodings

/// The `replacement` encoding, which decodes any non-empty input to a single U+FFFD.
pub const REPLACEMENT_INIT: Encoding = Encoding::new("replacement", VariantEncoding::Replacement);
/// The `replacement` encoding, which decodes any non-empty input to a single U+FFFD.
pub static REPLACEMENT: &Encoding = &REPLACEMENT_INIT;

/// UTF-16BE.  Decode only; encoding to it yields UTF-8, per `get an output encoding`.
pub const UTF_16BE_INIT: Encoding = Encoding::new("UTF-16BE", VariantEncoding::Utf16Be);
/// UTF-16BE.  Decode only; encoding to it yields UTF-8, per `get an output encoding`.
pub static UTF_16BE: &Encoding = &UTF_16BE_INIT;

/// UTF-16LE.  Decode only; encoding to it yields UTF-8, per `get an output encoding`.
pub const UTF_16LE_INIT: Encoding = Encoding::new("UTF-16LE", VariantEncoding::Utf16Le);
/// UTF-16LE.  Decode only; encoding to it yields UTF-8, per `get an output encoding`.
pub static UTF_16LE: &Encoding = &UTF_16LE_INIT;

/// x-user-defined, which maps bytes 0x80 to 0xFF into the private use area.
pub const X_USER_DEFINED_INIT: Encoding =
    Encoding::new("x-user-defined", VariantEncoding::XUserDefined);
/// x-user-defined, which maps bytes 0x80 to 0xFF into the private use area.
pub static X_USER_DEFINED: &Encoding = &X_USER_DEFINED_INIT;
