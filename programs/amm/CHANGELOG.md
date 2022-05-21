# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a
Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2022-05-20

### Added

- An endpoint `create_farm` which allows permission-less creation of new
  staking pools called `Farm`s.
- A new error variant `InvalidAccountInput` which means some sort of
  unspecified constraints violation. When using this variant, one invokes
  `err::acc` function and provides a message to be logged.

### Changed

- Renamed `ring_buffer` property to `snapshots` on the `Farm` account.
- Changed type of `ring_buffer_tip` from `u32` to `u64` because otherwise Rust
  incorrectly aligns the data and the expected size by anchor and by Rust
  doesn't match.

### Fixed

- Usage of `#[zero_copy]` and `#[account(zero_copy)]` for `Farm` and its
  children structs was incorrect. We were supposed to use the latter for `Farm`
  and the former for the children structs, as per the [`zero_copy`
  example](https://github.com/project-serum/anchor/tree/v0.24.2/tests/zero-copy).

## [0.2.0] - 2022-05-20

### Added

- Model for `Farm` according to the design doc.
- Model for `Farmer` according to the design doc.

## [0.1.0] - 2022-05-12

### Added

- Skeleton for models, endpoints and other modules. The skeleton is analogous
  to our other programs.
- A model for `Farm` which enables farming of a specific harvest mint.
