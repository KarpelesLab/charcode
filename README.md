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

let (text, encoding, had_errors) = WINDOWS_1252.decode(b"caf\xE9");
assert_eq!(text, "café");
assert_eq!(encoding, WINDOWS_1252);
assert!(!had_errors);
```

`decode` implements the standard's `decode` hook: a byte order mark wins over the
encoding you name, and malformed sequences become U+FFFD, with the returned flag
telling you whether any did. Encodings are looked up by any label the standard
defines, which is what a `Content-Type` header or a `<meta charset>` carries:

```rust
use charcode::Encoding;

let encoding = Encoding::for_label(b"Shift-JIS").unwrap();
assert_eq!(encoding.name(), "Shift_JIS");
let (text, _) = encoding.decode_without_bom_handling(b"\x93\xFA\x96{");
assert_eq!(text, "日本");
```

Encoding goes the other way. Characters the target cannot represent become HTML
numeric character references, the way form submission does:

```rust
use charcode::EUC_KR;

let (bytes, _, had_unmappable) = EUC_KR.encode("한국어 😀");
assert_eq!(&bytes[..], b"\xC7\xD1\xB1\xB9\xBE\xEE &#128512;");
assert!(had_unmappable);
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
decoder.decode_to_string(&[0xA4], &mut text, false);
decoder.decode_to_string(&[0x40], &mut text, true);
assert_eq!(text, "一");
```

`Decoder::decode_to_utf8` and `Encoder::encode_from_utf8` are the
allocation-free forms, writing into a `&mut [u8]` you provide. They work with an
output buffer as small as four bytes.

## Handling errors instead of substituting them

Every conversion comes in two flavours. The default substitutes, as the standard
requires for web content. The `without_replacement` forms stop at the first
problem and report it, for callers that need to reject bad input:

```rust
use charcode::{SHIFT_JIS, WINDOWS_1252};

assert!(SHIFT_JIS
    .decode_without_bom_handling_and_without_replacement(b"\x81\x20")
    .is_none());

let mut bytes = Vec::new();
let error = WINDOWS_1252
    .new_encoder()
    .encode_from_str_without_replacement("ab一", &mut bytes, true)
    .unwrap_err();
assert_eq!(error.character, '一');
assert_eq!(bytes, b"ab");
```

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
(US-ASCII) both give windows-1252. The CLI accepts these too, bare or prefixed:
`charcode -f cp932`, `-f 932` and `-f x-cp20936` all work.

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
| Single-byte | IBM866, ISO-8859-2/3/4/5/6/7/8/8-I/10/13/14/15/16, KOI8-R, KOI8-U, macintosh, windows-874, windows-1250 through windows-1258, x-mac-cyrillic |
| Chinese | GBK, gb18030, Big5 |
| Japanese | EUC-JP, ISO-2022-JP, Shift_JIS |
| Korean | EUC-KR |
| Other | replacement, x-user-defined |

UTF-16BE, UTF-16LE and `replacement` decode only; asking them to encode gives
UTF-8, which is what the standard's `get an output encoding` prescribes.

## Features

- `std` *(default)* — implements `std::error::Error` for the error types.
  Implies `alloc`.
- `alloc` *(default, via `std`)* — the conveniences that hand back an owned
  `String`, `Vec` or `Cow`: `Encoding::decode` and `encode`,
  `Decoder::decode_to_string`, `Encoder::encode_from_str`.
- `serde` — an encoding serializes as its name and deserializes from any label.
  Needs neither `std` nor `alloc`.
- `cli` — builds the `charcode` command-line tool described above. Adds no
  dependencies; it needs `std`.

With `default-features = false` and no `alloc`, what remains is the whole
conversion engine plus encoding lookup — everything in the [Converting a
stream](#converting-a-stream) section — converting into buffers you provide and
never touching a heap:

```toml
[dependencies]
charcode = { version = "0.1", default-features = false }
```

```rust
use charcode::{CoderResult, WINDOWS_1252};

let mut decoder = WINDOWS_1252.new_decoder_without_bom_handling();
let mut buffer = [0u8; 64];
let (result, _read, written, _had_errors) =
    decoder.decode_to_utf8_with_replacement(b"caf\xE9", &mut buffer, true);
assert_eq!(result, CoderResult::InputEmpty);
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
