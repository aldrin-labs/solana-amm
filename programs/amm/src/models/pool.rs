//! TODO: docs

use crate::prelude::*;
use std::collections::BTreeMap;
use std::mem;

#[derive(Default, Debug)]
#[account]
pub struct Pool {
    pub admin: Pubkey,
    pub signer: Pubkey,
    pub mint: Pubkey,
    pub program_toll_wallet: Pubkey,
    pub dimension: u64,
    /// The pool as a maximum reserve size of 4 and can have less reserves
    /// than that. If the pool only has 2 token reserves then, then first two
    /// elements of this array represent those reserves and the other two
    /// elements should have the default value.
    ///
    /// TODO: find out whether we can make this private
    pub reserves: [Reserve; 4],
    pub curve: Curve,
    pub fee: Permillion,
}

#[derive(
    AnchorDeserialize, AnchorSerialize, Copy, Clone, Debug, Eq, PartialEq,
)]
pub enum Curve {
    ConstProd,
    /// TODO: Think of a better name for `invariant`
    Stable {
        amplifier: u64,
        invariant: SDecimal,
    },
}

#[derive(
    AnchorDeserialize,
    AnchorSerialize,
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Default,
)]
pub struct Reserve {
    pub tokens: TokenAmount,
    pub mint: Pubkey,
    pub vault: Pubkey,
}

#[derive(
    AnchorDeserialize,
    AnchorSerialize,
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
)]
pub struct DepositMintTokens {
    pub mint: Pubkey,
    pub tokens: TokenAmount,
}

#[derive(
    AnchorDeserialize,
    AnchorSerialize,
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
)]
pub struct RedeemMintTokens {
    pub mint: Pubkey,
    pub tokens: TokenAmount,
}

#[derive(Debug, Eq, PartialEq, Default)]
pub struct DepositResult {
    pub lp_tokens_to_distribute: TokenAmount,
    pub tokens_to_deposit: BTreeMap<Pubkey, TokenAmount>,
}

#[derive(Debug, Eq, PartialEq, Default)]
pub struct RedeemResult {
    pub lp_tokens_to_burn: TokenAmount,
    pub tokens_to_redeem: BTreeMap<Pubkey, TokenAmount>,
}

impl Default for Curve {
    fn default() -> Self {
        Curve::ConstProd
    }
}

impl Curve {
    pub fn invariant(&self) -> Option<Decimal> {
        match self {
            Curve::ConstProd => None,
            Curve::Stable { invariant, .. } => Some(Decimal::from(*invariant)),
        }
    }
}

impl Pool {
    pub const SIGNER_PDA_PREFIX: &'static [u8; 6] = b"signer";

    pub fn space() -> usize {
        let discriminant = 8;
        let initializer = 32;
        let signer = 32;
        let lp_token_program_fee_wallet = 32;
        let mint = 32;
        let dimension = 8;
        let reserves = mem::size_of::<Reserve>() * 4;
        let curve = mem::size_of::<Curve>();
        let fee = mem::size_of::<Permillion>();

        discriminant
            + initializer
            + signer
            + lp_token_program_fee_wallet
            + mint
            + dimension
            + reserves
            + curve
            + fee
    }

    /// Returns only reserves which are initialized, ie. this would return
    /// a slice of length 2 if there are only two reserves, etc.
    pub fn reserves(&self) -> &[Reserve] {
        &self.reserves[..self.dimension as usize]
    }

    /// This method calculates the tokens to deposit out of a [`BTreeMap`] of
    /// max tokens available to deposit by the user. When the supply of lp
    /// tokens is zero, in other words, on the first deposit, the tokens to
    /// deposit will be equal to the values in `max_tokens`. Otherwise we will
    /// calculate the maximum amount of tokens we can deposit from all token
    /// mints, such that the reserve balance is preserved. This method
    /// returns [`DepositResult`] with the actual amount of tokens to deposit
    /// along with the amount of lp tokens to be minted in return.
    pub fn deposit_tokens(
        &mut self,
        max_tokens: BTreeMap<Pubkey, TokenAmount>,
        lp_mint_supply: TokenAmount,
    ) -> Result<DepositResult> {
        if max_tokens.values().any(|v| v.amount == 0) {
            return Err(error!(err::arg(
                "Must deposit positive amount of tokens for each mint"
            )));
        }

        if max_tokens.len() != self.dimension as usize {
            return Err(error!(err::arg(
                "Max tokens map does not match pool dimension"
            )));
        }

        if self
            .reserves()
            .iter()
            .any(|r| !max_tokens.contains_key(&r.mint))
        {
            return Err(error!(err::arg(
                "Not all reserve mints are represented in the max tokens map"
            )));
        }

        let is_first_deposit = lp_mint_supply.amount == 0;

        let (tokens_to_deposit, lp_tokens_to_distribute) = if is_first_deposit {
            let lp_tokens_to_distribute = *max_tokens.values().min().ok_or(
                // we've checked that max tokens matches the pool's
                // dimension
                AmmError::InvariantViolation,
            )?;

            (max_tokens, lp_tokens_to_distribute)
        } else {
            // pick the token with the lowest pool price and
            // price all other tokens with that denominator
            let reserve_prices: BTreeMap<Pubkey, Decimal> =
                self.get_reserve_parity_prices()?;

            // Convert max_tokens amounts to denominate in lowest denominated
            // token. Those values will be all comparable
            struct DenominatedToken {
                max_tokens_to_deposit: Decimal,
                total_parity_price: Decimal,
            }
            // Example:
            // {
            //     "mintA" : {
            //         "max_tokens_to_deposit": "10",
            //         "parity_price_per_token": "2",
            //         "total_parity_price": "20",
            //     },
            //     "mintB" : {  // this is the quote token
            //         "max_tokens_to_deposit": "10",
            //         "parity_price_per_token": "1",
            //         "total_parity_price": "10",
            //     },
            //     "mintC" : { // this is the token to deposit of the least
            //         "max_tokens_to_deposit": "5",
            //         "parity_price_per_token": "0.5",
            //         "total_parity_price": "2.5",
            //     },
            // }
            let denominated_tokens: BTreeMap<Pubkey, DenominatedToken> =
                max_tokens
                    .iter()
                    .map(|(mint, tokens)| {
                        let parity_price_per_token = *reserve_prices
                            .get(mint)
                            .ok_or(AmmError::InvariantViolation)?;

                        Ok((
                            *mint,
                            DenominatedToken {
                                max_tokens_to_deposit: (*tokens).into(),
                                total_parity_price: Decimal::from(*tokens)
                                    .try_mul(parity_price_per_token)?,
                            },
                        ))
                    })
                    .collect::<Result<_>>()?;

            // Get the the max_token that has the lowest deposit amount
            //
            // In the example above, this would be mintC
            //
            // This is the limiting factor on the amount of tokens to deposit
            // across all reserves.
            let lowest_token_deposit_total_parity_price = denominated_tokens
                .iter()
                .map(|(_, d)| d.total_parity_price)
                .min()
                .ok_or(AmmError::InvariantViolation)?;
            if lowest_token_deposit_total_parity_price == Decimal::zero() {
                msg!(
                    "No parity price can be zero because \
                    we're following a curve that is \
                    asymptotic to the axis"
                );
                return Err(error!(AmmError::InvariantViolation));
            }

            let tokens_to_deposit = denominated_tokens
                .into_iter()
                .map(|(mint, denominated_token)| {
                    // TODO: put this in README equation
                    //
                    // Consider the example above:
                    // * mintC is the limiting factor in the deposit, ie. we can
                    //   deposit least of mintC in terms of the common price.
                    //   Therefore the amount we deposit is equal to the
                    //   requested max amount by the user.
                    // * mintB is the quote token, ie. the prices of other mints
                    //   are given in mintB. Therefore, the amount to deposit is
                    //   equal to the lowest parity price.
                    // * mintA is neither the limiting factor nor the quote, so
                    //   follow the formula

                    // To keep the same ratios after deposit as there were
                    // before the deposit, we don't take all tokens that user
                    // provided in the "max_tokens" arguments. We found the
                    // limiting factor. Now we need to scale the max amount of
                    // tokens to deposit by the ratio of the total parity price
                    // to the limiting factor.
                    //
                    // For example:
                    // Limiting factor is $5, the total parity price is $10 and
                    // the amount of tokens that hose $10 represent is 100.
                    // We can only deposit $5 worth of those tokens.
                    // $5/$10 * 100 = 50 tokens.
                    let lowest_total_price_to_reserve_total_price =
                        lowest_token_deposit_total_parity_price
                            .try_div(denominated_token.total_parity_price)?;
                    if lowest_total_price_to_reserve_total_price
                        > Decimal::one()
                    {
                        msg!(
                            "The 'lowest_total_price_to_reserve_total_price' \
                            ratio should always be less than 1 because \
                            we are limiting the deposit based on the lowest \
                            reserve price"
                        );
                        return Err(error!(AmmError::InvariantViolation));
                    }

                    Ok((
                        mint,
                        TokenAmount {
                            amount: denominated_token
                                .max_tokens_to_deposit
                                .try_mul(
                                    lowest_total_price_to_reserve_total_price,
                                )?
                                // we ceil to prevent deposit of 0 tokens
                                .try_ceil()?,
                        },
                    ))
                })
                .collect::<Result<BTreeMap<Pubkey, TokenAmount>>>()?;

            let lp_tokens_to_distribute = self
                .get_eligible_lp_tokens(&tokens_to_deposit, lp_mint_supply)?;

            (tokens_to_deposit, lp_tokens_to_distribute)
        };

        // mutate the Pool reserve balances
        for (mint, tokens) in &tokens_to_deposit {
            let reserve =
                self.reserves.iter_mut().find(|r| &r.mint == mint).ok_or(
                    // we checked in the beginning of the method that all
                    // mints are represented
                    AmmError::InvariantViolation,
                )?;

            reserve.tokens.amount = reserve
                .tokens
                .amount
                .checked_add(tokens.amount)
                .ok_or(AmmError::MathOverflow)?;
        }

        Ok(DepositResult {
            lp_tokens_to_distribute,
            tokens_to_deposit,
        })
    }

    /// This method calculates the tokens to redeem out of a given amount of lp
    /// tokens the user is relinquishing back to the pool, to be burned. The
    /// user will also provide a [`BTreeMap`] of min tokens, which serves as a
    /// threshold that guarantees that the redemption only takes place if the
    /// computed tokens to be redeemed are above the min amounts. If this last
    /// condition is not met, the method will return an error.
    ///
    /// This method returns map with the actual amount of tokens to redeem.
    pub fn redeem_tokens(
        &mut self,
        min_tokens: BTreeMap<Pubkey, TokenAmount>,
        lp_tokens_to_burn: TokenAmount,
        lp_mint_supply: TokenAmount,
    ) -> Result<BTreeMap<Pubkey, TokenAmount>> {
        if lp_mint_supply.amount == 0 {
            return Err(error!(err::arg(
                "There are no lp tokens currently in supply."
            )));
        }

        // TODO: remove this constraint as it is checked before
        if lp_tokens_to_burn > lp_mint_supply {
            return Err(error!(err::arg(
                "The amount of lp tokens to burn cannot \
                surpass current supply."
            )));
        }

        if min_tokens.len() != self.dimension as usize {
            return Err(error!(err::arg(
                "Length of min tokens map does not match pool dimension"
            )));
        }

        if self
            .reserves()
            .iter()
            .any(|r| !min_tokens.contains_key(&r.mint))
        {
            return Err(error!(err::arg(
                "Not all reserve mints are represented in the min tokens map"
            )));
        }

        let weight = Decimal::from(lp_tokens_to_burn.amount)
            .try_div(Decimal::from(lp_mint_supply.amount))?;

        // Given a previous deposit of tokens, and provided that no swaps happen
        // in between, if a user withdraws liquidity by burning the same amount
        // of lp tokens it got from the deposit, the token amounts withdrawn
        // from the pool will be essentially the same amounts that were
        // deposited previously, for each given mint.
        let tokens_to_redeem: BTreeMap<Pubkey, TokenAmount> = self
            .reserves()
            .iter()
            .map(|r| {
                Ok((
                    r.mint,
                    TokenAmount::new(
                        Decimal::from(r.tokens.amount)
                            .try_mul(weight)?
                            .try_floor()?,
                    ),
                ))
            })
            .collect::<Result<_>>()?;

        let is_any_redeem_token_below_min_threshold =
            tokens_to_redeem.iter().any(|(mint, token)| {
                let min_token = min_tokens
                    .get(mint)
                    .ok_or(AmmError::InvariantViolation)
                    .unwrap();

                token < min_token
            });

        if is_any_redeem_token_below_min_threshold {
            return Err(error!(err::arg(
                "The amount of tokens to be redeemed is below \
                the min_tokens parameter for at least one of the reserves."
            )));
        }

        // mutate the Pool reserve balances
        for (mint, tokens) in &tokens_to_redeem {
            let reserve =
                self.reserves.iter_mut().find(|r| &r.mint == mint).ok_or(
                    // we checked in the beginning of the method that all
                    // mints are represented
                    AmmError::InvariantViolation,
                )?;

            reserve.tokens.amount = reserve
                .tokens
                .amount
                .checked_sub(tokens.amount)
                .ok_or(AmmError::MathOverflow)?;
        }

        Ok(tokens_to_redeem)
    }

    /// This method will return a [`BTreeMap`] with all the reserve token prices
    /// measured in parity (all with the same denominator/quote). We chose the
    /// token in the pool that has the lowest price to be the quote price for
    /// all the tokens. As an example, if we have x1, x2, x3 and x3 is
    /// the token with the biggest reserve, then this means x3 is the cheapest
    /// token from the perspective of the pool prices. Therefore we calculate
    /// x1 and x2 prices based on x3 as quote.
    ///
    /// # Important
    /// This function mustn't be called when any reserve's balance is 0.
    fn get_reserve_parity_prices(&self) -> Result<BTreeMap<Pubkey, Decimal>> {
        debug_assert!(self.dimension >= 2);
        let lowest_priced_token: Decimal = self
            .reserves()
            .iter()
            .map(|r| r.tokens.amount)
            .max()
            // there always have to be at least two reserves in the pool
            .ok_or(AmmError::InvariantViolation)?
            .into();

        // pick the token with the lowest pool price and
        // price all other tokens with that denominator
        self.reserves()
            .iter()
            .map(|reserve| {
                Ok((
                    reserve.mint,
                    lowest_priced_token
                        .try_div(Decimal::from(reserve.tokens))
                        .map_err(|_| {
                            msg!("No reserve can have a zero balance");
                            AmmError::InvariantViolation
                        })?,
                ))
            })
            .collect()
    }

    /// Any given token in the pool can be used to compute the amount
    /// of lp tokens to be distributed with a given deposit, as long as the
    /// ratios correspond to the ratios present in the pool. We compute
    /// the lp tokens to be distributed with a simple 'rule of 3'. For any given
    /// token in the pool x1, we multiple the deposit delta_x1 with the amount
    /// of lp tokens in supply, and then divide the by the current reserve
    /// amount x1.
    fn get_eligible_lp_tokens(
        &self,
        tokens_deposited: &BTreeMap<Pubkey, TokenAmount>,
        lp_mint_supply: TokenAmount,
    ) -> Result<TokenAmount> {
        debug_assert_ne!(lp_mint_supply, TokenAmount::new(0));
        debug_assert_eq!(tokens_deposited.len(), self.dimension as usize);

        let any_reserve = self.reserves[0];
        let reserve_deposit = tokens_deposited
            .get(&any_reserve.mint)
            .ok_or(AmmError::InvariantViolation)?;

        Ok(TokenAmount::new(
            lp_mint_supply
                .amount
                .checked_mul(reserve_deposit.amount)
                .ok_or(AmmError::MathOverflow)?
                .checked_div(any_reserve.tokens.amount)
                .ok_or(AmmError::MathOverflow)?,
        ))
    }

    /// This is called after a deposit or redemption.
    pub fn update_curve_invariant(&mut self) -> Result<()> {
        match self.curve {
            Curve::ConstProd => (),
            Curve::Stable { amplifier, .. } => {
                // need to recompute curve invariant, using Newton-Raphson
                // approximation method
                let token_reserves_amount: Vec<_> =
                    self.reserves().iter().map(|rs| rs.tokens).collect();

                let invariant = if token_reserves_amount
                    .iter()
                    .any(|tokens| tokens.amount == 0)
                {
                    // this can happen on redeem, when all tokens are withdrawn
                    Decimal::zero().into()
                } else {
                    math::stable_curve_invariant::compute(
                        amplifier,
                        &token_reserves_amount,
                    )?
                    .into()
                };

                self.curve = Curve::Stable {
                    amplifier,
                    invariant,
                };
            }
        }

        Ok(())
    }

    pub fn check_amount_tokens_is_valid(
        &self,
        amount_tokens: &BTreeMap<Pubkey, TokenAmount>,
    ) -> Result<()> {
        // check that max_amount_tokens have the correct mint pubkeys
        // vec of all non-trivial mints
        let num_available_mints = self
            .reserves()
            .iter()
            .filter(|r| amount_tokens.contains_key(&r.mint))
            .count();

        // in case there are missing mints from max_amount_tokens compared to
        // the pool reserves, we throw an error
        if num_available_mints != amount_tokens.len() {
            return Err(error!(AmmError::InvalidTokenMints));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn works_with_two_deposits_with_different_ratios() {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 2,
            reserves: [
                Reserve {
                    tokens: TokenAmount::new(100),
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(1),
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            ..Default::default()
        };

        let mut max_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();
        max_tokens.insert(mint1, TokenAmount::new(500));
        max_tokens.insert(mint2, TokenAmount::new(2));

        // deposit within a different ratio
        pool.deposit_tokens(max_tokens, TokenAmount::new(1))
            .unwrap();

        assert_eq!(pool.reserves[0].tokens.amount, 300);
        assert_eq!(pool.reserves[1].tokens.amount, 3);
    }

    #[test]
    fn it_calculates_tokens_to_deposit_when_first_deposit() -> Result<()> {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount::new(0), // 10
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0), // 100
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0), // 250
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            ..Default::default()
        };

        // Initial deposit
        let mut max_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();
        max_tokens.insert(mint1, TokenAmount::new(10));
        max_tokens.insert(mint2, TokenAmount::new(100));
        max_tokens.insert(mint3, TokenAmount::new(250));

        let deposit_result =
            pool.deposit_tokens(max_tokens, TokenAmount::new(0))?;

        // Check the pool was currectly updated
        assert_eq!(pool.reserves[0].mint, mint1);
        assert_eq!(pool.reserves[0].tokens.amount, 10);

        assert_eq!(pool.reserves[1].mint, mint2);
        assert_eq!(pool.reserves[1].tokens.amount, 100);

        assert_eq!(pool.reserves[2].mint, mint3);
        assert_eq!(pool.reserves[2].tokens.amount, 250);

        // check that calculated tokens to deposit is correct
        let tokens_to_deposit = &deposit_result.tokens_to_deposit;
        assert_eq!(tokens_to_deposit.get(&mint1).unwrap().amount, 10);
        assert_eq!(tokens_to_deposit.get(&mint2).unwrap().amount, 100);
        assert_eq!(tokens_to_deposit.get(&mint3).unwrap().amount, 250);

        // check that calculated lp tokens to disburse is correct
        // In this case the lp tokens disbursed should be equal to 10 since its
        // the deposit amount of the most expensive token
        assert_eq!(deposit_result.lp_tokens_to_distribute.amount, 10);

        Ok(())
    }

    #[test]
    fn it_calculates_tokens_to_deposit_when_not_first_deposit() -> Result<()> {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount::new(10),
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(100),
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(250),
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            ..Default::default()
        };

        let mut max_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();
        max_tokens.insert(mint1, TokenAmount::new(5));
        max_tokens.insert(mint2, TokenAmount::new(50));
        max_tokens.insert(mint3, TokenAmount::new(100));

        let deposit_result =
            pool.deposit_tokens(max_tokens, TokenAmount::new(10))?;

        // Check the pool was currectly updated
        assert_eq!(pool.reserves[0].mint, mint1);
        assert_eq!(pool.reserves[0].tokens.amount, 10 + 4);

        assert_eq!(pool.reserves[1].mint, mint2);
        assert_eq!(pool.reserves[1].tokens.amount, 100 + 40);

        assert_eq!(pool.reserves[2].mint, mint3);
        assert_eq!(pool.reserves[2].tokens.amount, 250 + 100);

        // check that calculated tokens to deposit is correct
        let tokens_to_deposit = &deposit_result.tokens_to_deposit;
        assert_eq!(tokens_to_deposit.get(&mint1).unwrap().amount, 4);
        assert_eq!(tokens_to_deposit.get(&mint2).unwrap().amount, 40);
        assert_eq!(tokens_to_deposit.get(&mint3).unwrap().amount, 100);

        // check that calculated lp tokens to disburse is correct
        // In this case the lp tokens disbursed should be equal to 4, we
        // calculate this via a simple rule of three
        assert_eq!(deposit_result.lp_tokens_to_distribute.amount, 4);

        Ok(())
    }

    #[test]
    fn it_handles_tokens_to_deposit_when_hashmap_is_empty() -> Result<()> {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount::new(10),
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(100),
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(250),
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            ..Default::default()
        };

        let max_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        // Assert that is error when first deposit
        assert!(pool
            .deposit_tokens(max_tokens.clone(), TokenAmount::new(0))
            .is_err());

        // Assert that is error when not first deposit
        assert!(pool
            .deposit_tokens(max_tokens, TokenAmount::new(10))
            .is_err());

        Ok(())
    }

    #[test]
    fn it_handles_tokens_to_deposit_when_all_max_tokens_are_zero() -> Result<()>
    {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount::new(10),
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(100),
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(250),
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            ..Default::default()
        };

        let mut max_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();
        max_tokens.insert(mint1, TokenAmount::new(0));
        max_tokens.insert(mint2, TokenAmount::new(0));
        max_tokens.insert(mint3, TokenAmount::new(0));

        // Assert that is error when first deposit
        assert!(pool
            .deposit_tokens(max_tokens.clone(), TokenAmount::new(0))
            .is_err());
        // Assert that is error when not first deposit
        assert!(pool
            .deposit_tokens(max_tokens.clone(), TokenAmount::new(10))
            .is_err());

        Ok(())
    }

    #[test]
    fn it_calculates_tokens_to_redeem_when_min_tokens_are_zero() -> Result<()> {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount::new(10),
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(100),
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(250),
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            ..Default::default()
        };

        let lp_mint_supply = TokenAmount::new(1_000);
        let lp_tokens_to_burn = TokenAmount::new(100);
        let mut min_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        min_tokens.insert(mint1, TokenAmount::new(0));
        min_tokens.insert(mint2, TokenAmount::new(0));
        min_tokens.insert(mint3, TokenAmount::new(0));

        let tokens_to_redeem =
            pool.redeem_tokens(min_tokens, lp_tokens_to_burn, lp_mint_supply)?;

        // Check the pool was currectly updated
        assert_eq!(pool.reserves[0].mint, mint1);
        assert_eq!(pool.reserves[0].tokens.amount, 10 - 1);

        assert_eq!(pool.reserves[1].mint, mint2);
        assert_eq!(pool.reserves[1].tokens.amount, 100 - 10);

        assert_eq!(pool.reserves[2].mint, mint3);
        assert_eq!(pool.reserves[2].tokens.amount, 250 - 25);

        // check that calculated tokens to redeem is correct
        assert_eq!(tokens_to_redeem.get(&mint1).unwrap().amount, 1);
        assert_eq!(tokens_to_redeem.get(&mint2).unwrap().amount, 10);
        assert_eq!(tokens_to_redeem.get(&mint3).unwrap().amount, 25);

        Ok(())
    }

    #[test]
    fn it_calculates_tokens_to_redeem_when_min_tokens_match_tokens_to_redeem(
    ) -> Result<()> {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount::new(10),
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(100),
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(250),
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            ..Default::default()
        };

        let lp_mint_supply = TokenAmount::new(1_000);
        let lp_tokens_to_burn = TokenAmount::new(100);
        let mut min_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        min_tokens.insert(mint1, TokenAmount::new(1));
        min_tokens.insert(mint2, TokenAmount::new(10));
        min_tokens.insert(mint3, TokenAmount::new(25));

        let tokens_to_redeem =
            pool.redeem_tokens(min_tokens, lp_tokens_to_burn, lp_mint_supply)?;

        // Check the pool was currectly updated
        assert_eq!(pool.reserves[0].mint, mint1);
        assert_eq!(pool.reserves[0].tokens.amount, 10 - 1);

        assert_eq!(pool.reserves[1].mint, mint2);
        assert_eq!(pool.reserves[1].tokens.amount, 100 - 10);

        assert_eq!(pool.reserves[2].mint, mint3);
        assert_eq!(pool.reserves[2].tokens.amount, 250 - 25);

        // check that calculated tokens to redeem is correct
        assert_eq!(tokens_to_redeem.get(&mint1).unwrap().amount, 1);
        assert_eq!(tokens_to_redeem.get(&mint2).unwrap().amount, 10);
        assert_eq!(tokens_to_redeem.get(&mint3).unwrap().amount, 25);

        Ok(())
    }

    #[test]
    fn it_calculates_tokens_to_redeem_after_token_deposit() -> Result<()> {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            ..Default::default()
        };

        let mut lp_mint_supply = TokenAmount::new(0);
        let mut max_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        max_tokens.insert(mint1, TokenAmount::new(100));
        max_tokens.insert(mint2, TokenAmount::new(100));
        max_tokens.insert(mint3, TokenAmount::new(100));

        pool.deposit_tokens(max_tokens, lp_mint_supply)?;

        lp_mint_supply = TokenAmount::new(100);
        let lp_tokens_to_burn = TokenAmount::new(50);
        let mut min_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        min_tokens.insert(mint1, TokenAmount::new(50));
        min_tokens.insert(mint2, TokenAmount::new(50));
        min_tokens.insert(mint3, TokenAmount::new(50));

        let tokens_to_redeem =
            pool.redeem_tokens(min_tokens, lp_tokens_to_burn, lp_mint_supply)?;

        // Check the pool was currectly updated
        assert_eq!(pool.reserves[0].mint, mint1);
        assert_eq!(pool.reserves[0].tokens.amount, 100 - 50);

        assert_eq!(pool.reserves[1].mint, mint2);
        assert_eq!(pool.reserves[1].tokens.amount, 100 - 50);

        assert_eq!(pool.reserves[2].mint, mint3);
        assert_eq!(pool.reserves[2].tokens.amount, 100 - 50);

        // check that calculated tokens to redeem is correct
        assert_eq!(tokens_to_redeem.get(&mint1).unwrap().amount, 50);
        assert_eq!(tokens_to_redeem.get(&mint2).unwrap().amount, 50);
        assert_eq!(tokens_to_redeem.get(&mint3).unwrap().amount, 50);

        // Second withdrawal
        lp_mint_supply = TokenAmount::new(50);
        let mut min_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        min_tokens.insert(mint1, TokenAmount::new(0));
        min_tokens.insert(mint2, TokenAmount::new(0));
        min_tokens.insert(mint3, TokenAmount::new(0));

        let tokens_to_redeem =
            pool.redeem_tokens(min_tokens, lp_tokens_to_burn, lp_mint_supply)?;

        // Check the pool was currectly updated
        assert_eq!(pool.reserves[0].mint, mint1);
        assert_eq!(pool.reserves[0].tokens.amount, 0);

        assert_eq!(pool.reserves[1].mint, mint2);
        assert_eq!(pool.reserves[1].tokens.amount, 0);

        assert_eq!(pool.reserves[2].mint, mint3);
        assert_eq!(pool.reserves[2].tokens.amount, 0);

        // check that calculated tokens to redeem is correct
        assert_eq!(tokens_to_redeem.get(&mint1).unwrap().amount, 50);
        assert_eq!(tokens_to_redeem.get(&mint2).unwrap().amount, 50);
        assert_eq!(tokens_to_redeem.get(&mint3).unwrap().amount, 50);

        Ok(())
    }

    #[test]
    fn it_errs_tokens_to_redeem_when_min_tokens_threshold_reached() -> Result<()>
    {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount::new(10),
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(100),
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(250),
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            ..Default::default()
        };

        let lp_mint_supply = TokenAmount::new(1_000);
        let lp_tokens_to_burn = TokenAmount::new(100);
        let mut min_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        min_tokens.insert(mint1, TokenAmount::new(1));
        min_tokens.insert(mint2, TokenAmount::new(10));
        min_tokens.insert(mint3, TokenAmount::new(26));

        assert!(pool
            .redeem_tokens(min_tokens, lp_tokens_to_burn, lp_mint_supply)
            .is_err());

        Ok(())
    }

    #[test]
    fn it_errs_tokens_to_redeem_when_missing_tokens_in_min_tokens_map(
    ) -> Result<()> {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();
        let fake_mint = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount::new(10),
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(100),
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(250),
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount::new(0),
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            ..Default::default()
        };

        let lp_mint_supply = TokenAmount::new(1_000);
        let lp_tokens_to_burn = TokenAmount::new(100);
        let mut min_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        min_tokens.insert(mint1, TokenAmount::new(1));
        min_tokens.insert(mint2, TokenAmount::new(10));
        min_tokens.insert(fake_mint, TokenAmount::new(1));

        assert!(pool
            .redeem_tokens(min_tokens, lp_tokens_to_burn, lp_mint_supply)
            .is_err());

        Ok(())
    }

    #[test]
    fn it_errs_tokens_to_redeem_when_zero_lp_mint_supply() -> Result<()> {
        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            ..Default::default()
        };

        let lp_mint_supply = TokenAmount::new(0);
        let lp_tokens_to_burn = TokenAmount::new(0);
        let min_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        assert!(pool
            .redeem_tokens(min_tokens, lp_tokens_to_burn, lp_mint_supply)
            .is_err());

        Ok(())
    }

    #[test]
    fn it_errs_tokens_to_redeem_when_lp_tokens_to_burn_is_gt_than_supply(
    ) -> Result<()> {
        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            ..Default::default()
        };

        let lp_mint_supply = TokenAmount::new(10);
        let lp_tokens_to_burn = TokenAmount::new(100);
        let min_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        assert!(pool
            .redeem_tokens(min_tokens, lp_tokens_to_burn, lp_mint_supply)
            .is_err());

        Ok(())
    }

    #[test]
    fn it_errs_tokens_to_redeem_when_min_tokens_map_len_diff_dimension(
    ) -> Result<()> {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            ..Default::default()
        };

        let lp_mint_supply = TokenAmount::new(1_000);
        let lp_tokens_to_burn = TokenAmount::new(100);
        let mut min_tokens: BTreeMap<Pubkey, TokenAmount> = BTreeMap::new();

        min_tokens.insert(mint1, TokenAmount::new(1));
        min_tokens.insert(mint2, TokenAmount::new(10));

        assert!(pool
            .redeem_tokens(min_tokens, lp_tokens_to_burn, lp_mint_supply)
            .is_err());

        Ok(())
    }

    #[test]
    fn test_update_curve_invariant() {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let mut pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount { amount: 10 },
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount { amount: 100 },
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount { amount: 250 },
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount { amount: 0 },
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            curve: Curve::Stable {
                amplifier: 10_u64,
                invariant: Decimal::from(360_u64).into(),
            },
            ..Default::default()
        };

        pool.update_curve_invariant().unwrap();

        let invariant = match pool.curve {
            Curve::ConstProd => panic!("unexpected constant product curve"),
            Curve::Stable { invariant, .. } => invariant,
        };

        assert_eq!(
            Decimal::from(invariant),
            Decimal::from_scaled_val(352805603000000000000)
        );
    }

    #[test]
    fn test_check_amount_tokens_is_valid_fails() {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount { amount: 10 },
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount { amount: 100 },
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount { amount: 250 },
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount { amount: 0 },
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            curve: Curve::Stable {
                amplifier: 10_u64,
                invariant: Decimal::from(360_u64).into(),
            },
            ..Default::default()
        };

        let amount_tokens1 = BTreeMap::from([
            (Pubkey::new_unique(), TokenAmount { amount: 10 }),
            (mint2, TokenAmount { amount: 100 }),
            (mint3, TokenAmount { amount: 250 }),
        ]);

        assert!(pool.check_amount_tokens_is_valid(&amount_tokens1).is_err());

        let amount_tokens2 = BTreeMap::from([
            (mint1, TokenAmount { amount: 10 }),
            (Pubkey::new_unique(), TokenAmount { amount: 100 }),
            (mint3, TokenAmount { amount: 250 }),
        ]);

        assert!(pool.check_amount_tokens_is_valid(&amount_tokens2).is_err());

        let amount_tokens3 = BTreeMap::from([
            (mint1, TokenAmount { amount: 10 }),
            (mint2, TokenAmount { amount: 100 }),
            (Pubkey::new_unique(), TokenAmount { amount: 250 }),
        ]);

        assert!(pool.check_amount_tokens_is_valid(&amount_tokens3).is_err());
    }

    #[test]
    fn test_check_amount_tokens_is_valid_succeeds() {
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();

        let pool = Pool {
            mint: Pubkey::new_unique(),
            dimension: 3,
            reserves: [
                Reserve {
                    tokens: TokenAmount { amount: 10 },
                    mint: mint1,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount { amount: 100 },
                    mint: mint2,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount { amount: 250 },
                    mint: mint3,
                    vault: Pubkey::default(),
                },
                Reserve {
                    tokens: TokenAmount { amount: 0 },
                    mint: Pubkey::default(),
                    vault: Pubkey::default(),
                },
            ],
            curve: Curve::Stable {
                amplifier: 10_u64,
                invariant: Decimal::from(360_u64).into(),
            },
            ..Default::default()
        };

        let amount_tokens = BTreeMap::from([
            (mint1, TokenAmount { amount: 10 }),
            (mint2, TokenAmount { amount: 100 }),
            (mint3, TokenAmount { amount: 250 }),
        ]);

        pool.check_amount_tokens_is_valid(&amount_tokens).unwrap();
    }
}
