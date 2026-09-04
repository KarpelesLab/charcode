# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/KarpelesLab/charcode/compare/v0.1.3...v0.2.0) - 2026-09-04

### Added

- add ISO-2022-CN behind its own feature
- add EUC-JP and ISO-2022-JP themselves, on the shared JIS X 0208 delta
- add Shift_JIS itself, and rename the standard's to windows-31j
- add Big5 itself, and rename the standard's index to Big5-HKSCS
- add ISO-2022-KR behind its own feature
- add GB 2312-80, the charset `gb2312` names
- add true ISO-8859-9 and ISO-8859-11/TIS-620

### Fixed

- *(docs)* qualify the cross-links between the standard's encodings and the charsets
- distinguish a lenient superset from a remapping
- *(ci)* capture the refusal message instead of piping it
- *(ci)* assert the CLI's behaviour rather than a count
- [**breaking**] stop the general lookup returning the standard's substitutions

### Other

- 47, not 46 — ISO-2022-CN landed after that count
- describe the charsets outside the standard and why they are separate

## [0.1.3](https://github.com/KarpelesLab/charcode/compare/v0.1.2...v0.1.3) - 2026-09-04

### Fixed

- *(test)* stop racing the CLI's own exit when writing its input

### Other

- inline the hot helpers and bucket the encode tables
- stop rescanning the whole input on every streaming call

## [0.1.2](https://github.com/KarpelesLab/charcode/compare/v0.1.1...v0.1.2) - 2026-09-04

### Added

- replace the method pairs with options objects

### Fixed

- *(ci)* make the generated-tables check reproducible

## [0.1.1](https://github.com/KarpelesLab/charcode/compare/v0.1.0...v0.1.1) - 2026-09-04

### Added

- look encodings up by Windows code page number
- make `alloc` an optional default feature

### Fixed

- *(cli)* require a prefix on code page numbers, add Encoding::for_cp

### Other

- add the CI, crates.io, docs.rs and license badges
