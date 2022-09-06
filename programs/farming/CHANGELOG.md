# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a
Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [5.1.1] - 20022-08-30

### Changed

- Upgraded to decimal crate `0.7.0` which contains a division fix.

## [5.1.0] - 20022-08-07

### Added

- Endpoint `airdrop` to increase a farmer's eligible harvest. Useful for
  migrations.

## [5.0.0] - 20022-07-28

### Added

- New account `payer` must be provided as a signer to `create_farmer` endpoint.

### Changed

- `authority` account in `create_farmer` endpoint doesn't have to be a signer.

## [4.0.0] - 20022-07-28

- Reverting update of anchor back `0.24.2` and Solana to `1.9.18`. The new
  version is not stable enough yet.

## [3.0.0] - 20022-07-12

### Changed

- Upgraded anchor to `0.25.0` and Solana to `1.10.29`.

## [2.1.1] - 20022-07-04

### Removed

- No-op call to `try_find_program_address` from compounding endpoint.

## [2.1.0] - 2022-06-20

### Changed

- Removing harvest only if the harvest vault is empty. This implies that all
  users have claimed their harvest.

## [2.0.0] - 2022-06-19

### Added

- New error variants related to adding a harvest.

### Changed

- When creating a new harvest period, the admin specifies the start and the
  length, rather than the start and the end slot.
- Admin can now schedule a new harvest period even if there's a harvest
  period in progress.

### Removed

- Error variant `InvalidSlot`

## [1.0.0] - 2022-06-17

### Changed

- Renamed program to `farming`.
- Renamed error to `FarmingError`.

## [0.8.0] - 2022-06-17

### Changed

- The tps history is now stored as "periods". Periods are non-overlapping and
  always bounded on the timeline, that is the admin _must_ specify how long
  does the farming last. Once started, farming emission rate cannot be changed.
- Endpoint `set_tokens_per_slot` was renamed to `new_harvest_period` and
  accepts new accounts: admin's harvest wallet, harvest vault and farm signer.
  The endpoint makes sure that there are enough tokens in the harvest vault to
  cover the whole harvest period.

### Removed

- `add_harvest` endpoint no longer accepts tps as parameter, one must always
  call `new_harvest_period` to change emission rate.

### Fixed

- Vesting period tokens were previously added to the staked tokens before
  calculation of the harvest, which meant that actually the vested tokens still
  earned tokens in the snapshot they were added at. Now, we split the
  calculation into 2 parts. First, we calculate with the current amount of
  staked tokens for the unfinished snapshot. Then we add the vested tokens to
  staked tokens and finish the calculation until the most recent slot.
- If a snapshot had staked amount 0, the first snapshot which didn't was
  counted as if it had started at that snapshot with staked amount 0.

## [0.7.7] - 2022-06-10

### Added

- Endpoint `compound_same_farm` which can be called by anyone (presumably a bot
  or a user) and transfers all harvest of mint A to the whitelisted stake vault
  of the same farm. This can only work if the farm's harvest mint matched
  the stake mint.
- Endpoint `compound_across_farm` which can be called by anyone (presumably a
  bot or a user) and transfers all of a farmer's harvest of mint A to the
  whitelisted stake vault of a different farm (target farm).This can only work
  for farms which have a harvest mint match the stake mint.
- Endpoint `whitelist_farm_for_compounding` that can be called by the admin of
  the source farm in order to whitelist the target farms that the source farm
  can send the harvest tokens to.
- Endpoint `dewhitelist_farm_for_compounding` that can be called by the
  admin of the source farm in order to remove a target farm from the whitelist.
- Method on `Farmer` model called `claim_harvest` that, for a given mint, it
  flushes out the eligble tokens from the `Farmer` struct and returns the
  token amount eligible.

## [0.7.6] - 2022-06-09

### Changed

- When we convert map to an array of `Farmer.harvests`, we pad the array with
  empty harvests (default pubkey and zero earn.) This was done in two places:
  `claim_eligible_harvest` endpoint and `fn update_eligible_harvest` on
  `Farmer`. This version deduplicates the logic into a single new method
  `set_harvests`.

## [0.7.5] - 2022-06-06

### Added

- Endpoint `close_farmer` which closes a `Farmer` account that has no more
  staked tokens or claimable harvests.

## [0.7.4] - 2022-06-06

### Added

- Endpoint `claim_eligible_harvest` which reads pairs of (vault, wallet) from
  remaining accounts and checks their mint. The mint if used to retrieve amount
  of tokens eligible to claim by the `Farmer`. After that specific amount is
  transferred, we set the `Farmer`'s harvest of that mint to zero.

## [0.7.3] - 2022-06-06

### Added

- Endpoint `update_eligible_harvest` which moves funds from vested to staked
  and calculates harvest since last update. This is also what happens in
  `start_farming` and `stop_farming`.
- Wrapped `add_to_vested` and `update_eligible_harvest` methods on `Farmer`
  to `check_vested_period_and_update_harvest`.
- Changed logic for slot calculation in `update_eligible_harvest_in_open_window`
  to account for missing slot. The slots were being calculated by subtracting
  `current_slot` by `calculate_next_harvest_from.slot`, however since we want
  to include the current slot in the calculation we change it it by adding 1
  to the equation, therefore making it `current_slot + 1` subtracted
  by `calculate_next_harvest_from.slot`.

## [0.7.2] - 2022-06-06

### Added

- Endpoint `stop_farming` which _(i)_ updates eligible harvest; _(ii)_ removes
  user's tokens in `Farmer` from either vested or staked funds, or both;
  _(iii)_ transfers that amount of tokens requested to unstake from the
  `Farm`'s stake vault to user's wallet.

## [0.7.1] - 2022-06-06

### Added

- Endpoint `start_farming` which _(i)_ updates eligible harvest; _(ii)_ stakes
  user's tokens into a `Farmer`'s vested tokens and _(iii)_ transfers
  that amount of tokens to the `Farm`'s stake vault.

## [0.7.0] - 2022-06-03

### Added

- A method on `Farmer` model which calculates harvest in the snapshots history
  (excluding the open window).
- A method on `Farmer` model which calls both methods to calculate harvest
  in snapshot and in open window.
- A method on `Farm` model which returns an iterator with the eligible snapshots
  for harvest in the snapshot ring buffer
- Renamed field `farmer_harvest_calculated_until` to `calculate_next_harvest_from`
  in struct `Farm`.

## [0.6.1] - 2022-06-01

### Added

- Endpoint `create_farmer` logic implemented.

## [0.6.0] - 2022-06-01

### Removed

- Endpoint `claim_eligible_harvest` no longer takes bump seed as an argument.
- Endpoint `stop_farming` no longer takes bump seed as an argument.
- Endpoint `create_farm` no longer takes bump seed as an argument.
- Endpoint `remove_harvest` no longer takes bump seed as an argument.
- Endpoint `add_harvest` no longer takes bump seed as an argument.

## [0.5.3] - 2022-05-31

### Added

- Endpoint accounts structures for `create_farmer`, `close_farmer`,
  `start_farming`, `stop_farming`, `update_eligible_harvest` and
  `claim_eligible_harvest`. The endpoints are currently no-ops, logic shall be
  added in upcoming MRs. The accounts structures enabled frontend to begin
  their implementation earlier.

## [0.5.2] - 2022-05-30

### Added

- Endpoint for set tokens per slot configuration parameter.
- Method on `Farm` model called `set_tokens_per_slot` where the
  core logic of the `set_tokens_per_slot` endpoint resides. It
  allows admin of the farm to be able to change the number of
  distributed tokens per slot, at most a fixed finite number
  of times
- Added method `oldest_snapshot` for `Farm`.

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
