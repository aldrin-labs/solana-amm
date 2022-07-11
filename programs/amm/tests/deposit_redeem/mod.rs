use ::amm::amm::deposit_liquidity;
use ::amm::amm::redeem_liquidity;
use ::amm::prelude::*;
use anchor_spl::token;
pub use anchor_spl::token::spl_token::state::{Account as TokenAccount, Mint};
use anchortest::{
    builder::*,
    spl::{self, TokenAccountExt},
    stub,
};
use pretty_assertions::assert_eq;
use solana_sdk::instruction::Instruction;
use solana_sdk::program_pack::Pack;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub struct Tester {
    pub user: AccountInfoWrapper,
    pub pool: AccountInfoWrapper,
    pub pool_signer: AccountInfoWrapper,
    pub lp_mint: AccountInfoWrapper,
    pub lp_token_wallet: AccountInfoWrapper,
    pub token_program: AccountInfoWrapper,
    pub vaults_wallets: Vec<AccountInfoWrapper>,
}

impl Tester {
    pub fn new_const_prod(dimension: usize) -> (Self, Vec<Reserve>) {
        Self::new(dimension, Curve::ConstProd)
    }

    pub fn new_stable_curve(
        dimension: usize,
        amplifier: u64,
        invariant: Decimal,
    ) -> (Self, Vec<Reserve>) {
        Self::new(
            dimension,
            Curve::Stable {
                amplifier,
                invariant: invariant.into(),
            },
        )
    }

    pub fn new(dimension: usize, curve: Curve) -> (Self, Vec<Reserve>) {
        let user = AccountInfoWrapper::new().mutable().signer();
        let pool = AccountInfoWrapper::new().owner(amm::ID).mutable();
        let pool_signer = AccountInfoWrapper::pda(
            amm::ID,
            "pool_signer",
            &[Pool::SIGNER_PDA_PREFIX, pool.key.as_ref()],
        );
        let lp_mint = AccountInfoWrapper::new()
            .mutable()
            .pack(spl::mint::new(pool_signer.key))
            .owner(token::ID);
        let lp_token_wallet = AccountInfoWrapper::new()
            .mutable()
            .pack(spl::token_account::new(user.key).mint(lp_mint.key))
            .owner(token::ID);
        let token_program = AccountInfoWrapper::with_key(token::ID).program();
        let mut reserves = [Reserve::default(); consts::MAX_RESERVES];
        let vaults_wallets: Vec<_> = (0..dimension)
            .map(|index| {
                let mint = Pubkey::new_unique();
                let vault = AccountInfoWrapper::new()
                    .pack(spl::token_account::new(pool_signer.key).mint(mint))
                    .owner(token::ID);

                reserves[index] = Reserve {
                    vault: vault.key,
                    mint: mint,
                    tokens: TokenAmount::new(0),
                };

                let wallet = AccountInfoWrapper::new()
                    .pack(
                        spl::token_account::new(user.key)
                            .mint(mint)
                            .amount(1_000_000_000),
                    )
                    .owner(token::ID);
                vec![vault, wallet].into_iter()
            })
            .flatten()
            .collect();
        assert_eq!(vaults_wallets.len(), dimension * 2);

        let pool_data = Pool {
            signer: pool_signer.key,
            mint: lp_mint.key,
            dimension: dimension as u64,
            curve,
            reserves,
            ..Default::default()
        };
        let reserves = pool_data.reserves().iter().copied().collect();
        let pool = pool.data(pool_data);

        (
            Self {
                user,
                pool,
                pool_signer,
                lp_mint,
                lp_token_wallet,
                token_program,
                vaults_wallets,
            },
            reserves,
        )
    }
}

impl Tester {
    pub fn deposit_liquidity(
        &mut self,
        max_amount_tokens: BTreeMap<Pubkey, TokenAmount>,
        reserves: &[Reserve],
    ) -> Result<()> {
        let mut pool = Pool::try_deserialize(&mut self.pool.data.as_slice())?;
        let lp_mint = Mint::unpack(&mut self.lp_mint.data.as_slice())?;

        let DepositResult {
            lp_tokens_to_distribute,
            tokens_to_deposit,
        } = pool
            .deposit_tokens(
                max_amount_tokens.clone(),
                TokenAmount::new(lp_mint.supply),
            )
            // We might provide args which make deposit_tokens fail, but we
            // still want to test that scenario, therefore we must't panic here.
            // The default value therefore becomes irrelevant because the handle
            // function shall never reach any transfer.
            .unwrap_or_default();

        // Generally, the order of the reserves and the transfers does not
        // correspond. However, in our tests, we generate the vaults_wallets
        // vec in such a fashion that it actually does correspond to the order
        // or reserves, therefore we take a shortcut and just order the
        // transfers by reserves.
        let transfers: Vec<_> = reserves
            .iter()
            .zip(self.vaults_wallets.chunks(2))
            .map(|(r, vault_wallet)| {
                (
                    vault_wallet[0].key, // vault
                    vault_wallet[1].key, // wallet
                    tokens_to_deposit
                        .get(&r.mint)
                        .copied()
                        // in case we want to test for mismatch between
                        // input args and reserves, we cannot panic
                        .unwrap_or(TokenAmount::new(0)),
                )
            })
            .collect();
        let state = self.set_syscalls(CpiValidatorState::Deposit {
            user: self.user.key,
            transfers,
            next_cpi: MintLpTokens {
                mint: self.lp_mint.key,
                destination: self.lp_token_wallet.key,
                pool_signer: self.pool_signer.key,
                lp_tokens_to_distribute,
            },
        });

        let mut ctx = self.context_wrapper();
        let mut accounts = ctx.accounts()?;

        let max_amount_tokens: Vec<_> = max_amount_tokens
            .into_iter()
            .map(|(mint, tokens)| DepositMintTokens { mint, tokens })
            .collect();

        deposit_liquidity(ctx.build(&mut accounts), max_amount_tokens)?;
        accounts.exit(&amm::ID)?;

        assert_eq!(*state.lock().unwrap(), CpiValidatorState::Done);
        Ok(())
    }

    pub fn redeem_liquidity(
        &mut self,
        min_amount_tokens: BTreeMap<Pubkey, TokenAmount>,
        lp_tokens_to_burn: TokenAmount,
        reserves: &[Reserve],
    ) -> Result<()> {
        let mut pool = Pool::try_deserialize(&mut self.pool.data.as_slice())?;
        let lp_mint = Mint::unpack(&mut self.lp_mint.data.as_slice())?;
        let tokens_to_redeem = pool
            .redeem_tokens(
                min_amount_tokens.clone(),
                lp_tokens_to_burn,
                TokenAmount::new(lp_mint.supply),
            )
            // We might provide args which make redeem_tokens fail, but we
            // still want to test that scenario, therefore we must't panic here.
            // The default value therefore becomes irrelevant because the handle
            // function shall never reach any transfer.
            .unwrap_or_default();

        let RedeemResult {
            lp_tokens_to_burn,
            tokens_to_redeem,
        } = RedeemResult {
            lp_tokens_to_burn,
            tokens_to_redeem,
        };

        // Generally, the order of the reserves and the transfers does not
        // correspond. However, in our tests, we generate the vaults_wallets
        // vec in such a fashion that it actually does correspond to the order
        // or reserves, therefore we take a shortcut and just order the
        // transfers by reserves.
        let transfers: Vec<_> = reserves
            .iter()
            .zip(self.vaults_wallets.chunks(2))
            .map(|(r, vault_wallet)| {
                (
                    vault_wallet[0].key, // vault
                    vault_wallet[1].key, // wallet
                    tokens_to_redeem
                        .get(&r.mint)
                        .copied()
                        // in case we want to test for mismatch between
                        // input args and reserves, we cannot panic
                        .unwrap_or(TokenAmount::new(0)),
                )
            })
            .collect();
        let state = self.set_syscalls(CpiValidatorState::Redeem {
            pool_signer: self.pool_signer.key,
            transfers,
            next_cpi: BurnLpTokens {
                mint: self.lp_mint.key,
                source: self.lp_token_wallet.key,
                user: self.user.key,
                lp_tokens_to_burn,
            },
        });

        let mut ctx = self.context_wrapper();
        let mut accounts = ctx.accounts()?;

        let min_amount_tokens: Vec<_> = min_amount_tokens
            .into_iter()
            .map(|(mint, tokens)| RedeemMintTokens { mint, tokens })
            .collect();

        redeem_liquidity(
            ctx.build(&mut accounts),
            lp_tokens_to_burn,
            min_amount_tokens,
        )?;
        accounts.exit(&amm::ID)?;

        assert_eq!(*state.lock().unwrap(), CpiValidatorState::Done);

        Ok(())
    }

    fn context_wrapper(&mut self) -> ContextWrapper {
        ContextWrapper::new(amm::ID)
            .acc(&mut self.user)
            .acc(&mut self.pool)
            .acc(&mut self.pool_signer)
            .acc(&mut self.lp_mint)
            .acc(&mut self.lp_token_wallet)
            .acc(&mut self.token_program)
            .remaining_accounts(self.vaults_wallets.iter_mut())
    }

    fn set_syscalls(
        &self,
        state: CpiValidatorState,
    ) -> Arc<Mutex<CpiValidatorState>> {
        let state = Arc::new(Mutex::new(state));
        stub::Syscalls::new(CpiValidator(Arc::clone(&state))).set();
        state
    }
}

struct CpiValidator(Arc<Mutex<CpiValidatorState>>);

#[derive(Debug, PartialEq, Eq)]
enum CpiValidatorState {
    Deposit {
        user: Pubkey,
        transfers: Vec<(Pubkey, Pubkey, TokenAmount)>,
        next_cpi: MintLpTokens,
    },
    MintLpTokens(MintLpTokens),
    Redeem {
        pool_signer: Pubkey,
        transfers: Vec<(Pubkey, Pubkey, TokenAmount)>,
        next_cpi: BurnLpTokens,
    },
    BurnLpTokens(BurnLpTokens),
    Done,
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct MintLpTokens {
    mint: Pubkey,
    destination: Pubkey,
    pool_signer: Pubkey,
    lp_tokens_to_distribute: Option<TokenAmount>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct BurnLpTokens {
    mint: Pubkey,
    source: Pubkey,
    user: Pubkey,
    lp_tokens_to_burn: TokenAmount,
}

impl stub::ValidateCpis for CpiValidator {
    fn validate_next_instruction(
        &mut self,
        ix: &Instruction,
        accounts: &[AccountInfo],
    ) {
        let mut state = self.0.lock().unwrap();

        match *state {
            CpiValidatorState::Deposit {
                user,
                ref mut transfers,
                ref next_cpi,
            } => {
                // take the first transfer, ie. the one that should correspond
                // to the current instruction, as they are sorted
                let (vault, wallet, tokens) = transfers.remove(0);

                let expected_ix = token::spl_token::instruction::transfer(
                    &token::ID,
                    &wallet,
                    &vault,
                    &user,
                    &[],
                    tokens.amount,
                )
                .unwrap();
                assert_eq!(&expected_ix, ix);

                let from_wallet = &accounts[0];
                let to_vault = &accounts[1];
                assert_eq!(from_wallet.key(), wallet.key());
                assert_eq!(to_vault.key(), vault.key());

                spl::token_account::transfer(
                    &from_wallet,
                    &to_vault,
                    tokens.amount,
                )
                .expect("Source wallet does not have enough tokens");

                if transfers.is_empty() {
                    *state = CpiValidatorState::MintLpTokens(next_cpi.clone());
                }
            }
            CpiValidatorState::MintLpTokens(MintLpTokens {
                mint,
                destination,
                pool_signer,
                lp_tokens_to_distribute,
            }) => {
                let expected_ix = token::spl_token::instruction::mint_to(
                    &token::ID,
                    &mint,
                    &destination,
                    &pool_signer,
                    &[],
                    lp_tokens_to_distribute.unwrap_or_default().amount,
                )
                .unwrap();
                assert_eq!(&expected_ix, ix);

                let wallet = &accounts[0];
                let lp_mint = &accounts[1];
                assert_eq!(wallet.key(), destination);
                assert_eq!(lp_mint.key(), mint);

                spl::mint::mint_to(
                    wallet,
                    lp_mint,
                    lp_tokens_to_distribute.unwrap_or_default().amount,
                )
                .expect("Cannot mint LP tokens");

                *state = CpiValidatorState::Done;
            }
            CpiValidatorState::Redeem {
                pool_signer,
                ref mut transfers,
                ref mut next_cpi,
            } => {
                // take the first transfer, ie. the one that should correspond
                // to the current instruction, as they are sorted
                let (vault, wallet, tokens) = transfers.remove(0);

                let expected_ix = token::spl_token::instruction::transfer(
                    &token::ID,
                    &vault,
                    &wallet,
                    &pool_signer,
                    &[],
                    tokens.amount,
                )
                .unwrap();
                assert_eq!(&expected_ix, ix);

                let from_vault = &accounts[0];
                let to_wallet = &accounts[1];
                assert_eq!(to_wallet.key(), wallet.key());
                assert_eq!(from_vault.key(), vault.key());

                spl::token_account::transfer(
                    &from_vault,
                    &to_wallet,
                    tokens.amount,
                )
                .expect("Source vault does not have enough tokens");

                if transfers.is_empty() {
                    *state = CpiValidatorState::BurnLpTokens(next_cpi.clone());
                }
            }
            CpiValidatorState::BurnLpTokens(BurnLpTokens {
                mint,
                source,
                user,
                lp_tokens_to_burn,
            }) => {
                let expected_ix = token::spl_token::instruction::burn(
                    &token::ID,
                    &source,
                    &mint,
                    &user,
                    &[],
                    lp_tokens_to_burn.amount,
                )
                .unwrap();
                assert_eq!(&expected_ix, ix);

                let wallet = &accounts[0];
                let lp_mint = &accounts[1];
                assert_eq!(wallet.key(), source);
                assert_eq!(lp_mint.key(), mint);

                spl::mint::burn_from(wallet, lp_mint, lp_tokens_to_burn.amount)
                    .expect("Cannot burn LP tokens");

                *state = CpiValidatorState::Done;
            }
            CpiValidatorState::Done => {
                panic!("No more instructions expected, got {:#?}", ix);
            }
        }
    }
}
