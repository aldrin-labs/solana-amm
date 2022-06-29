# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a
Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] - 2022-06-29

### Added

- Endpoint `set_pool_swap_fee`

## [0.3.0] - 2022-06-29

### Changed

- Renamed model `Fraction` to `Permillion`.

### Added

- Error variant `InvalidArg`
- Model `Discount` which is in a one-to-one relationship with a user (ie.
  a pubkey) and defines user's discount on swap.
- Endpoint `put_discount` which creates or updates user's discount. This
  endpoint is permissioned and can only be called by a signer with a pubkey
  defined in the `DiscountSettings` model.

## [0.2.1] - 2022-06-28

### Added

- `DiscountsSettings` model which is initialized by the program upgrade
  authority. It determines who's the authority that receives a cut on swap fees.
- `create_discount_settings` endpoint which inits the aforementioned model.

## [0.2.0] - 2022-06-27

### Changed

- `Pool`'s property `initializer` renamed to `admin`.
- `Pool`'s property `lp_token_program_fee_wallet` renamed to
  `program_toll_wallet`.
- `Pool`'s property `lp_token_mint` renamed to `mint`.

### Added

- `ProgramToll` model which is initialized by the program;s upgrade authority
  and determines who's the authority that receives a cut on swap fees.
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
