# Motivation

To create a user incentive for token possession, we distribute time dependent
rewards. A user stakes tokens of a mint `S`, ie. they lock them with the
program, and they become eligible for a harvest mint `H`. The distribution is
advertised in units _N harvest tokens per $1000 staked_.

![Example of how staking is advertised][example-staking-units]

`S` can equal `H` as is the case with [`RIN` staking][rin-staking]. That is, a
user stakes their `RIN` tokens and get their harvest in `RIN`.

The typical use-case is for `S` to be an LP token of an AMM pool. Users deposit
their liquidity and collect fees, and on top of that Aldrin competes with other
AMM providers by offering rewards.

What follows is a second iteration on the farming program design. The first
version has several major flaws such as limited farming period; necessity for
users to re-do their position every fortnight; claiming harvest required many
instructions; farmers couldn't claim harvest continuously; harvest calculation
required several accounts and was complicated which led to hard to understand
bugs; and more.

# Glossary

A **farmer** is a user who staked their tokens with the program.

A **harvest mint** is a token program's [`Mint`][token-program-mint], and
tokens of the harvest mint, called **harvest**, are distributed to farmers as
rewards for their staking.

A **snapshot** is a point in time recorded in history, a **snapshot window** is
a period of time between two consecutive snapshots.

# Approach

All staked tokens are stored in a program's vault (token account.) The amount
`v` of a vault `V` represents how many tokens are staked in total.

The harvest is distributed using a configurable _tokens per slot_ (`ρ`.) This
value represents how many tokens should be divided between all farmers per slot
(~0.4s.)

To distribute harvest fairly, we calculate a ratio `r` between a farmer's share
of `v` (ie. how many tokens did the farmer stake) and `v`. `r` then scales `ρ`
down to the farmer's share of it.

---

_[Problem 1]_ We cannot use the latest state of `v` for this calculation.
Assume we did and reason through following scenario:

- We set up a farm which distributes 10k tokens over one month.
- A farmer who posses $1m creates two deposits at the beginning of the period,
  both for $500k.
- Say that there are $2m in the pool in total, including these two deposits.
  Assume the other $1m is pretty stable.
- The farmer then waits the whole period and by the end of it they are eligible
  for half of the rewards, i.e. 5k R. That's because they own 1m/2m = 0.5 of
  the deposits.
- They redeem the first deposit ($500k) and get 2.5k tokens, because 500k/2m =
  0.25. But then they withdraw this liquidity. Now there's only $1.5m in the
  pool.
- They redeem the second deposit, 500k/1.5m = 1/3. If our algorithm
  distribution is based on immediate `v`, they'd withdraw 10k \* 1/3 ~= 3333. So
  even though their deposit should have been eligible for 5k, they ended up
  with ~5.833k.

---

With each `V` we associate a unique snapshot ring buffer. Periodically, a bot
invokes `take_snapshot` endpoint which writes the latest state of `v` to the
buffer. This endpoint is permission-less, but it asserts that some minimum
amount of time has passed. The buffer is stored on an account `Farm` which is
introduced later. While it could be split into another account, we prefer to
minimise the number of accounts we use and the buffer is required in all
endpoints. Since the frontend will use cached values served by backend, we
don't need to consider the `Farm` size.

---

_[Problem 2]_ The snapshot ring buffer does a full rotation once every few
months. Therefore, the available history is limited.

---

There is an endpoint `calculate_available_harvest` which can be called by
anyone for any farmer's account whose last harvest was older than some period
of time `c` (which is less than the full rotation period). That is, even if a
farmer doesn't interact with the program, automation ensures that their share
over each unclaimed snapshot is preserved before history is erased.

The endpoint increments _available harvest_ token counter on the farmer's
account. Should for some reason automation fail for months on end, and the user
wouldn't perform this operation manually either, then we need to have an
edge-case condition: burn all harvest until oldest buffer entry. Farmers won't
have to re-stake ever, farming can run ad infinitum. _(TBD: Potentially, we can
give the caller a small reward for their effort from user's stash, thereby not
only Aldrin bots will have an incentive to do this.)_

To summarize, the `calculate_available_harvest` endpoint can be called _only_
by the farmer if the last harvest was done earlier than `c`, and by anyone
otherwise.

![Schematics of the snapshot buffer][snapshot-buffer]

The account which defines `ρ` is called a `Farm` mentioned above account (`F`.)
`F` is in a one-to-one relationship with `V`. An admin might want to distribute
multiple harvest mints for a single `V`. Eg. we may distribute `RIN` and `SOL`
for `USDC/ETH` pool farming. The `F` account has an array property `harvests`
which is limited to `Ψ` entries. An entry represents a single harvest mint
along with its configuration (such as `ρ`.) See below for more information
about `Ψ`.

---

_[Problem 3]_ The value of `ρ` can be changed by the admin. Therefore, two
farmers whose positions are identical see different harvest if one claims just
before and the other just after a change in `ρ`. On top of that, there's a
different `ρ` for each `F`, but only one snapshots ring buffer.

---

We store not only the total amount of staked tokens, but also history of `ρ`
for each harvest mint. However, this settings won't change that often. It
would needlessly take too much space if we stored it for each snapshot.
Instead, on the `F`, we store only changes to this value. These changes are
stored in a matrix, because there are different `ρ` values for different
harvest mints. Eg. going with the example above, `RIN` might be emitted at
different rate to `SOL`, they both have different `ρ`. This imposes a limit `Ψ`
mentioned above on how many harvest mints can be associated with a `V`. We opt
for a value based on a judgement call with the tokenomics team. In the old
program, this value was 10. While a design with unlimited number of harvest
mints would be possible, it would require many accounts and out goal is to
optimize for transaction size.

---

_[Problem 4]_ The ring buffer system allows us to fairly distribute harvest
until the last snapshot slot. However, we would like to enable continuous
harvest. A farmer should be able to harvest at any point in time all the tokens
they are eligible for, not only all the tokens they are eligible for until the
last snapshot slot.

A UX complaint on the old farming program was its inability to distribute
harvest continuously. Farmers had to wait until the a snapshot was taken.

---

The claim logic is split into two parts. First part, as described above, uses
the snapshot buffer. The second part calculates harvest _since_ the last
snapshot slot, ie. in the open snapshot window.

Along with `V` (stake vault), there is a second vault `V'` (vesting vault). `V`
is being periodically snapshoted. `V'` is the target of deposit of any newly
staked tokens. When a snapshot is taken, we transfer all funds from `V'` to `V`
and thereby create a constant _total volume_ for the upcoming snapshot window.
That allows us to calculate a predictable share for each user, because all
claims are going to be divided by the same total. We can safely ignore
withdrawals, because they don't overshoot our expectations in terms of harvest
claimed. This mechanism guarantees that in each snapshot window we distribute
at most `l * ρ` tokens (where `l` is snapshot window length in slots.)

An issue is that farmers aren't eligible for harvest at all for some period of
time, more specifically until the current window ends. We call this the vesting
period. If the snapshot window is made sufficiently small, the vesting period
during which a user is not eligible for harvest is negligible.

---

_[Problem 5]_ Mutating farmer's stake total projects into the past. Consider
following:

- A farmer deposits 1 `USDC` which finishes its vesting period in `w0` (window
  0), ie. they are eligible for harvest from `w0`.
- The total deposited amount for `w0` is 10 `USDC`. The total harvest for `w0`
  is 100 `RIN`.
- The farmer is eligible for 1/10th of the harvest, 10 RIN.
- However, they wait. During `w3` they stake another 4 `USDC`. Now their total
  staked amount from `w4` onwards is 5 `USDC`.
- We don't have the information that those 4 `USDC` should not be counted
  towards the harvest in `w0`.
- They claim their harvest in `w4`. The program sees that they have staked 5
  `USDC` and that the total deposited amount for `w0` was 10 `USDC`. They get
  50 `RIN` instead of 10 `RIN`.

---

Every time a farmer starts of stops farming (deposits or withdraws stake
tokens) we calculate their harvest until the current slot. Mutating the total
staked tokens must always be preceded with setting the _harvest claim at_ (last
harvest slot) to the current one.

Back to the farm accounts. With each `F` we associate:

- an _admin_ signer who is allowed to change settings and such;

- a _stake mint `S`_. Created e.g. in the core part of the AMM logic and here
  serves as a natural boundary between the two features: _(1)_ depositing
  liquidity and swapping; _(2)_ farming with which this document is concerned;

- a _stake vault `V`_;

- a _vesting vault `V'`_;

- a _snapshot ring buffer_.

- an array of _harvests_. For each harvest we store the history of `ρ` and a
  _harvest vault_ from which the harvest tokens are transferred to farmers;

The admin maintains distribution by modulating _tokens per slot_ settings and
by depositing more tokens into the _harvest vault_. Anyone can become an admin,
that is anyone can create their own farms.

A farmer has only a single account `R` for farming per `V`, ie. `R` is in a
many-to-one relationship to `F`. This minimizes the number of accounts one
needs to provide to transactions and therefore enables single transaction
claim. This account tracks everything related to the farmer's stake. With each
`R` we:

- associate the _authority_, ie. the signer who can claim and stop farming;

- which _farm_ account is the farmer associated with;

- store how many tokens did the farmer _stake_;

- how many _tokens_ are currently _in the vesting period_, ie. not eligible for
  harvest until next snapshot window;

- store the last slot that the farmer calculated harvest until, therefore
  _harvest calculated until_;

- store how many tokens is the farmer eligible for **excluding** harvest since
  the _last harvest_ slot. This is going to be used mainly for the logic above
  which mentions how `calculate_available_harvest` is invoked by bots. This
  endpoint increments relevant available harvest integer. Since there are
  multiple harvestable mints, this must be an array of `(Mint, Amount)` tuples.
  The mint can be a hash or pubkey. The mint tells us for which token mint does
  the associated integer, _available harvest_ amount, apply. The length of this
  array is given by `Ψ` mentioned above.

## List of projected endpoints

- `create_farm`
- `take_snapshot`
- `set_farm_owner`
- `add_harvest`
- `remove_harvest`
- `set_harvest_tokens_per_slot`

- `create_farmer`
- `start_farming`
- `stop_farming`
- `calculate_available_harvest`
- `claim_available_harvest`
- `calculate_and_claim_available_harvest`
- `close_farmer`

<!-- List of References -->

[rin-staking]: https://dex.aldrin.com/staking
[example-staking-units]: docs/images/staking_units.png
[snapshot-buffer]: docs/images/snapshot_buffer.png
[token-program-mint]: https://docs.rs/anchor-spl/0.24.2/anchor_spl/token/struct.Mint.html
