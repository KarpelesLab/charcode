# charcode

[![CI](https://github.com/KarpelesLab/charcode/actions/workflows/ci.yml/badge.svg)](https://github.com/KarpelesLab/charcode/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/charcode.svg)](https://crates.io/crates/charcode)
[![docs.rs](https://img.shields.io/docsrs/charcode)](https://docs.rs/charcode)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Character encoding conversion for Rust, implementing the
[WHATWG Encoding Standard][spec] — the set of encodings, labels and error
behaviours that browsers actually use.

- **No dependencies.** Nothing outside the standard library. `serde` is optional
  and off by default.
- **No `unsafe`.** The crate is `#![forbid(unsafe_code)]`.
- **`no_std`.** Works with an allocator, or without one.
- **Complete.** All 40 encodings in the standard and all 228 of their labels.

```toml
[dependencies]
charcode = "0.1"
```

## Converting a buffer

```rust
use charcode::{Encoding, WINDOWS_1252};

let (text, encoding, tally) = WINDOWS_1252.decode(b"caf\xE9");
assert_eq!(text, "café");
assert_eq!(encoding, WINDOWS_1252);
assert!(tally.is_lossless());
```

`decode` implements the standard's `decode` hook: a byte order mark wins over the
encoding you name, and malformed sequences become U+FFFD, with the returned flag
telling you whether any did. Encodings are looked up by any label the standard
defines, which is what a `Content-Type` header or a `<meta charset>` carries:

```rust
use charcode::Encoding;

let encoding = Encoding::for_whatwg_label(b"Shift-JIS").unwrap();
assert_eq!(encoding.name(), "Shift_JIS");
let (text, _, _) = encoding.decode(b"\x93\xFA\x96{");
assert_eq!(text, "日本");
```

Encoding goes the other way, and **stops** at the first character the target
cannot represent — quietly mangling text is never the default:

```rust
use charcode::{EncodeOptions, EUC_KR, Unmappable};

let (bytes, _, _) = EUC_KR.encode("한국어").unwrap();
assert_eq!(&bytes[..], b"\xC7\xD1\xB1\xB9\xBE\xEE");

// An emoji is not in EUC-KR, so say what should happen to it.
assert!(EUC_KR.encode("한국어 😀").is_err());

let options = EncodeOptions::new().unmappable(Unmappable::Replace('?'));
let (bytes, _, tally) = EUC_KR.encode_with("한국어 😀", options).unwrap();
assert_eq!(&bytes[..], b"\xC7\xD1\xB1\xB9\xBE\xEE ?");
assert_eq!(tally.errors, 1);
```

Both return a [`Cow`], borrowed when the input already is the output, so ASCII
through an ASCII-compatible encoding costs only the scan.

## Converting a stream

For input that arrives in pieces, a `Decoder` carries a partial sequence from one
buffer to the next. Pass `last = true` with the final buffer so a truncated
sequence is reported rather than dropped:

```rust
use charcode::BIG5;

let mut decoder = BIG5.new_decoder();
let mut text = String::new();
decoder.decode_to_string(&[0xA4], &mut text, false).unwrap();
decoder.decode_to_string(&[0x40], &mut text, true).unwrap();
assert_eq!(text, "一");
```

`Decoder::decode_to_utf8` and `Encoder::encode_from_utf8` are the
allocation-free forms, writing into a `&mut [u8]` you provide. They work with an
output buffer as small as four bytes.

## Error policies

`DecodeOptions` and `EncodeOptions` say what to do about input that does not
decode and characters the target cannot represent. Decoding substitutes U+FFFD
by default, as the standard requires; encoding fails by default, because every
alternative changes the text.

| | `Malformed` | `Unmappable` |
| --- | --- | --- |
| stop and report | `Fail` | `Fail` *(default)* |
| drop it | `Omit` | `Omit` |
| write a character | `Replace(c)` *(default U+FFFD)* | `Replace(c)` |
| `&#19968;` | | `Html` |
| `\u4e00` | | `JsonEscape` |

```rust
use charcode::{Bom, DecodeOptions, EncodeOptions, Malformed, Unmappable, WINDOWS_1252};

// Reject anything that does not decode cleanly.
let strict = DecodeOptions::new().malformed(Malformed::Fail);
assert!(WINDOWS_1252.try_decode(b"\x81", strict).is_ok()); // windows-1252 maps every byte
assert!(charcode::UTF_8.try_decode(b"\xFF", strict).is_err());

// Escape what windows-1252 cannot hold, as JSON would.
let options = EncodeOptions::new().unmappable(Unmappable::JsonEscape);
let (bytes, _, _) = WINDOWS_1252.encode_with("a\\b一", options).unwrap();
assert_eq!(&bytes[..], b"a\\\\b\\u4e00");
```

`Html` and `JsonEscape` also rewrite their own introducer — `&` becomes `&amp;`,
`\` becomes `\\` — so what they write reads back unambiguously. Without that, a
literal `&#65;` in the input would come back as `A`.

The `translit` feature adds `EncodeOptions::transliterate`, `iconv`'s
`//TRANSLIT`: try a close ASCII equivalent first — `é` as `e`, `œ` as `oe`, `—`
as `-`, `€` as `EUR` — and fall through to the policy above for a character with
no sensible one. It carries about 30 KiB of table derived from Unicode's own
decompositions, and is off by default.

For HTML form submission specifically, `Encoding::encode_html_form` is the
standard's `encode` hook: numeric references, and no `&` escaping, which is the
ambiguity form submission has always had.

## Windows code pages

Encodings can also be looked up by Microsoft code page number, which is what
`GetACP`, a .NET `Encoding.CodePage` or an old database column reports:

```rust
use charcode::{Encoding, SHIFT_JIS, WINDOWS_1252};

assert_eq!(Encoding::for_windows_code_page(932), Some(SHIFT_JIS));
assert_eq!(Encoding::for_windows_code_page(1252), Some(WINDOWS_1252));
assert_eq!(WINDOWS_1252.windows_code_page(), Some(1252));
```

A number for an encoding the standard folds into a superset resolves to that
superset, exactly as the equivalent label does — 28591 (ISO-8859-1) and 20127
(US-ASCII) both give windows-1252. `Encoding::for_cp` is the same lookup under
the name a `cp932`-style spelling suggests. The CLI accepts those spellings too
— `charcode -f cp932`, `-f windows-932`, `-f x-cp20936` — though a bare number
is not a charset name and stays an error.

## Command-line tool

The `cli` feature builds `charcode`, an `iconv`-style converter that adds no
dependencies of its own:

```sh
cargo install charcode --features cli
```

```console
$ printf 'caf\xe9' | charcode -f latin1 -t utf-8
café
$ charcode -f shift_jis -t utf-8 notes.txt > notes.utf8.txt
$ charcode --list-labels | grep 'windows-1252$'
```

Like `iconv`, it stops at the first byte it cannot convert, `-c` omits the
offending input instead, and either way a lossy run exits non-zero so a script
can tell the difference. `--substitute` asks for the standard's web behaviour
instead — U+FFFD for malformed input, a numeric character reference for a
character the output encoding cannot represent:

```console
$ printf 'a\xffb' | charcode
charcode: (standard input): malformed byte sequence of 1 byte(s) at offset 1
Use -c to omit it, or --substitute to replace it with U+FFFD.
$ printf 'a\xffb' | charcode --substitute
a<U+FFFD>b
```

Any label works wherever an encoding is named, a leading byte order mark
overrides `-f`, and `iconv`'s `//IGNORE` suffix is accepted as a synonym for
`-c`. `charcode --help` lists the rest.

## Supported encodings

| Group | Encodings |
| --- | --- |
| Unicode | UTF-8, UTF-16BE, UTF-16LE |
| Always present | ISO-8859-1, US-ASCII (identity maps, no tables) |
| Single-byte | IBM866, ISO-8859-2/3/4/5/6/7/8/8-I/10/13/14/15/16, KOI8-R, KOI8-U, macintosh, windows-874, windows-1250 through windows-1258, x-mac-cyrillic |
| Chinese | GBK, gb18030, Big5 |
| Japanese | EUC-JP, ISO-2022-JP, Shift_JIS |
| Korean | EUC-KR |
| Other | replacement, x-user-defined |

UTF-16BE, UTF-16LE and `replacement` decode only; asking them to encode gives
UTF-8, which is what the standard's `get an output encoding` prescribes.

## Features

### Capabilities

- `std` *(default)* — implements `std::error::Error` for the error types.
  Implies `alloc`.
- `alloc` *(default, via `std`)* — the conveniences that hand back an owned
  `String`, `Vec` or `Cow`: `Encoding::decode` and `encode`,
  `Decoder::decode_to_string`, `Encoder::encode_from_str`.
- `serde` — an encoding serializes as its name and deserializes from any label.
  Needs neither `std` nor `alloc`.
- `cli` — builds the `charcode` command-line tool described above. Adds no
  dependencies; it needs `std`.

### Which encodings are compiled in

- `whatwg` *(default)* — the standard's 40 encodings, plus `whatwg-aliases`.
- `single-byte` — the standard's 28 legacy single-byte encodings.
- `big5`, `euc-jp`, `euc-kr`, `gb18030`, `iso-2022-jp`, `shift-jis` — one per
  legacy multi-byte encoding, because theirs are the large tables. `gb18030`
  also provides GBK.
- `extras` — the four groups below at once.
- `dos` — IBM PC / OEM code pages: 437, 737, 775, 850, 852, 855, 856, 857,
  860–865, 869, 1006.
- `ebcdic` — IBM mainframe code pages: 037, 424, 500, 875, 1026.
- `mac` — Apple's regional variants of Mac OS Roman: Arabic, Celtic, Central
  European, Croatian, Farsi, Gaelic, Greek, Icelandic, Romanian, Turkish.
- `misc` — Atari ST and KZ-1048.
- `unicode-extras` — UTF-32BE/LE and UTF-7. No tables; these are algorithmic.

UTF-8, UTF-16BE/LE, `replacement` and `x-user-defined` need no tables and are
always present. Static data ranges from about 1 KiB with no table group, to
540 KiB for the whole standard, to 560 KiB for everything.

### Two lookups, on purpose

`Encoding::for_label` answers with the charset a label **names**.

`Encoding::for_whatwg_label` (behind `whatwg-aliases`) answers with what the
WHATWG Encoding Standard **resolves** it to. For 52 of its labels that is a
different charset — usually a superset a browser is more likely to have meant:

```rust
use charcode::{Encoding, ISO_8859_1, WINDOWS_1252};

// `iso-8859-1` names ISO-8859-1, where byte 0x80 is a C1 control.
assert_eq!(Encoding::for_label(b"iso-8859-1"), Some(ISO_8859_1));
assert_eq!(ISO_8859_1.decode(b"\x80").0, "\u{80}");

// The standard sends it to windows-1252, where 0x80 is a euro sign.
assert_eq!(Encoding::for_whatwg_label(b"iso-8859-1"), Some(WINDOWS_1252));
assert_eq!(WINDOWS_1252.decode(b"\x80").0, "€");
```

The same goes for `ascii`, `us-ascii`, `iso-8859-9`, `tis-620`, `gb2312`,
`ks_c_5601-1987` and the rest. `for_label` never substitutes: a label naming a
charset this build does not carry gives `None` rather than something close to
it.

The standard's lookup is also a boundary in the other direction — it answers
**only** with encodings the standard sanctions. It leaves some charsets out
deliberately (UTF-7 and HZ-GB-2312 can both be used to smuggle markup past a
filter that only inspects the bytes), so a build that adds `unicode-extras` or
`dos` for local use does not thereby widen what a label off the network can
select:

```rust
use charcode::Encoding;

assert_eq!(Encoding::for_whatwg_label(b"utf-7"), None);
assert_eq!(Encoding::for_whatwg_label(b"cp437"), None);
```

The alias layer is independent of which tables you take:

```toml
# Japanese and Unicode, with the standard's naming, plus the DOS code pages
# available locally but not selectable by a remote label.
charcode = { version = "0.1", default-features = false, features = [
    "std", "whatwg-aliases", "shift-jis", "euc-jp", "iso-2022-jp", "dos",
] }
```

With `default-features = false` and no `alloc`, what remains is the whole
conversion engine plus encoding lookup — everything in the [Converting a
stream](#converting-a-stream) section — converting into buffers you provide and
never touching a heap:

```toml
[dependencies]
charcode = { version = "0.1", default-features = false }
```

```rust
use charcode::{Bom, DecodeOptions, DecoderResult, WINDOWS_1252};

let mut decoder = WINDOWS_1252.new_decoder_with(DecodeOptions::new().bom(Bom::Ignore));
let mut buffer = [0u8; 64];
let (result, _read, written) = decoder.decode_to_utf8(b"caf\xE9", &mut buffer, true);
assert_eq!(result, DecoderResult::InputEmpty);
assert_eq!(core::str::from_utf8(&buffer[..written]).unwrap(), "café");
```

## Relation to `encoding_rs`

Both implement the same standard and should agree on every conversion.
`encoding_rs` is the one Firefox ships and is faster, using SIMD and a good deal
of `unsafe` to get there. `charcode` trades that for having no dependencies, no
`unsafe`, and a smaller surface to audit. Use `encoding_rs` if throughput on
large documents is what matters; use `charcode` if a dependency-free, `unsafe`-free
build is.

## Index tables

The tables in `src/tables/` are generated from the standard's own
[`indexes.json`][indexes] by `tools/generate_tables.py`, from the copy of that data checked in under
`tools/data/`. The generated sources are checked in too, so
building the crate needs no build script and no network. Re-run the generator
only when the upstream indexes change:

```sh
python3 tools/generate_tables.py
```

## Minimum supported Rust version

1.88. Raising it is a breaking change.

## License

MIT. See [LICENSE](LICENSE).

[spec]: https://encoding.spec.whatwg.org/
[indexes]: https://encoding.spec.whatwg.org/indexes.json
[`Cow`]: https://doc.rust-lang.org/std/borrow/enum.Cow.html
