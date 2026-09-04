//! Every code point an encoder can produce is encoded and decoded again.
//!
//! Most of the standard's indexes round-trip exactly.  The exceptions are all
//! deliberate, and each one is listed here with the reason the standard gives.

use alloc::string::String;

use crate::Encoding;
use crate::encodings::*;

/// Encodes one character, requiring that the encoding can represent it.
fn encode_char(encoding: &'static Encoding, c: char) -> alloc::vec::Vec<u8> {
    let mut buffer = alloc::vec::Vec::new();
    let mut string = String::new();
    string.push(c);
    encoding
        .new_encoder()
        .encode_from_str_without_replacement(&string, &mut buffer, true)
        .unwrap_or_else(|e| panic!("{} cannot encode {e}", encoding.name()));
    buffer
}

fn round_trip(encoding: &'static Encoding, code_points: impl Iterator<Item = u32>) {
    for scalar in code_points {
        let Some(c) = char::from_u32(scalar) else {
            continue;
        };
        let bytes = encode_char(encoding, c);
        let (decoded, had_errors) = encoding.decode_without_bom_handling(&bytes);
        assert!(
            !had_errors,
            "{}: U+{scalar:04X} encoded to {bytes:02X?}, which does not decode",
            encoding.name()
        );
        assert_eq!(
            decoded.chars().next(),
            Some(c),
            "{}: U+{scalar:04X} encoded to {bytes:02X?}, which decodes to {decoded:?}",
            encoding.name()
        );
        assert_eq!(decoded.chars().count(), 1, "{}", encoding.name());
    }
}

/// ASCII is passed through unchanged by every ASCII-compatible encoding.
#[test]
fn ascii_round_trips() {
    for &encoding in Encoding::all() {
        if !encoding.is_ascii_compatible() {
            continue;
        }
        let output = encoding.output_encoding();
        round_trip(output, 0..0x80);
    }
}

#[cfg(feature = "single-byte")]
#[test]
fn single_byte_round_trips() {
    use crate::tables::single_byte as sb;
    let cases: [(&'static Encoding, &[u16]); 28] = [
        (IBM866, &sb::IBM866_ENCODE_CODE_POINTS),
        (ISO_8859_2, &sb::ISO_8859_2_ENCODE_CODE_POINTS),
        (ISO_8859_3, &sb::ISO_8859_3_ENCODE_CODE_POINTS),
        (ISO_8859_4, &sb::ISO_8859_4_ENCODE_CODE_POINTS),
        (ISO_8859_5, &sb::ISO_8859_5_ENCODE_CODE_POINTS),
        (ISO_8859_6, &sb::ISO_8859_6_ENCODE_CODE_POINTS),
        (ISO_8859_7, &sb::ISO_8859_7_ENCODE_CODE_POINTS),
        (ISO_8859_8, &sb::ISO_8859_8_ENCODE_CODE_POINTS),
        (ISO_8859_8_I, &sb::ISO_8859_8_ENCODE_CODE_POINTS),
        (ISO_8859_10, &sb::ISO_8859_10_ENCODE_CODE_POINTS),
        (ISO_8859_13, &sb::ISO_8859_13_ENCODE_CODE_POINTS),
        (ISO_8859_14, &sb::ISO_8859_14_ENCODE_CODE_POINTS),
        (ISO_8859_15, &sb::ISO_8859_15_ENCODE_CODE_POINTS),
        (ISO_8859_16, &sb::ISO_8859_16_ENCODE_CODE_POINTS),
        (KOI8_R, &sb::KOI8_R_ENCODE_CODE_POINTS),
        (KOI8_U, &sb::KOI8_U_ENCODE_CODE_POINTS),
        (MACINTOSH, &sb::MACINTOSH_ENCODE_CODE_POINTS),
        (WINDOWS_874, &sb::WINDOWS_874_ENCODE_CODE_POINTS),
        (WINDOWS_1250, &sb::WINDOWS_1250_ENCODE_CODE_POINTS),
        (WINDOWS_1251, &sb::WINDOWS_1251_ENCODE_CODE_POINTS),
        (WINDOWS_1252, &sb::WINDOWS_1252_ENCODE_CODE_POINTS),
        (WINDOWS_1253, &sb::WINDOWS_1253_ENCODE_CODE_POINTS),
        (WINDOWS_1254, &sb::WINDOWS_1254_ENCODE_CODE_POINTS),
        (WINDOWS_1255, &sb::WINDOWS_1255_ENCODE_CODE_POINTS),
        (WINDOWS_1256, &sb::WINDOWS_1256_ENCODE_CODE_POINTS),
        (WINDOWS_1257, &sb::WINDOWS_1257_ENCODE_CODE_POINTS),
        (WINDOWS_1258, &sb::WINDOWS_1258_ENCODE_CODE_POINTS),
        (X_MAC_CYRILLIC, &sb::X_MAC_CYRILLIC_ENCODE_CODE_POINTS),
    ];
    for (encoding, code_points) in cases {
        round_trip(encoding, code_points.iter().map(|&cp| u32::from(cp)));
    }
}

#[test]
fn x_user_defined_round_trips() {
    round_trip(X_USER_DEFINED, 0xF780..=0xF7FF);
}

#[cfg(feature = "big5")]
#[test]
fn big5_round_trips() {
    use crate::tables::big5::BIG5_ENCODE_CODE_POINTS;
    round_trip(BIG5, BIG5_ENCODE_CODE_POINTS.iter().copied());
}

#[cfg(feature = "euc-kr")]
#[test]
fn euc_kr_round_trips() {
    use crate::tables::euc_kr::EUC_KR_ENCODE_CODE_POINTS;
    round_trip(
        EUC_KR,
        EUC_KR_ENCODE_CODE_POINTS.iter().map(|&cp| u32::from(cp)),
    );
}

#[cfg(feature = "gb18030")]
#[test]
fn gb18030_round_trips() {
    use crate::tables::gb18030::GB18030_ENCODE_CODE_POINTS;
    round_trip(
        GB18030,
        GB18030_ENCODE_CODE_POINTS
            .iter()
            .map(|&cp| u32::from(cp))
            .filter(|cp| !GB18030_ENCODER_EXCEPTIONS.contains(cp)),
    );
}

/// gb18030's encoder maps these to two-byte sequences that the decoder reads as
/// something else, to keep encoding them the way GB18030-2005 did.  The standard
/// calls the table out as asymmetric.
#[cfg(feature = "gb18030")]
const GB18030_ENCODER_EXCEPTIONS: &[u32] = &[];

#[cfg(all(feature = "euc-jp", feature = "iso-2022-jp", feature = "shift-jis"))]
#[test]
fn jis_round_trips() {
    use crate::tables::jis::{JIS0208_ENCODE_CODE_POINTS, SHIFT_JIS_ENCODE_CODE_POINTS};
    round_trip(
        EUC_JP,
        JIS0208_ENCODE_CODE_POINTS
            .iter()
            .map(|&cp| u32::from(cp))
            .filter(|&cp| !JIS_ENCODER_EXCEPTIONS.contains(&cp)),
    );
    round_trip(
        ISO_2022_JP,
        JIS0208_ENCODE_CODE_POINTS
            .iter()
            .map(|&cp| u32::from(cp))
            .filter(|&cp| !JIS_ENCODER_EXCEPTIONS.contains(&cp)),
    );
    round_trip(
        SHIFT_JIS,
        SHIFT_JIS_ENCODE_CODE_POINTS
            .iter()
            .map(|&cp| u32::from(cp))
            .filter(|&cp| !JIS_ENCODER_EXCEPTIONS.contains(&cp)),
    );
}

/// The Japanese encoders fold U+2212 MINUS SIGN into U+FF0D FULLWIDTH HYPHEN-MINUS
/// before looking it up, so it cannot come back.
#[cfg(all(feature = "euc-jp", feature = "iso-2022-jp", feature = "shift-jis"))]
const JIS_ENCODER_EXCEPTIONS: &[u32] = &[0x2212];

/// Half-width katakana survives EUC-JP and Shift_JIS, but ISO-2022-JP has no
/// half-width forms and maps them to their full-width equivalents.
#[cfg(all(feature = "euc-jp", feature = "iso-2022-jp", feature = "shift-jis"))]
#[test]
fn half_width_katakana() {
    for scalar in 0xFF61..=0xFF9Fu32 {
        let c = char::from_u32(scalar).unwrap();
        round_trip(EUC_JP, core::iter::once(scalar));
        round_trip(SHIFT_JIS, core::iter::once(scalar));
        let bytes = encode_char(ISO_2022_JP, c);
        let (decoded, _) = ISO_2022_JP.decode_without_bom_handling(&bytes);
        assert_ne!(decoded, alloc::string::String::from(c));
    }
}
