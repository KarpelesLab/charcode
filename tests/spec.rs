//! Behaviour required by the WHATWG Encoding Standard, exercised through the
//! public API.  Each test names the algorithm it comes from.
//!
//! These use the owned-output API and every encoding the standard defines, so
//! they need `alloc` and `whatwg`; the buffer API and the feature subsets are
//! covered by the crate's own tests.

#![cfg(all(feature = "alloc", feature = "whatwg"))]

use charcode::*;

fn decode(encoding: &'static Encoding, bytes: &[u8]) -> String {
    encoding
        .decode_with(bytes, DecodeOptions::new().bom(Bom::Ignore))
        .0
        .into_owned()
}

/// Encodes the way the standard's `encode` hook does, which is what most of
/// these tests are checking.
fn encode(encoding: &'static Encoding, text: &str) -> Vec<u8> {
    encoding.encode_html_form(text).0.into_owned()
}

// --- 4.2 Names and labels ------------------------------------------------

#[test]
fn labels_are_matched_case_insensitively_and_trimmed() {
    // This file is about what the standard says, so it asks the lookup that
    // implements it.  `for_label` deliberately answers differently for the
    // labels the standard reassigns; see `the_two_lookups_disagree_on_purpose`.
    let f = |l: &[u8]| Encoding::for_whatwg_label(l);
    assert_eq!(f(b"UTF-8"), Some(UTF_8));
    assert_eq!(f(b"utf8"), Some(UTF_8));
    assert_eq!(f(b"\t\n\x0C\r UTF-8 \t"), Some(UTF_8));
    assert_eq!(f(b"LATIN1"), Some(WINDOWS_1252));
    assert_eq!(f(b"ascii"), Some(WINDOWS_1252));
    assert_eq!(f(b"iso-8859-1"), Some(WINDOWS_1252));
    assert_eq!(f(b"chinese"), Some(GBK));
    assert_eq!(f(b"utf-16"), Some(UTF_16LE));
}

/// The reassignments the standard makes for web compatibility live in its own
/// lookup and nowhere else.
#[test]
fn the_two_lookups_disagree_on_purpose() {
    // A label that names one charset and is resolved by the standard to another.
    assert_eq!(Encoding::for_label(b"iso-8859-1"), Some(ISO_8859_1));
    assert_eq!(
        Encoding::for_whatwg_label(b"iso-8859-1"),
        Some(WINDOWS_1252)
    );
    assert_eq!(Encoding::for_label(b"ascii"), Some(US_ASCII));
    assert_eq!(Encoding::for_whatwg_label(b"ascii"), Some(WINDOWS_1252));

    // 0x80 is where it shows: a C1 control, or a euro sign.
    assert_eq!(decode(ISO_8859_1, b"\x80"), "\u{80}");
    assert_eq!(decode(WINDOWS_1252, b"\x80"), "\u{20AC}");
    assert_eq!(decode(US_ASCII, b"\x80"), "\u{FFFD}");

    // ISO-8859-9 and TIS-620 exist now, and are not the Windows supersets the
    // standard resolves their labels to.
    assert_eq!(Encoding::for_label(b"iso-8859-9"), Some(ISO_8859_9));
    assert_eq!(
        Encoding::for_whatwg_label(b"iso-8859-9"),
        Some(WINDOWS_1254)
    );
    assert_eq!(Encoding::for_label(b"tis-620"), Some(ISO_8859_11));
    assert_eq!(Encoding::for_whatwg_label(b"tis-620"), Some(WINDOWS_874));

    // GB 2312 is its own charset, not the GBK the standard resolves it to;
    // the two disagree at two of GB 2312's code points.
    assert_eq!(Encoding::for_label(b"gb2312"), Some(GB2312));
    assert_eq!(Encoding::for_whatwg_label(b"gb2312"), Some(GBK));
    assert_eq!(decode(GB2312, b"\xA1\xAA"), "\u{2015}");
    assert_eq!(decode(GBK, b"\xA1\xAA"), "\u{2014}");

    // A label naming a charset this build does not have yet resolves to
    // nothing, rather than quietly to something adjacent.
    assert_eq!(Encoding::for_label(b"iso-2022-kr"), None);
    assert_eq!(
        Encoding::for_whatwg_label(b"iso-2022-kr"),
        Some(REPLACEMENT)
    );

    // A *faithful* superset is a different matter.  WHATWG's EUC-KR is the
    // Unified Hangul Code, which contains every one of KS X 1001's 8224 code
    // points with none remapped, so a caller asking for `ks_c_5601-1987` gets
    // the same answer for every byte sequence that charset defines.
    for label in [&b"korean"[..], b"ks_c_5601-1987", b"ksc5601", b"iso-ir-149"] {
        assert_eq!(Encoding::for_label(label), Some(EUC_KR), "{label:?}");
    }
}

#[test]
fn unknown_labels_are_rejected() {
    assert_eq!(Encoding::for_label(b""), None);
    assert_eq!(Encoding::for_label(b"   "), None);
    assert_eq!(Encoding::for_label(b"utf 8"), None);
    assert_eq!(Encoding::for_label(b"nonsense"), None);
    // Inner whitespace is not trimmed, only leading and trailing.
    assert_eq!(Encoding::for_label(b"u tf-8"), None);
    // Longer than any label in the standard.
    assert_eq!(Encoding::for_label(&[b'a'; 100]), None);
    // Not UTF-8 at all.
    assert_eq!(Encoding::for_label(&[0xFF, 0xFE]), None);
}

#[test]
fn replacement_labels_can_be_filtered_out() {
    for label in [
        &b"csiso2022kr"[..],
        b"hz-gb-2312",
        b"iso-2022-cn",
        b"iso-2022-cn-ext",
        b"iso-2022-kr",
        b"replacement",
    ] {
        assert_eq!(
            Encoding::for_whatwg_label(label),
            Some(REPLACEMENT),
            "{label:?}"
        );
        assert_eq!(
            Encoding::for_whatwg_label_no_replacement(label),
            None,
            "{label:?}"
        );
        // The general lookup does not hand back `replacement` for a label that
        // names a real encoding; it says it has none.  `replacement` itself is
        // the exception: that label names exactly what it resolves to.
        if label != b"replacement" {
            assert_eq!(Encoding::for_label(label), None, "{label:?}");
        }
    }
    // ISO-2022-JP is a real encoding and survives the filter.
    assert_eq!(
        Encoding::for_whatwg_label_no_replacement(b"iso-2022-jp"),
        Some(ISO_2022_JP)
    );
    assert_eq!(Encoding::for_label(b"iso-2022-jp"), Some(ISO_2022_JP));
}

#[test]
fn every_encoding_is_reachable_by_its_own_name() {
    // The charsets outside the standard add to this.
    #[cfg(not(any(feature = "dos", feature = "ebcdic", feature = "mac", feature = "misc")))]
    assert_eq!(Encoding::all().len(), 40);
    for &encoding in Encoding::all() {
        // The standard's lookup answers only for its own encodings; every
        // encoding, its own or ours, is reachable by name through the general
        // one.
        assert_eq!(
            Encoding::for_label(encoding.name().as_bytes()),
            Some(encoding),
            "{}",
            encoding.name()
        );
        if encoding.is_whatwg() {
            assert_eq!(
                Encoding::for_whatwg_label(encoding.name().as_bytes()),
                Some(encoding),
                "{}",
                encoding.name()
            );
        }
    }
}

// --- 4.3 Output encodings ------------------------------------------------

#[test]
fn output_encoding_replaces_the_three_that_cannot_encode() {
    assert_eq!(REPLACEMENT.output_encoding(), UTF_8);
    assert_eq!(UTF_16BE.output_encoding(), UTF_8);
    assert_eq!(UTF_16LE.output_encoding(), UTF_8);
    assert_eq!(WINDOWS_1252.output_encoding(), WINDOWS_1252);
    for &encoding in Encoding::all() {
        let output = encoding.output_encoding();
        assert_eq!(output.output_encoding(), output, "{}", encoding.name());
    }
    // Encoding to UTF-16 gives UTF-8, and says so.
    let (bytes, encoding, _) = UTF_16LE.encode("hi").unwrap();
    assert_eq!(&bytes[..], b"hi");
    assert_eq!(encoding, UTF_8);
}

// --- 6.1 BOM sniffing ----------------------------------------------------

#[test]
fn a_bom_overrides_the_named_encoding() {
    let (text, encoding, _) = WINDOWS_1252.decode(b"\xEF\xBB\xBFcaf\xC3\xA9");
    assert_eq!(text, "caf\u{E9}");
    assert_eq!(encoding, UTF_8);

    let (text, encoding, _) = WINDOWS_1252.decode(b"\xFF\xFEa\x00b\x00");
    assert_eq!(text, "ab");
    assert_eq!(encoding, UTF_16LE);

    let (text, encoding, _) = WINDOWS_1252.decode(b"\xFE\xFF\x00a\x00b");
    assert_eq!(text, "ab");
    assert_eq!(encoding, UTF_16BE);
}

#[test]
fn bom_removal_only_strips_this_encodings_own_mark() {
    let remove = DecodeOptions::new().bom(Bom::Remove);
    assert_eq!(UTF_8.decode_with(b"\xEF\xBB\xBFa", remove).0, "a");
    // A UTF-16 mark is not UTF-8's, so it decodes as content.
    assert_eq!(
        UTF_8.decode_with(b"\xFF\xFEa", remove).0,
        "\u{FFFD}\u{FFFD}a"
    );
    assert_eq!(
        WINDOWS_1252.decode_with(b"\xEF\xBB\xBFa", remove).0,
        "\u{EF}\u{BB}\u{BF}a"
    );
}

#[test]
fn without_bom_handling_keeps_the_mark_as_content() {
    assert_eq!(
        UTF_8
            .decode_with(b"\xEF\xBB\xBFa", DecodeOptions::new().bom(Bom::Ignore))
            .0,
        "\u{FEFF}a"
    );
    assert_eq!(Encoding::for_bom(b"\xEF\xBB\xBF"), Some((UTF_8, 3)));
    assert_eq!(Encoding::for_bom(b"\xEF\xBB"), None);
    assert_eq!(Encoding::for_bom(b""), None);
}

#[test]
fn a_bom_split_across_chunks_is_still_recognized() {
    let mut decoder = WINDOWS_1252.new_decoder();
    let mut text = String::new();
    decoder.decode_to_string(b"\xEF", &mut text, false).unwrap();
    decoder.decode_to_string(b"\xBB", &mut text, false).unwrap();
    decoder
        .decode_to_string(b"\xBFcaf\xC3\xA9", &mut text, true)
        .unwrap();
    assert_eq!(text, "caf\u{E9}");
    assert_eq!(decoder.encoding(), UTF_8);
}

#[test]
fn a_near_miss_bom_is_decoded_as_content() {
    let mut decoder = WINDOWS_1252.new_decoder();
    let mut text = String::new();
    decoder
        .decode_to_string(b"\xEF\xBB", &mut text, false)
        .unwrap();
    decoder.decode_to_string(b"\x41", &mut text, true).unwrap();
    assert_eq!(text, "\u{EF}\u{BB}A");
    assert_eq!(decoder.encoding(), WINDOWS_1252);
}

// --- 8.1 UTF-8 -----------------------------------------------------------

#[test]
fn utf8_substitutes_maximal_subparts() {
    // The substitutions match Unicode's "Best Practices for Using U+FFFD",
    // which is what the standard requires.
    assert_eq!(decode(UTF_8, b"\xF0\x9F\x98\x80"), "\u{1F600}");
    assert_eq!(decode(UTF_8, b"\xF0\x9F\x98"), "\u{FFFD}");
    assert_eq!(decode(UTF_8, b"\xE0\x80\x80"), "\u{FFFD}\u{FFFD}\u{FFFD}");
    assert_eq!(decode(UTF_8, b"\xED\xA0\x80"), "\u{FFFD}\u{FFFD}\u{FFFD}");
    assert_eq!(decode(UTF_8, b"\xC0\xAF"), "\u{FFFD}\u{FFFD}");
    assert_eq!(
        decode(UTF_8, b"\xF4\x90\x80\x80"),
        "\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}"
    );
    assert_eq!(decode(UTF_8, b"a\xE2\x82b"), "a\u{FFFD}b");
    assert_eq!(
        String::from_utf8_lossy(b"\x80\xE1\xA0\xC0\x41"),
        decode(UTF_8, b"\x80\xE1\xA0\xC0\x41")
    );
}

#[test]
fn valid_utf8_is_borrowed_not_copied() {
    let ignore = DecodeOptions::new().bom(Bom::Ignore);
    let bytes = "caf\u{E9}".as_bytes();
    assert!(matches!(
        UTF_8.decode_with(bytes, ignore).0,
        std::borrow::Cow::Borrowed(_)
    ));
    // An ASCII-compatible encoding borrows ASCII too.
    assert!(matches!(
        WINDOWS_1252.decode_with(b"ascii", ignore).0,
        std::borrow::Cow::Borrowed(_)
    ));
    // But not once a byte means something other than itself.
    assert!(matches!(
        WINDOWS_1252.decode_with(b"caf\xE9", ignore).0,
        std::borrow::Cow::Owned(_)
    ));
}

// --- 9 Legacy single-byte ------------------------------------------------

#[test]
fn single_byte_unmapped_bytes_become_replacements() {
    // windows-1252 maps every byte, including the C1 controls.
    assert_eq!(decode(WINDOWS_1252, b"\x81\x8D\x90"), "\u{81}\u{8D}\u{90}");
    // ISO-8859-6 leaves most of its upper half unmapped.
    assert_eq!(decode(ISO_8859_6, b"\xA1"), "\u{FFFD}");
    assert_eq!(decode(ISO_8859_6, b"\xA4"), "\u{A4}");
    assert_eq!(ISO_8859_8_I.name(), "ISO-8859-8-I");
    assert_eq!(decode(ISO_8859_8_I, b"\xE0"), decode(ISO_8859_8, b"\xE0"));
}

// --- 10.2 gb18030 --------------------------------------------------------

#[test]
fn gb18030_restores_bytes_from_an_incomplete_four_byte_sequence() {
    // A lead byte and a digit, then a byte that cannot continue: the digit and
    // the offending byte are pushed back and decoded again.
    assert_eq!(decode(GB18030, b"\x81\x30\x41"), "\u{FFFD}0A");
    // Three bytes in, the second and third are pushed back as well.
    assert_eq!(decode(GB18030, b"\x81\x30\x81\x41"), "\u{FFFD}0\u{4E04}");
    // A truncated sequence at the end of the stream is a single error.
    assert_eq!(decode(GB18030, b"\x81\x30\x81"), "\u{FFFD}");
    assert_eq!(decode(GB18030, b"\x81"), "\u{FFFD}");
}

#[test]
fn gb18030_maps_0x80_to_the_euro_sign() {
    assert_eq!(decode(GB18030, b"\x80"), "\u{20AC}");
    // GBK encodes the euro as 0x80; gb18030 uses its index entry instead.
    assert_eq!(encode(GBK, "\u{20AC}"), b"\x80");
    assert_eq!(encode(GB18030, "\u{20AC}"), b"\xA2\xE3");
}

#[test]
fn gb18030_four_byte_sequences_cover_the_rest_of_unicode() {
    assert_eq!(decode(GB18030, b"\x90\x30\x81\x30"), "\u{10000}");
    assert_eq!(encode(GB18030, "\u{10000}"), b"\x90\x30\x81\x30");
    assert_eq!(decode(GB18030, b"\xE3\x32\x9A\x35"), "\u{10FFFF}");
    assert_eq!(encode(GB18030, "\u{10FFFF}"), b"\xE3\x32\x9A\x35");
    // The last pointer below the four-byte gap is U+FFFF.
    assert_eq!(decode(GB18030, b"\x84\x31\xA4\x39"), "\u{FFFF}");
    // GBK's encoder never emits four-byte sequences.
    assert_eq!(encode(GBK, "\u{10000}"), b"&#65536;");
}

#[test]
fn gb18030_has_an_asymmetric_private_use_table() {
    // U+E5E5 cannot be encoded: 0xA3 0xA0 decodes to U+3000 for compatibility.
    assert_eq!(decode(GB18030, b"\xA3\xA0"), "\u{3000}");
    assert_eq!(encode(GB18030, "\u{E5E5}"), b"&#58853;");
    // These keep their GB18030-2005 two-byte forms even though decoding them
    // now gives a different code point.
    assert_eq!(encode(GB18030, "\u{E78D}"), b"\xA6\xD9");
    assert_eq!(decode(GB18030, b"\xA6\xD9"), "\u{FE10}");
    assert_eq!(encode(GB18030, "\u{E864}"), b"\xFE\xA0");
    // U+E7C7 has a pointer of its own in the ranges.
    assert_eq!(encode(GB18030, "\u{E7C7}"), b"\x81\x35\xF4\x37");
    assert_eq!(decode(GB18030, b"\x81\x35\xF4\x37"), "\u{E7C7}");
}

// --- 11.1 Big5 -----------------------------------------------------------

#[test]
fn big5_pointers_that_decode_to_two_scalar_values() {
    assert_eq!(decode(BIG5, b"\x88\x62"), "\u{CA}\u{304}");
    assert_eq!(decode(BIG5, b"\x88\x64"), "\u{CA}\u{30C}");
    assert_eq!(decode(BIG5, b"\x88\xA3"), "\u{EA}\u{304}");
    assert_eq!(decode(BIG5, b"\x88\xA5"), "\u{EA}\u{30C}");
}

#[test]
fn big5_encoder_avoids_the_hkscs_extensions() {
    // Six code points appear twice; the standard picks the later pointer so that
    // encoding does not produce an extension byte sequence.
    for (text, bytes) in [
        ("\u{2550}", &b"\xF9\xF9"[..]),
        ("\u{255E}", b"\xF9\xE9"),
        ("\u{2561}", b"\xF9\xEB"),
        ("\u{256A}", b"\xF9\xEA"),
        ("\u{5341}", b"\xA4\x51"),
        ("\u{5345}", b"\xA4\xCA"),
    ] {
        assert_eq!(encode(BIG5, text), bytes, "{text:?}");
        assert_eq!(decode(BIG5, bytes), text);
    }
}

#[test]
fn big5_restores_an_ascii_trail_byte() {
    // 0x81 is a lead byte but 0x21 cannot follow it, so 0x21 is decoded again.
    assert_eq!(decode(BIG5, b"\x81\x21"), "\u{FFFD}!");
    assert_eq!(decode(BIG5, b"\x81\xFF"), "\u{FFFD}");
}

// --- 12 Japanese ---------------------------------------------------------

#[test]
fn euc_jp_decodes_jis0212_but_never_encodes_it() {
    assert_eq!(decode(EUC_JP, b"\x8F\xA2\xC2"), "\u{A1}");
    assert_eq!(encode(EUC_JP, "\u{A1}"), b"&#161;");
    // Half-width katakana uses the 0x8E prefix in both directions.
    assert_eq!(decode(EUC_JP, b"\x8E\xB1"), "\u{FF71}");
    assert_eq!(encode(EUC_JP, "\u{FF71}"), b"\x8E\xB1");
}

#[test]
fn shift_jis_end_user_defined_characters() {
    assert_eq!(decode(SHIFT_JIS, b"\xF0\x40"), "\u{E000}");
    assert_eq!(decode(SHIFT_JIS, b"\xF9\xFC"), "\u{E757}");
    // 0x80 is a code point, not a lead byte.
    assert_eq!(decode(SHIFT_JIS, b"\x80"), "\u{80}");
    // 0xA0 is neither.
    assert_eq!(decode(SHIFT_JIS, b"\xA0\x41"), "\u{FFFD}A");
}

#[test]
fn japanese_encoders_fold_the_yen_and_overline() {
    assert_eq!(encode(SHIFT_JIS, "\u{A5}\u{203E}"), b"\x5C\x7E");
    assert_eq!(encode(EUC_JP, "\u{A5}\u{203E}"), b"\x5C\x7E");
    // ISO-2022-JP has a mode in which those two bytes mean exactly that.
    assert_eq!(encode(ISO_2022_JP, "\u{A5}"), b"\x1B(J\x5C\x1B(B");
}

#[test]
fn iso_2022_jp_concatenation_hazard_from_the_standard() {
    // The standard's own example: encoding U+00A5 twice and concatenating the
    // results does not decode back to two yen signs.
    let once = encode(ISO_2022_JP, "\u{A5}");
    let twice: Vec<u8> = once.iter().chain(once.iter()).copied().collect();
    assert_eq!(decode(ISO_2022_JP, &twice), "\u{A5}\u{FFFD}\u{A5}");
}

#[test]
fn iso_2022_jp_rejects_back_to_back_escapes() {
    // Two escapes with no output in between is an error, which is what stops an
    // escape sequence from being smuggled past a filter.
    assert_eq!(decode(ISO_2022_JP, b"\x1B(B\x1B(B"), "\u{FFFD}");
    assert_eq!(decode(ISO_2022_JP, b"\x1B(BA\x1B(B"), "A");
    // An escape that names no mode is an error, and its bytes are decoded again.
    assert_eq!(decode(ISO_2022_JP, b"\x1B(Z"), "\u{FFFD}(Z");
    assert_eq!(decode(ISO_2022_JP, b"\x1BA"), "\u{FFFD}A");
    assert_eq!(decode(ISO_2022_JP, b"\x1B"), "\u{FFFD}");
}

#[test]
fn iso_2022_jp_encoder_returns_to_ascii_at_the_end() {
    assert_eq!(encode(ISO_2022_JP, "\u{65E5}"), b"\x1B$B\x46\x7C\x1B(B");
    assert_eq!(encode(ISO_2022_JP, "a\u{65E5}b"), b"a\x1B$B\x46\x7C\x1B(Bb");
    // The escape-introducing characters can never be encoded.
    assert_eq!(encode(ISO_2022_JP, "\x1B"), b"&#65533;");
    assert_eq!(encode(ISO_2022_JP, "\x0E\x0F"), b"&#65533;&#65533;");
    // Half-width katakana has no ISO-2022-JP form and folds to full-width.
    assert_eq!(
        decode(ISO_2022_JP, &encode(ISO_2022_JP, "\u{FF71}")),
        "\u{30A2}"
    );
}

// --- 14.1 replacement ----------------------------------------------------

#[test]
fn replacement_decodes_anything_to_one_error() {
    assert_eq!(decode(REPLACEMENT, b""), "");
    assert_eq!(decode(REPLACEMENT, b"a"), "\u{FFFD}");
    assert_eq!(decode(REPLACEMENT, b"a long stretch of bytes"), "\u{FFFD}");
    assert!(
        !REPLACEMENT
            .decode_with(b"x", DecodeOptions::new().bom(Bom::Ignore))
            .2
            .is_lossless()
    );
}

// --- 14.5 x-user-defined -------------------------------------------------

#[test]
fn x_user_defined_maps_the_upper_half_to_private_use() {
    assert_eq!(decode(X_USER_DEFINED, b"\x80\xFF"), "\u{F780}\u{F7FF}");
    assert_eq!(encode(X_USER_DEFINED, "\u{F780}\u{F7FF}"), b"\x80\xFF");
    assert_eq!(encode(X_USER_DEFINED, "\u{F77F}"), b"&#63359;");
}

// --- Encoder error handling ----------------------------------------------

#[test]
fn unmappable_characters_become_numeric_references() {
    let (bytes, _, tally) = WINDOWS_1252.encode_html_form("a\u{4E00}b");
    assert_eq!(&bytes[..], b"a&#19968;b");
    assert_eq!(tally.errors, 1);
    assert_eq!(encode(WINDOWS_1252, "\u{10FFFF}"), b"&#1114111;");
    let (_, _, tally) = WINDOWS_1252.encode_html_form("plain");
    assert!(tally.is_lossless());
}

#[test]
fn without_replacement_reports_the_offending_character() {
    let mut bytes = Vec::new();
    let error = WINDOWS_1252
        .new_encoder()
        .encode_from_str("ab\u{4E00}", &mut bytes, true)
        .unwrap_err();
    assert_eq!(error.character, '\u{4E00}');
    assert_eq!(error.offset, 5);
    assert_eq!(bytes, b"ab");

    let strict = DecodeOptions::new()
        .bom(Bom::Ignore)
        .malformed(Malformed::Fail);
    assert_eq!(
        WINDOWS_1252
            .try_decode(b"caf\xE9", strict)
            .map(|(t, _, _)| t),
        Ok(std::borrow::Cow::Owned(String::from("caf\u{E9}")))
    );
    assert!(ISO_8859_6.try_decode(b"\xA1", strict).is_err());
}

#[test]
fn a_four_byte_output_buffer_is_enough_to_make_progress() {
    let mut decoder = BIG5.new_decoder_with(DecodeOptions::new().bom(Bom::Ignore));
    let mut buffer = [0u8; DECODER_MIN_BUFFER];
    let (result, read, written) = decoder.decode_to_utf8(b"\x88\x62\x41", &mut buffer, true);
    assert_eq!(result, DecoderResult::OutputFull);
    assert_eq!((read, written), (2, 4));
    assert_eq!(&buffer[..4], "\u{CA}\u{304}".as_bytes());

    let mut encoder = GB18030.new_encoder();
    let mut buffer = [0u8; ENCODER_MIN_BUFFER];
    let (result, _, written) = encoder.encode_from_utf8("\u{10000}\u{10001}", &mut buffer, true);
    assert_eq!(result, EncoderResult::OutputFull);
    assert_eq!(&buffer[..written], b"\x900\x810");
}

// --- Encoding as a value -------------------------------------------------

#[test]
fn encodings_compare_and_print_by_name() {
    assert_eq!(UTF_8, UTF_8);
    assert_ne!(UTF_8, WINDOWS_1252);
    assert_eq!(format!("{UTF_8}"), "UTF-8");
    assert_eq!(format!("{UTF_8:?}"), "Encoding(UTF-8)");
    assert!(WINDOWS_1252.is_single_byte());
    assert!(X_USER_DEFINED.is_single_byte());
    assert!(!BIG5.is_single_byte());
    assert!(WINDOWS_1252.is_ascii_compatible());
    assert!(!UTF_16LE.is_ascii_compatible());
    assert!(!ISO_2022_JP.is_ascii_compatible());
    assert!(!REPLACEMENT.is_ascii_compatible());
}

// --- the standard's lookup is a boundary, not a convenience ---------------

#[test]
fn the_whatwg_lookup_refuses_everything_outside_the_standard() {
    // Every label the standard defines resolves through it...
    assert_eq!(Encoding::for_whatwg_label(b"latin1"), Some(WINDOWS_1252));
    assert_eq!(
        Encoding::for_whatwg_label(b"\tShift-JIS\n"),
        Some(SHIFT_JIS)
    );
    assert_eq!(Encoding::for_whatwg_label(b"hz-gb-2312"), Some(REPLACEMENT));
    assert_eq!(
        Encoding::for_whatwg_label_no_replacement(b"hz-gb-2312"),
        None
    );

    // ...and every encoding it can return is one the standard defines.
    for &encoding in Encoding::all() {
        for label in encoding.labels() {
            if let Some(found) = Encoding::for_whatwg_label(label.as_bytes()) {
                assert!(found.is_whatwg(), "{label} reached {}", found.name());
            }
        }
    }

    // The charsets the extra groups add are never selectable this way, however
    // many of them are compiled in.
    for label in [
        &b"cp437"[..],
        b"ibm437",
        b"cp037",
        b"ebcdic-cp-us",
        b"x-mac-greek",
        b"atari-st",
        b"kz-1048",
    ] {
        assert_eq!(Encoding::for_whatwg_label(label), None, "{label:?}");
        // The general lookup still finds them when their group is compiled in.
        #[cfg(feature = "extras")]
        assert!(Encoding::for_label(label).is_some(), "{label:?}");
    }
}

#[test]
fn is_whatwg_marks_exactly_the_standards_encodings() {
    for &encoding in Encoding::all() {
        // These are ours, not the standard's: it resolves their labels to a
        // Windows superset instead.
        if matches!(
            encoding.name(),
            "ISO-8859-1" | "ISO-8859-9" | "ISO-8859-11" | "US-ASCII" | "GB2312"
        ) {
            assert!(!encoding.is_whatwg(), "{}", encoding.name());
            continue;
        }
        let standard = matches!(
            encoding.name(),
            "UTF-8" | "UTF-16BE" | "UTF-16LE" | "replacement" | "x-user-defined"
        ) || (encoding.name().starts_with("ISO-8859")
            && encoding.name() != "ISO-8859-1")
            || encoding.name().starts_with("windows-")
            || encoding.name().starts_with("KOI8")
            || matches!(
                encoding.name(),
                "IBM866"
                    | "macintosh"
                    | "x-mac-cyrillic"
                    | "GBK"
                    | "gb18030"
                    | "Big5"
                    | "EUC-JP"
                    | "ISO-2022-JP"
                    | "Shift_JIS"
                    | "EUC-KR"
            );
        assert_eq!(encoding.is_whatwg(), standard, "{}", encoding.name());
    }
}
