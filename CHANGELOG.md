# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/KarpelesLab/charcode/compare/v0.1.1...v0.2.0) - 2026-09-04

### Added

- [**breaking**] replace the method pairs with options objects

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
