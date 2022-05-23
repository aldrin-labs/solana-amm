- Solana v1.9.18
- Anchor v0.24.2
- [Code coverage][project-code-coverage]
- [Rust docs][project-rust-docs]
- [Changelog][project-changelog]

# AMM

TODO: https://gitlab.com/crypto_project/defi/amm/-/issues/13

## Equations

Search for `ref. eq. (x)` to find an equation _x_ in the codebase.

| Symbol  | Description                        |
| ------- | ---------------------------------- |
| $`s`$   | current slot                       |
| $`F_u`$ | slot of farmer's last harvest      |
| $`F_s`$ | farmer's staked amount             |
| $`v`$   | total staked amount in farm        |
| $`ρ`$   | farm's emission of tokens per slot |

⌐

To calculate farmer's eligible harvest for elapsed period of time:

```math
(s - F_u) ρ \dfrac{F_s}{v}
\tag{1}
```

⌙

<!-- List of References -->

[token-program-mint]: https://docs.rs/anchor-spl/0.24.2/anchor_spl/token/struct.Mint.html
[project-code-coverage]: https://crypto_project.gitlab.io/defi/amm/coverage
[project-rust-docs]: https://crypto_project.gitlab.io/defi/amm/amm
[project-changelog]: https://crypto_project.gitlab.io/defi/amm/amm.changelog.html
