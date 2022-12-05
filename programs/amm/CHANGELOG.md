# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a
Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.1] - 20022-09-03

### Fixed

- On swap, we did not update the sell reserve's state with fee and therefore
  when a liquidity provider withdrew their funds, they would not receive the fees.
  The fees were essentially stuck in the vault.

## [2.0.0] - 20022-09-05

### Changed

- Deduplicated structs `DepositMintTokens` and `RedeemMintTokens` to `TokenLimit`.

## [1.0.2] - 20022-09-04

### Fixed

- A possible issue where the stable pool in a pathological state could scale
  down discriminant value to zero. We fail the swap for these pathological cases
  to prevent misuse via a flashloan attack.

## [1.0.1] - 20022-08-21

### Changed

- Depositing is now done in fewer compute steps.
- Finding an exponent is more performant as it no longers computes some
  unnecessary paths and the exponent was moved from decimal to binary.

## [1.0.0] - 20022-07-28

### Changed

- Reverting update of anchor back `0.24.2` and Solana to `1.9.18`. The new
  version is not stable enough yet.

## [0.6.1] 2022-07-13

### Changed

- Renamed function in swap equation.

## [0.6.0] - 20022-07-13

### Changed

- Upgraded anchor to `0.25.0` and Solana to `1.10.29`.

## [0.5.1] 2022-07-07

### Added

- Endpoints `deposit_liquidity`, `redeem_liquidity` and `swap` now print LP
  token supply at the end of the instruction.

## [0.5.0] 2022-07-08

### Changed

- The pool's property `fee` was renamed to `swap_fee`.

### Added

- Endpoint to perform a swap.
- Program's toll share is set to a third of the pool's fee.

## [0.4.8] 2022-08-08

### Fixed

- We now fail the operation if the user attempts to deposit so little liquidity
  that it cannot be represented by LP tokens.

## [0.4.7] 2022-08-06

- Added `try_mul_div` function to allow calculations involivng simultaneously
  multiplications and divisions to follow a computational path that decreases
  the risk of getting a MathOverflow.
- Added `find_exponent` function that finds the exponent of a number, which is
  a required value for the `try_mul_function`
- Changed `deposit_tokens` method to use `try_mul_div` in its computation to
  find the amount of tokens to deposit
- Changed `get_eligible_lp_tokens` method to use `try_mul_div` in its
  computation to find the amount of lp to distribute

## [0.4.6] 2022-07-07

### Changed

- Using large decimal number for stable curve invariant calculation and swap
  equations which has 9 decimal lances instead of 6.

## [0.4.5] 2022-07-07

### Fixed

- Review logic of approximation within the Newton-Raphson logic. Due to
  numerical instability and lack of precision, our approximations to root values
  don't zero out the stable swap polynomial (even though, it should be
  sufficiently close to 0, around 1e-4 magnitude precision).
  For that reason, we loose our checking and allow that a certain value `x` to
  be considered as a root in case of `SSP(x) < 1e-3`. ##[0.4.5] 2022-07-o7

- Refactor code to compute stable curve invariant. Our approach has several
  advantages, mainly due to the use of `Decimal` type instead of `LargeDecimal`.
  These include:
  1. Less memory usage;
  2. Higher reserve amounts allowed;
  3. No numerical instability due to decimal precision.

## [0.4.4] 2022-07-06

### Added

- Logic to compute the amount of tokens to obtain after a `swap`
  operation.

## [0.4.3] - 20022-07-04

### Added

- Added method `redeem_tokens` to struct `Pool` which contains the logic to
  alter the `Pool` state when reserve tokens are redeemed by liquidity
  providers.

## [0.4.2] - 20022-07-04

### Fixed

- Adding caching to stable curve invariant calculation to avoid recalculation.
- Since we use product of reserves and power, the stable curve invariant
  calculation failed very quickly, rendering the algorithm virtually unusable.
  We now use `LargeDecimal` as a workaround to the issue of math overflow.

## [0.4.1] - 20022-07-04

### Changed

- We error the pool creation if the provided LP mint supply is not zero but the
  vaults are empty. This implies that the admin has minted LP tokens before
  creating the pool, and therefore could get free liquidity if users deposited
  into the pool.
- We error the pool creation if the provided LP mint supply is zero but the
  vaults aren't empty. While there is no risk for the users in this scenario,
  having 0 supply should imply empty vaults, an invariant of the program which
  we want to preserve.

## [0.4.0] - 20022-07-04

### Removed

- Error variants `InvalidTokenVaultWalletSpecification` and
  `InvalidAccountOwner`, we use `err::acc` instead.

## [0.3.3] - 20022-07-01

### Added

- Implements `deposit-liquidity` endpoint logic.

### Changed

- Add `DepositMintTokens` structure to encapsulate tuples `(Publickey, TokenAmount)`.

## [0.3.2] - 2022-06-29

### Added

- Logic to perform Newton-Raphson method to compute new value
  of curve invariant, whenever one deposits/redeems liquidity
  on the given LP.

## [0.3.1] - 2022-06-29

### Added

- Logic which enables us to calculate the amount of tokens to deposit and LP
  tokens to mint.
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
