- Solana v1.9.18
- Anchor v0.24.2
- [Code coverage][project-code-coverage]

# AMM

- [Rust docs][amm-rust-docs]
- [Changelog][amm-changelog]

# Farming program (Fp)

- [Rust docs][fp-rust-docs]
- [Changelog][fp-changelog]
- [`DFarMhaRkdYqhK5jZsexMftaJuWHrY7VzAfkXx5ZmxqZ` is dev program][fp-dev-solscan]

## Motivation

To create a user incentive for token possession, we distribute time dependent
rewards. A user stakes tokens of a mint `S`, ie. they deposit them with the
program, and they become eligible for a harvest mint `H`. The distribution is
advertised in units _N harvest tokens per $1000 value staked_.

![Example of how staking is advertised][example-staking-units]

`S` can equal `H` as is the case with [`RIN` staking][rin-staking]. That is, a
user stakes their `RIN` tokens and get their harvest in `RIN`.

The typical use-case is for `S` to be an LP token of an AMM pool. Users deposit
their liquidity and collect fees, and on top of that Aldrin competes with other
AMM providers by offering rewards.

## Glossary

A **farmer** is a user who staked their tokens with the farming program.

A **harvest mint** is a token program's [`Mint`][token-program-mint], and
tokens of the harvest mint, called **harvest**, are distributed to farmers as
rewards for their staking.

A **snapshot** is a point in time recorded in history, a **snapshot window** is
a period of time between two consecutive snapshots.

## Requirements

This is a second Aldrin's iteration on farming and staking logic. We learnt
from our past design and identified several inconveniences of the previous
version which became a focus in the new version.

1. The farming duration was limited to several weeks after which the admin had
   to re-create a farm. The new design must enable long running farms, admin
   should only vary setting as they need to and top-up harvest.
2. Farmers had to claim their funds and re-stake every few weeks due to the
   first point. The new design must enable a farmer to stake their funds and
   not touch them for an arbitrary period of time without losing any harvest.
3. To claim harvest, the FE had to build a complicated series of several
   transactions across different farms due to the first point. The new design
   must simplify FE harvest claim into a single transaction with a single
   global farm.
4. Farmers had to wait after they claimed their harvest. The new design must
   enable continuous emissions.
5. Harvest calculation was involved and poorly documented. The new design must
   simplify emission logic.

## Design

The first decision to make is whether to associate a single `S` with multiple
`H`s uniquely, or whether to have each `S` and each `H` as separate accounts
and join them via a third data type. To better illustrate the distinction,
let's translate it into an analogical SQL database layout:

```
table: farms
rows: id; settings; harvest1; harvest2; ..; harvestn

---- vs ----

table: farms
rows: id; settings

table: harvests
rows: ...

table: farm_harvest
rows: farm_id; harvest_id
```

While the latter offers greater flexibility, we opt for the former due to its
simplicity. It only takes a single account to represent the whole farm. While
we might use the latter in future for more advanced staking strategies, we have
a complete idea about what we require of our LP token farming and a single
token staking. These are a huge enough use-case that it warrants a simple
dedicated logic in the farming program.

The next decision is about configuration which determines the emission rate.
The harvest is distributed using a configurable _tokens per slot_ (`ρ` or
_tps_.) This value represents how many tokens should be divided between all
farmers per slot (~0.4s.)

### `Farm`

is an account under an admin's authority which represents emission setup.

There's one stake vault (token account) `V` per `Farm`. All users's staked
tokens are stored in `V`. The amount `v` represents how many tokens are staked
in total. The vault is under an authority of farm's signer PDA.

To ensure uniqueness, `V` has a seed of `["stake_vault", farmPubkey]` and the
farm's signer PDA which has authority over it has a seed of
`["signer", farmPubkey]`.

To distribute harvest fairly, we calculate a ratio `r` between a farmer's share
of `v` (ie. how many tokens did the farmer stake) and `v`. `r` then scales `ρ`
down to the farmer's share of it. See [eq. (1)](#equations).

The configuration value `ρ` is stored on `Farm`. An admin might want to
distribute multiple harvest mints. Eg. we may distribute `RIN` and `SOL` for
`USDC/ETH` pool farming. `Farm` must therefore have an array property
`harvests` which is limited to `Ψ` entries. An entry in `harvests` represents
a single harvest mint. To enhance the admin’s control over the farm, for each
given harvest mint, the program allows the admin to set up finite
non-overlapping harvest periods, up to a limit of 10, where each period has
its own `ρ`. In relation to the limit `Ψ` of harvests mints, we opt for a
value based on a judgement call with the tokenomics team. In the old program,
this value was 10. While a design with unlimited number of harvest mints would
be possible, it would require many accounts and out goal is to optimize for
transaction size.

<details>
<summary markdown="span">
[PROBLEM no.1]
</summary>

The value of `ρ` can be changed by the admin. Therefore, two farmers whose
positions are identical see different harvest if one claims just before and the
other just after a change in `ρ`.

</details>

For each harvest mint in a given farm, we store on `Farm` the farming periods,
each with its own `ρ`. Whenever the admin wants to change `p` he/she will have
to create a new farming period.

These changes are stored in a matrix, because there are
different `ρ` values for different harvest mints, for different periods of
time. Eg. going with the example above, `RIN` might be emitted at different
rate to `SOL`, they both have different `ρ`. An example of this matrix:

```
----+----------------------------+----------------------------+-----
SOL | value 10; slot 21; slot 39 | value 25; slot 20; slot 5  | ...
RIN | value 12; slot 31; slot 50 | value 80; slot 30; slot 10 | ...
...
```

In each period, represented by element of the matrix in a given row, the
starting point corresponds to the first slot and the ending point corresponds
to the second slot in the tupple.
We order each row in the matrix in descending order of periods. Ie. when an
admin changes adds a new period, we shift the array to right and insert the new
value to index 0.
The number of changes to this value is limited by the length of a row in the
matrix. This is a hard coded value in the code base.

<details>
<summary markdown="span">
[PROBLEM no.2]
</summary>

We cannot use the latest state of `v` for this calculation. Assume we did and
reason through following scenario:

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
  distribution is based on immediate `v`, they'd withdraw 10k \* 1/3 ~= 3333.
  So even though their deposit should have been eligible for 5k, they ended up
  with ~5.833k.

</details>

With each `Farm` we associate a unique snapshot ring buffer. Periodically, a
bot invokes [`take_snapshot`](#endpoints) endpoint which writes the latest
state of `v` to the buffer. This endpoint is permission-less, but it asserts
that some minimum amount of time has passed.

![How slots divide snapshots][snapshot-history]

Another decision is on whether we store the snapshots ring buffer on `Farm`
account or as a separate account. While it could be split into another account,
we prefer to minimize the number of accounts we use and the buffer is required
in all endpoints. Since FE will use cached values served BE, we don't need to
consider the `Farm` byte size - it won't be fetched by RPC.

> See [equation (1)](#equations) for legend to following figures.

![Schematics of the snapshot buffer][snapshot-buffer]
![Calculating eligible harvest in past snapshots][snapshot-closed-calc]

<details>
<summary markdown="span">
[PROBLEM no.3]
</summary>

Admin wants to change `ρ`, but to calculate harvest for users we must remember
`ρ` for every snapshot.

</details>

The history of changes to `ρ` is limited by the limited amount of harvest
periods. We store the periods, and consequently `p`, in a queue from which we
pop oldest period. With each change we remember when did the admin trigger it.
The harvests periods do not have to match the snapshots at their start nor at
their end. The eligible harvest in a given snapshot can be processes by the
program even if there are multiple harvest periods within it, with distinct
`p` values.

Whenever we encouter ourselves in a harvesting period, the `p` of such period
cannot be altered, only the `p` of harvest periods which have not yet started.

To summarize, a `Farm` account contains data about:

- an _admin_ signer who is allowed to change settings and such;

- a _stake mint_. Created e.g. in the core part of the AMM logic and here
  serves as a natural boundary between the two features: _(1)_ depositing
  liquidity and swapping; _(2)_ farming with which this document is concerned;

- a _stake vault_;

- a _snapshot ring buffer_;

- an array of _harvests_. For each harvest we store the harvest _periods_, each
  with its own `ρ`, the _harvest mint_ and a
  _harvest vault_ from which the harvest tokens are transferred to farmers.

### `Farmer`

is an account under a user's authority which tracks their stake and harvest.

<details>
<summary markdown="span">
[PROBLEM no.4]
</summary>

The snapshot ring buffer does a full rotation once every few weeks. Therefore,
the available history is limited.

</details>

There is an endpoint [`update_eligible_harvest`](#endpoints) which can be
called by anyone for any `Farmer`. That is, even if a farmer doesn't interact
with the program, automation ensures that their share over each unclaimed
snapshot is preserved before history is erased.

The endpoint increments _available harvest_ token counter on `Farmer`. Should
for some reason automation fail for weeks on end, and the user wouldn't perform
this operation manually either, then we need to have an edge-case condition:
burn all harvest until oldest buffer entry. Farmers won't have to re-stake
ever, farming can run ad infinitum, however farmers will only accumulate
harvest throughout the timespan of available harvest periods.

<details>
<summary markdown="span">
[PROBLEM no.5]
</summary>

The ring buffer system allows us to fairly distribute harvest until the last
snapshot slot. However, we would like to enable continuous harvest. A farmer
should be able to harvest at any point in time all the tokens they are eligible
for, not only all the tokens they are eligible for until the last snapshot
slot.

A UX complaint on the old farming program was its inability to distribute
harvest continuously. Farmers had to wait until the a snapshot was taken.

</details>

The claim logic is split into two parts. First part, as described above, uses
the snapshot buffer. The second part calculates harvest _since_ the last
snapshot slot, ie. in the open snapshot window. We use the last snapshot total
staked amount as the _total volume_ for the upcoming snapshot window. That
allows us to calculate a predictable share for each user, because all claims
are going to be divided by the same total. We can safely ignore withdrawals,
because they don't overshoot our expectations in terms of harvest claimed. This
mechanism guarantees that in each snapshot window we distribute at most `l * ρ`
tokens (where `l` is snapshot window length in slots.)

> See [equation (2)](#equations) for legend to following figure.

![Calculating eligible harvest in open snapshot][snapshot-open-calc]

An outcome of this design is that a farmer isn't eligible for harvest at all
for some period of time, more specifically until the current window ends. We
call this the vesting period.

<details>
<summary markdown="span">
[PROBLEM no.6]
</summary>

Mutating farmer's stake total projects into the past. Consider following:

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

</details>

Every time a farmer starts or stops farming (deposits or withdraws stake
tokens) we calculate their harvest until the current slot. Mutating the total
staked tokens must always be preceded with setting the _harvest claim at_ (last
harvest slot) to the current one.

To summarize, a `Farmer` account contains data about:

- associate the _authority_, ie. the signer who can claim and stop farming;

- which _farm_ account is the farmer associated with;

- store how many tokens did the farmer _stake_;

- how many _tokens_ are currently _in the vesting period_, ie. not eligible for
  harvest until next snapshot window;

- store the next slot that the farmer should calculate their harvest from;

- store how many tokens is the farmer eligible for **excluding** harvest since
  the _last harvest_ slot. This is going to be used mainly for the logic above
  which mentions how `calculate_available_harvest` is invoked by bots. This
  endpoint increments relevant available harvest integer. Since there are
  multiple harvestable mints, this must be an array of `(Mint, Amount)` tuples.
  The mint can be a hash or pubkey. The mint tells us for which token mint does
  the associated integer, _available harvest_ amount, apply. The length of this
  array is given by `Ψ` mentioned above.

### Compounding

We want to automate claiming harvest and re-staking it for the farmers. For
example, stakers in the `RIN` farm (`RIN` stake mint, `RIN` reward mint)
shouldn't have to revisit the UI to claim and stake again. Or stakers in the
`USDC/SOL` farm who earn `RIN` harvest should be able to get their harvest
automatically staked in the `RIN` farm. The former is called "compounding in
the same farm", the latter "compounding across farms." There are endpoints for
both actions. These endpoints are permission-less. This enables our automation
to periodically execute them.

<details>
<summary markdown="span">
[PROBLEM no.7]
</summary>

Anyone can create a new farm with the relevant staking mint and set up their
own automation which would funnel funds from all farms into their own.

</details>

The admin of a farm must whitelist the pubkey of each farm for which the
compounding should be enabled. This is done by using endpoint
**`whitelist_farm_for_compounding`** which creates a new PDA account. The seed
of this PDA is the source farm's pubkey and the target's farm pubkey. For
compounding in the same farm, the two pubkeys are the same. The compounding
endpoints then assert the existence of this account before proceeding.

### Endpoints

A pubkey becomes an admin upon calling **`create_farm`** endpoint. In this
endpoint, the admin defines what is the mint of the tokens which the farmers
will stake.

The admin can then add new mints which will be released to the farmers as
harvest with **`add_harvest`** endpoint. In this endpoint, the admin defines
the mint.

To start farming a particular harvest mint, the admin calls
**`new_harvest_period`** endpoint. This endpoint takes as an input the harvest
mint, the slot from which the harvest will be eligible for claiming, how many
slots does the period last and the emission rate `ρ`. If the start at slot is
zero, the program will use the current slot as the beginning of the harvest
period. There can be at most one period open at a time. However, the admin can
schedule one period in future. When the admin calls this endpoint, they must
also provide their harvest token wallet. We calculate the total amount of
harvest tokens that will be released to the farmers in this period with
`period length * ρ` and transfer this amount to the harvest vault.

There is a limit on how many harvest mints can be added to the farm. The admin
can call **`remove_harvest`** endpoint to remove a harvest mint if the harvest
vault is empty. This implies that all users have claimed their harvest and they
won't lose out.

The admin can transfer ownership of a farm with **`set_farm_owner`**.

Periodically, the permission-less endpoint **`take_snapshot`** must be called
to record history of the farm's stake vault.

---

A pubkey becomes a farmer upon calling the **`create_farmer`** endpoint. This
creates `Farmer` account which is a PDA with the farm's pubkey and user's
pubkey as a seed.

To stake tokens, the farmer calls **`start_farming`** endpoint. This endpoint
takes as an input the amount of tokens to stake. The tokens undergo a vesting
period which ends when a new snapshot is taken.

To withdraw their staked tokens, the farmer calls **`stop_farming`** endpoint.
This endpoint takes as an input the amount of tokens to withdraw. The tokens
are transferred to the farmer's wallet.

To update farmer's harvest, the permission-less endpoint
**`update_eligible_harvest`** can be called by anyone. When called, the history
of the farm is used to calculate eligible harvest.

To transfer the accrued harvest to date to farmer's wallet, they must call
**`claim_eligible_harvest`**. This endpoint has a more complex API: it takes a
list of remaining accounts where each pair is the harvest vault from which to
transfer, and the wallet into which to transfer.

If the farmer wants to stop their interaction with the farm and reclaim their
tokens, then can call **`close_farmer`** endpoint.

# Equations

Search for `ref. eq. (x)` to find an equation _x_ in the codebase.

| Symbol   | Description                                 |
| -------- | ------------------------------------------- |
| $`c`$    | current slot                                |
| $`F_u`$  | slot of farmer's last harvest               |
| $`F_s`$  | farmer's staked amount                      |
| $`v_w`$  | total staked amount in farm at snapshot _w_ |
| $`s_w`$  | when was snapshot _w_ taken                 |
| $`ρ_w`$  | farm's tps for snapshot _w_                 |
| $`w(t)`$ | snapshot at slot _t_                        |
| $`p`$    | latest snapshot                             |

⌐

To calculate farmer's eligible harvest in the open window, ie. continuous
harvest:

```math
( c - \max{(F_u, s_p)} + 1 )  ρ_p  \dfrac{F_s}{v_p}
\tag{1}
```

⊢

To calculate farmer's eligible harvest in the closed windows, ie. using the
snapshot ring buffer history:

```math
\sum_{j=w(F_u)}^{p-1} ( s_{j+1} - \max{(F_u, s_j)} )  ρ_j  \dfrac{F_s}{v_j}
\tag{2}
```

⌙

<!-- List of References -->

[token-program-mint]: https://docs.rs/anchor-spl/0.24.2/anchor_spl/token/struct.Mint.html
[project-code-coverage]: https://crypto_project.gitlab.io/defi/amm/coverage
[fp-rust-docs]: https://crypto_project.gitlab.io/defi/amm/farming
[fp-changelog]: https://crypto_project.gitlab.io/defi/amm/fp.changelog.html
[amm-rust-docs]: https://crypto_project.gitlab.io/defi/amm/amm
[amm-changelog]: https://crypto_project.gitlab.io/defi/amm/amm.changelog.html
[rin-staking]: https://dex.aldrin.com/staking
[example-staking-units]: docs/images/staking_units.png
[snapshot-buffer]: docs/images/snapshot_buffer.png
[token-program-mint]: https://docs.rs/anchor-spl/0.24.2/anchor_spl/token/struct.Mint.html
[snapshot-closed-calc]: docs/images/harvest_calc_past_snapshots.png
[snapshot-open-calc]: docs/images/harvest_calc_open_snapshot.png
[snapshot-history]: docs/images/history.png
[fp-dev-solscan]: https://solscan.io/account/DFarMhaRkdYqhK5jZsexMftaJuWHrY7VzAfkXx5ZmxqZ
