//! Throughput measurements, run with `cargo bench`.
//!
//! Deliberately not a `#[bench]` harness: that needs nightly, and a criterion
//! dependency would sit oddly in a crate whose point is having none.  The
//! numbers are wall-clock megabytes per second over a fixed corpus, which is
//! enough to tell an optimization from a regression.

use std::hint::black_box;
use std::time::{Duration, Instant};

use charcode::{
    BIG5, Bom, DecodeOptions, EUC_JP, EUC_KR, EncodeOptions, Encoding, GB18030, ISO_2022_JP,
    SHIFT_JIS, UTF_8, UTF_16LE, Unmappable, WINDOWS_1252,
};

/// Repeats `text` until it is at least this many bytes, so every case measures
/// the steady state rather than start-up.
const TARGET: usize = 1 << 20;

fn grow(text: &str) -> String {
    let mut out = String::with_capacity(TARGET + text.len());
    while out.len() < TARGET {
        out.push_str(text);
    }
    out
}

/// Runs `body` until it has taken at least 200ms, and reports the best rate.
fn measure(name: &str, bytes: usize, mut body: impl FnMut() -> usize) {
    // Warm up, and let the result escape so nothing is optimized away.
    black_box(body());
    let mut best = Duration::MAX;
    let overall = Instant::now();
    let mut runs = 0u32;
    while overall.elapsed() < Duration::from_millis(200) || runs < 3 {
        let start = Instant::now();
        black_box(body());
        best = best.min(start.elapsed());
        runs += 1;
    }
    let mb = bytes as f64 / (1024.0 * 1024.0);
    let rate = mb / best.as_secs_f64();
    println!("{name:<44} {rate:>8.0} MiB/s");
}

fn decode_case(name: &str, encoding: &'static Encoding, bytes: &[u8]) {
    let options = DecodeOptions::new().bom(Bom::Ignore);
    let mut out = String::with_capacity(bytes.len() * 3 + 8);
    measure(name, bytes.len(), || {
        out.clear();
        let mut decoder = encoding.new_decoder_with(options);
        decoder.decode_to_string(bytes, &mut out, true).unwrap();
        out.len()
    });
}

fn encode_case(name: &str, encoding: &'static Encoding, text: &str) {
    let options = EncodeOptions::new().unmappable(Unmappable::Omit);
    let mut out = Vec::with_capacity(text.len() * 2 + 8);
    measure(name, text.len(), || {
        out.clear();
        let mut encoder = encoding.new_encoder_with(options);
        encoder.encode_from_str(text, &mut out, true).unwrap();
        out.len()
    });
}

/// Encodes `text` so the decode benchmarks have realistic input.
fn to_bytes(encoding: &'static Encoding, text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    encoding
        .new_encoder_with(EncodeOptions::new().unmappable(Unmappable::Omit))
        .encode_from_str(text, &mut out, true)
        .expect("omitting never fails");
    out
}

fn main() {
    let ascii = grow("The quick brown fox jumps over the lazy dog. 0123456789. ");
    let latin = grow(
        "Voix ambigue d'un coeur qui au zephyr prefere les jattes de kiwis. \u{E9}\u{E8}\u{E0}\u{FC}\u{F1}. ",
    );
    let cyrillic = grow(
        "\u{412} \u{447}\u{430}\u{449}\u{430}\u{445} \u{44E}\u{433}\u{430} \u{436}\u{438}\u{43B}-\u{431}\u{44B}\u{43B} \u{446}\u{438}\u{442}\u{440}\u{443}\u{441}. ",
    );
    let japanese = grow(
        "\u{3044}\u{308D}\u{306F}\u{306B}\u{307B}\u{3078}\u{3068}\u{3061}\u{308A}\u{306C}\u{308B}\u{3092}\u{FF61}\u{65E5}\u{672C}\u{8A9E}\u{306E}\u{6587}\u{7AE0}\u{3002} ",
    );
    let chinese = grow(
        "\u{4E2D}\u{6587}\u{6587}\u{672C}\u{FF0C}\u{7528}\u{4E8E}\u{6D4B}\u{8BD5}\u{7F16}\u{7801}\u{6027}\u{80FD}\u{3002} ",
    );
    let korean = grow(
        "\u{D55C}\u{AD6D}\u{C5B4} \u{BB38}\u{C7A5}\u{C744} \u{C778}\u{CF54}\u{B529} \u{C131}\u{B2A5} \u{C2DC}\u{D5D8}. ",
    );
    let mixed = grow("ascii text, then \u{E9}\u{E8} and \u{65E5}\u{672C}\u{8A9E} and \u{1F600}. ");

    println!("\n--- decode to UTF-8 ---");
    decode_case("UTF-8, ascii", UTF_8, ascii.as_bytes());
    decode_case("UTF-8, mixed scripts", UTF_8, mixed.as_bytes());
    decode_case("windows-1252, ascii", WINDOWS_1252, ascii.as_bytes());
    decode_case(
        "windows-1252, latin",
        WINDOWS_1252,
        &to_bytes(WINDOWS_1252, &latin),
    );
    decode_case(
        "windows-1251, cyrillic",
        Encoding::for_label(b"windows-1251").unwrap(),
        &to_bytes(Encoding::for_label(b"windows-1251").unwrap(), &cyrillic),
    );
    decode_case("UTF-16LE, mixed", UTF_16LE, &{
        let mut v = Vec::new();
        for c in mixed.encode_utf16() {
            v.extend_from_slice(&c.to_le_bytes());
        }
        v
    });
    decode_case(
        "Shift_JIS, japanese",
        SHIFT_JIS,
        &to_bytes(SHIFT_JIS, &japanese),
    );
    decode_case("EUC-JP, japanese", EUC_JP, &to_bytes(EUC_JP, &japanese));
    decode_case(
        "ISO-2022-JP, japanese",
        ISO_2022_JP,
        &to_bytes(ISO_2022_JP, &japanese),
    );
    decode_case("GB18030, chinese", GB18030, &to_bytes(GB18030, &chinese));
    decode_case("Big5, chinese", BIG5, &to_bytes(BIG5, &chinese));
    decode_case("EUC-KR, korean", EUC_KR, &to_bytes(EUC_KR, &korean));

    println!("\n--- encode from UTF-8 ---");
    encode_case("UTF-8, ascii", UTF_8, &ascii);
    encode_case("windows-1252, ascii", WINDOWS_1252, &ascii);
    encode_case("windows-1252, latin", WINDOWS_1252, &latin);
    encode_case(
        "windows-1251, cyrillic",
        Encoding::for_label(b"windows-1251").unwrap(),
        &cyrillic,
    );
    encode_case("Shift_JIS, japanese", SHIFT_JIS, &japanese);
    encode_case("EUC-JP, japanese", EUC_JP, &japanese);
    encode_case("ISO-2022-JP, japanese", ISO_2022_JP, &japanese);
    encode_case("GB18030, chinese", GB18030, &chinese);
    encode_case("Big5, chinese", BIG5, &chinese);
    encode_case("EUC-KR, korean", EUC_KR, &korean);

    println!("\n--- one-shot API (borrowing where it can) ---");
    let ascii_bytes = ascii.as_bytes();
    measure("UTF_8.decode, ascii (borrows)", ascii_bytes.len(), || {
        UTF_8.decode(black_box(ascii_bytes)).0.len()
    });
    measure(
        "WINDOWS_1252.decode, ascii (borrows)",
        ascii_bytes.len(),
        || WINDOWS_1252.decode(black_box(ascii_bytes)).0.len(),
    );
    let latin_1252 = to_bytes(WINDOWS_1252, &latin);
    measure(
        "WINDOWS_1252.decode, latin (owns)",
        latin_1252.len(),
        || WINDOWS_1252.decode(black_box(&latin_1252)).0.len(),
    );
}
