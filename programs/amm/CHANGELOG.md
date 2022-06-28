# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a
Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2022-06-27

### Changed

- `Pool`'s property `initializer` renamed to `admin`.
- `Pool`'s property `lp_token_program_fee_wallet` renamed to
  `program_toll_wallet`.
- `Pool`'s property `lp_token_mint` renamed to `mint`.

### Added

- `ProgramToll` model which is initialized by the admin and determines who's
  the authority that receives a cut on swap fees.
- `create_program_toll` endpoint which inits the aforementioned model.
- "dev" feature which conditionally compiles logic used only in the tests or
  dev version of the program.
- `create_pool` endpoint which initializes a new pool. This endpoint is generic
  and can be used to create both constant product and stable curve, and
  multi-asset pools.

## [0.1.0] - 2022-06-22

### Added

- Pool account model
- Serializable decimal model
