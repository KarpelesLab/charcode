//! Tests that need access to the index tables.
//!
//! Each test reconstructs the byte sequence for a pointer from the algorithm in
//! the standard, independently of the decoder, and checks that decoding it yields
//! the code point the index holds.  That catches a mistranscribed formula, which
//! is the failure mode the generated tables cannot protect against.

mod round_trip;
mod streaming;

use alloc::string::String;

use crate::encodings::*;
use crate::tables::big5::BIG5_DECODE;
use crate::tables::euc_kr::EUC_KR_DECODE;
use crate::tables::gb18030::GB18030_DECODE;
use crate::tables::jis::{JIS0208_DECODE, JIS0212_DECODE};
use crate::tables::single_byte as sb;
use crate::{Encoding, index};

/// Decodes with no BOM handling and no substitution, for tests that want to see
/// exactly what a byte sequence maps to.
fn decode_strict(encoding: &'static Encoding, bytes: &[u8]) -> Option<String> {
    encoding
        .decode_without_bom_handling_and_without_replacement(bytes)
        .map(|cow| cow.into_owned())
}

fn expect_char(encoding: &'static Encoding, bytes: &[u8], code_point: u32) {
    let expected = char::from_u32(code_point).expect("index holds scalar values");
    let decoded = decode_strict(encoding, bytes);
    assert_eq!(
        decoded.as_deref(),
        Some(&*alloc::string::String::from(expected)),
        "{} should decode {bytes:02X?} to U+{code_point:04X}",
        encoding.name()
    );
}

#[test]
fn single_byte_indexes() {
    let cases: [(&'static Encoding, &[u16; 128]); 28] = [
        (IBM866, &sb::IBM866_DECODE),
        (ISO_8859_2, &sb::ISO_8859_2_DECODE),
        (ISO_8859_3, &sb::ISO_8859_3_DECODE),
        (ISO_8859_4, &sb::ISO_8859_4_DECODE),
        (ISO_8859_5, &sb::ISO_8859_5_DECODE),
        (ISO_8859_6, &sb::ISO_8859_6_DECODE),
        (ISO_8859_7, &sb::ISO_8859_7_DECODE),
        (ISO_8859_8, &sb::ISO_8859_8_DECODE),
        (ISO_8859_8_I, &sb::ISO_8859_8_DECODE),
        (ISO_8859_10, &sb::ISO_8859_10_DECODE),
        (ISO_8859_13, &sb::ISO_8859_13_DECODE),
        (ISO_8859_14, &sb::ISO_8859_14_DECODE),
        (ISO_8859_15, &sb::ISO_8859_15_DECODE),
        (ISO_8859_16, &sb::ISO_8859_16_DECODE),
        (KOI8_R, &sb::KOI8_R_DECODE),
        (KOI8_U, &sb::KOI8_U_DECODE),
        (MACINTOSH, &sb::MACINTOSH_DECODE),
        (WINDOWS_874, &sb::WINDOWS_874_DECODE),
        (WINDOWS_1250, &sb::WINDOWS_1250_DECODE),
        (WINDOWS_1251, &sb::WINDOWS_1251_DECODE),
        (WINDOWS_1252, &sb::WINDOWS_1252_DECODE),
        (WINDOWS_1253, &sb::WINDOWS_1253_DECODE),
        (WINDOWS_1254, &sb::WINDOWS_1254_DECODE),
        (WINDOWS_1255, &sb::WINDOWS_1255_DECODE),
        (WINDOWS_1256, &sb::WINDOWS_1256_DECODE),
        (WINDOWS_1257, &sb::WINDOWS_1257_DECODE),
        (WINDOWS_1258, &sb::WINDOWS_1258_DECODE),
        (X_MAC_CYRILLIC, &sb::X_MAC_CYRILLIC_DECODE),
    ];
    for (encoding, table) in cases {
        for byte in 0..=0x7Fu8 {
            expect_char(encoding, &[byte], u32::from(byte));
        }
        for (pointer, &code_point) in table.iter().enumerate() {
            let byte = (pointer + 0x80) as u8;
            if code_point == 0 {
                assert_eq!(
                    decode_strict(encoding, &[byte]),
                    None,
                    "{} byte {byte:02X} is unmapped",
                    encoding.name()
                );
            } else {
                expect_char(encoding, &[byte], u32::from(code_point));
            }
        }
    }
}

#[test]
fn gb18030_two_byte_index() {
    for (pointer, &code_point) in GB18030_DECODE.iter().enumerate() {
        let lead = pointer / 190 + 0x81;
        let trail = pointer % 190;
        let trail = trail + if trail < 0x3F { 0x40 } else { 0x41 };
        assert!((0x81..=0xFE).contains(&lead) && trail != 0x7F);
        let bytes = [lead as u8, trail as u8];
        for encoding in [GB18030, GBK] {
            if code_point == 0 {
                assert_eq!(decode_strict(encoding, &bytes), None);
            } else {
                expect_char(encoding, &bytes, u32::from(code_point));
            }
        }
    }
}

#[test]
fn gb18030_four_byte_ranges() {
    // Every pointer the four-byte form can express, at the resolution of the
    // range table's boundaries plus a step through each range.
    for pointer in (0..=1_237_575u32).step_by(97) {
        let b1 = pointer / (10 * 126 * 10);
        let rest = pointer % (10 * 126 * 10);
        let b2 = rest / (10 * 126);
        let rest = rest % (10 * 126);
        let b3 = rest / 10;
        let b4 = rest % 10;
        let bytes = [
            (b1 + 0x81) as u8,
            (b2 + 0x30) as u8,
            (b3 + 0x81) as u8,
            (b4 + 0x30) as u8,
        ];
        match index::gb18030_ranges_code_point(pointer) {
            Some(code_point) => expect_char(GB18030, &bytes, code_point),
            None => assert_eq!(
                decode_strict(GB18030, &bytes),
                None,
                "pointer {pointer} is unmapped"
            ),
        }
        // GBK's decoder is gb18030's, so it accepts four-byte sequences too.
        assert_eq!(decode_strict(GBK, &bytes), decode_strict(GB18030, &bytes));
    }
}

#[test]
fn big5_index() {
    for (pointer, &code_point) in BIG5_DECODE.iter().enumerate() {
        let lead = pointer / 157 + 0x81;
        let trail = pointer % 157;
        let trail = trail + if trail < 0x3F { 0x40 } else { 0x62 };
        if lead > 0xFE {
            continue;
        }
        let bytes = [lead as u8, trail as u8];
        let pair = match pointer {
            1133 => Some("\u{00CA}\u{0304}"),
            1135 => Some("\u{00CA}\u{030C}"),
            1164 => Some("\u{00EA}\u{0304}"),
            1166 => Some("\u{00EA}\u{030C}"),
            _ => None,
        };
        if let Some(pair) = pair {
            assert_eq!(decode_strict(BIG5, &bytes).as_deref(), Some(pair));
        } else if code_point == 0 {
            assert_eq!(decode_strict(BIG5, &bytes), None, "pointer {pointer}");
        } else {
            expect_char(BIG5, &bytes, code_point);
        }
    }
}

#[test]
fn euc_kr_index() {
    for (pointer, &code_point) in EUC_KR_DECODE.iter().enumerate() {
        let lead = pointer / 190 + 0x81;
        let trail = pointer % 190 + 0x41;
        let bytes = [lead as u8, trail as u8];
        if code_point == 0 {
            assert_eq!(decode_strict(EUC_KR, &bytes), None, "pointer {pointer}");
        } else {
            expect_char(EUC_KR, &bytes, u32::from(code_point));
        }
    }
}

#[test]
fn euc_jp_indexes() {
    for pointer in 0..8836usize {
        let bytes = [(pointer / 94 + 0xA1) as u8, (pointer % 94 + 0xA1) as u8];
        match index::code_point(&JIS0208_DECODE, pointer) {
            Some(code_point) => expect_char(EUC_JP, &bytes, code_point),
            None => assert_eq!(decode_strict(EUC_JP, &bytes), None, "jis0208 {pointer}"),
        }
        let bytes = [0x8F, bytes[0], bytes[1]];
        match index::code_point(&JIS0212_DECODE, pointer) {
            Some(code_point) => expect_char(EUC_JP, &bytes, code_point),
            None => assert_eq!(decode_strict(EUC_JP, &bytes), None, "jis0212 {pointer}"),
        }
    }
    // Half-width katakana through the 0x8E prefix.
    for byte in 0xA1..=0xDFu8 {
        expect_char(EUC_JP, &[0x8E, byte], 0xFF61 - 0xA1 + u32::from(byte));
    }
}

#[test]
fn shift_jis_index() {
    for pointer in 0..JIS0208_DECODE.len() {
        let lead = pointer / 188;
        let lead_offset = if lead < 0x1F { 0x81 } else { 0xC1 };
        let trail = pointer % 188;
        let offset = if trail < 0x3F { 0x40 } else { 0x41 };
        let (lead, trail) = ((lead + lead_offset) as u8, (trail + offset) as u8);
        let addressable = ((0x81..=0x9F).contains(&lead) || (0xE0..=0xFC).contains(&lead))
            && ((0x40..=0x7E).contains(&trail) || (0x80..=0xFC).contains(&trail));
        if !addressable {
            continue;
        }
        let bytes = [lead, trail];
        if (8836..=10715).contains(&pointer) {
            // The Windows end-user defined characters, which no index covers.
            expect_char(SHIFT_JIS, &bytes, 0xE000 - 8836 + pointer as u32);
        } else {
            match index::code_point(&JIS0208_DECODE, pointer) {
                Some(code_point) => expect_char(SHIFT_JIS, &bytes, code_point),
                None => assert_eq!(decode_strict(SHIFT_JIS, &bytes), None, "pointer {pointer}"),
            }
        }
    }
    for byte in 0xA1..=0xDFu8 {
        expect_char(SHIFT_JIS, &[byte], 0xFF61 - 0xA1 + u32::from(byte));
    }
    expect_char(SHIFT_JIS, &[0x80], 0x80);
}

#[test]
fn iso_2022_jp_index() {
    for pointer in 0..8836usize {
        let bytes = [
            0x1B,
            0x24,
            0x42,
            (pointer / 94 + 0x21) as u8,
            (pointer % 94 + 0x21) as u8,
        ];
        match index::code_point(&JIS0208_DECODE, pointer) {
            Some(code_point) => expect_char(ISO_2022_JP, &bytes, code_point),
            None => assert_eq!(
                decode_strict(ISO_2022_JP, &bytes),
                None,
                "pointer {pointer}"
            ),
        }
    }
    // The half-width katakana escape.
    for byte in 0x21..=0x5Fu8 {
        expect_char(
            ISO_2022_JP,
            &[0x1B, 0x28, 0x49, byte],
            0xFF61 - 0x21 + u32::from(byte),
        );
    }
    // The Roman escape remaps two ASCII bytes.
    expect_char(ISO_2022_JP, &[0x1B, 0x28, 0x4A, 0x5C], 0x00A5);
    expect_char(ISO_2022_JP, &[0x1B, 0x28, 0x4A, 0x7E], 0x203E);
}

#[test]
fn x_user_defined_index() {
    for byte in 0..=0xFFu8 {
        let expected = if byte.is_ascii() {
            u32::from(byte)
        } else {
            0xF780 + u32::from(byte) - 0x80
        };
        expect_char(X_USER_DEFINED, &[byte], expected);
    }
}

#[test]
fn utf_16_decodes_both_orders() {
    expect_char(UTF_16LE, &[0x41, 0x00], 0x41);
    expect_char(UTF_16BE, &[0x00, 0x41], 0x41);
    // A surrogate pair for U+1F600.
    expect_char(UTF_16LE, &[0x3D, 0xD8, 0x00, 0xDE], 0x1F600);
    expect_char(UTF_16BE, &[0xD8, 0x3D, 0xDE, 0x00], 0x1F600);
    // Unpaired surrogates are errors in both orders.
    assert_eq!(decode_strict(UTF_16LE, &[0x00, 0xD8]), None);
    assert_eq!(decode_strict(UTF_16BE, &[0xDC, 0x00]), None);
}

#[test]
fn every_label_resolves() {
    use crate::tables::labels::LABELS;
    assert_eq!(LABELS.len(), 228);
    for (label, expected) in LABELS.iter() {
        assert_eq!(
            Encoding::for_label(label.as_bytes()),
            Some(*expected),
            "label {label:?}"
        );
        // The lookup is case-insensitive and trims ASCII whitespace.
        let upper = label.to_ascii_uppercase();
        assert_eq!(Encoding::for_label(upper.as_bytes()), Some(*expected));
        let padded = alloc::format!("\t {label} \r\n");
        assert_eq!(Encoding::for_label(padded.as_bytes()), Some(*expected));
    }
    // The table has to stay sorted for the binary search to be correct.
    assert!(LABELS.windows(2).all(|w| w[0].0 < w[1].0));
}

#[test]
fn gb18030_encoder_side_table() {
    // The eighteen private use code points the encoder maps asymmetrically.
    for (scalar, bytes) in [
        (0xE78Du32, [0xA6u8, 0xD9u8]),
        (0xE796, [0xA6, 0xF3]),
        (0xE81E, [0xFE, 0x59]),
        (0xE864, [0xFE, 0xA0]),
    ] {
        let c = char::from_u32(scalar).unwrap();
        let mut out = alloc::vec::Vec::new();
        GB18030
            .new_encoder()
            .encode_from_str_without_replacement(&alloc::string::String::from(c), &mut out, true)
            .expect("side table entries are encodable");
        assert_eq!(out, bytes, "U+{scalar:04X}");
    }
}
