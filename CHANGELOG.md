# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0]

Initial release: a complete implementation of the
[WHATWG Encoding Standard](https://encoding.spec.whatwg.org/).

### Added

- All 40 encodings in the standard, and lookup by all 228 of their labels
  through `Encoding::for_label`.
- One-shot conversion with `Encoding::decode`, `decode_with_bom_removal`,
  `decode_without_bom_handling`, `decode_without_bom_handling_and_without_replacement`
  and `Encoding::encode`, returning a `Cow` that borrows where it can.
- Streaming conversion with `Decoder` and `Encoder`, including byte order mark
  sniffing and removal, and allocation-free `&mut [u8]` output.
- Optional `serde` support behind the `serde` feature.
- `no_std` support; the crate needs `alloc` and nothing else.

[Unreleased]: https://github.com/KarpelesLab/charcode/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/KarpelesLab/charcode/releases/tag/v0.1.0
