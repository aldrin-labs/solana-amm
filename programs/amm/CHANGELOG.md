# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a
Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.5.1] - 2022-05-27

### Added

- Endpoint for set farm owner.

## [0.5.0] - 2022-05-26

### Added

- Endpoint for taking a snapshot.
- Method on `Farm` model called `take_snapshot` where the
  core logic of the `take_snapshot` endpoint resides. It computes
  the total amount staked and the current slot, and stores it in a
  snapshot while ticking the ring_buffer_tip.
- Added `min_snapshot_window_slots` field in `Farm`

### Removed

- Removed `vesting_vault` field in `Farm`

## [0.4.2] - 2022-05-23

### Added

- Endpoint for removing a harvest mint.

## [0.4.1] - 2022-05-22

### Added

- Endpoint for adding a harvest mint.

## [0.4.0] - 2022-05-22

### Added

- A method on `Farmer` model which calculates harvest in the current snapshot
  window, thereby allowing continuous harvest.
- A new dependency crate
  [`decimal`](https://gitlab.com/crypto_project/defi/decimal).
- A method on `Harvest` model which returns _tokens per slot_ configurable at
  a given slot.
- An `AmmError` variant `InvariantViolated` which is used for unreachable
  program paths. If everything works correctly, this variant should never be
  reached.

### Changed

- Renamed properties `harvest_mint` and `harvest_vault` on `Harvest` model to
  `mint` and `vault` respectively.
- Renamed `available_harvest` to `harvests` on `Farmer` model and changed the
  inner value from `MintHash` to a new model which tracks the mint pubkey and
  the amount available.

### Removed

- Model `MintHash` as we use pubkey instead.

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
