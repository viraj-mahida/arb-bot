//! The shared vocabulary of the bot: DEX, pool, tick, and tick array.
//!
//! Every later step speaks in these types. They are DEX-agnostic on purpose:
//! Orca and Raydium store their pools with different byte layouts (Step 2
//! handles that), but once decoded, both look exactly like the structs below.
//! Adding a new concentrated-liquidity DEX later means writing a new decoder,
//! not changing these types.

use super::solana_public_key_helpers::PublicKeyBytes;

/// Which decentralized exchange (DEX) program owns a pool.
///
/// **What it is:** on Solana, a DEX is just a *program* (a smart contract) living
/// at a fixed address. Pools are accounts owned by that program.
///
/// **Why the bot needs it:** each DEX lays out its account bytes differently,
/// derives addresses differently, and has its own official quote library. This
/// enum tells the other steps which rulebook to use.
///
/// **Next:** more DEXes (Meteora, Phoenix, Lifinity, …). Each one becomes a
/// new variant here plus its own decoder and quote file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DexProgram {
    /// Raydium's Concentrated Liquidity Market Maker program.
    RaydiumClmm,
    /// Orca's Whirlpool program (also a concentrated liquidity market maker).
    OrcaWhirlpool,
}

impl DexProgram {
    /// Short machine-friendly name used in log lines, e.g. `"orca_whirlpool"`.
    pub fn name(self) -> &'static str {
        match self {
            DexProgram::RaydiumClmm => "raydium_clmm",
            DexProgram::OrcaWhirlpool => "orca_whirlpool",
        }
    }

    /// How many tick slots fit inside one on-chain tick-array account.
    ///
    /// **Where it comes from:** a hard-coded constant in each DEX's program.
    /// Orca chose 88 slots per array; Raydium chose 60.
    ///
    /// **Why it matters:** together with `tick_spacing` it decides how wide a
    /// price range one tick array covers, which decides which tick-array
    /// addresses we must subscribe to.
    pub fn ticks_per_tick_array(self) -> i32 {
        match self {
            DexProgram::RaydiumClmm => 60,
            DexProgram::OrcaWhirlpool => 88,
        }
    }
}

/// Everything the bot knows about one concentrated-liquidity pool right now.
///
/// **What a pool is:** a pool is an on-chain account that holds two tokens
/// (token A and token B, e.g. SOL and USDC) and lets anyone swap one for the
/// other. The price is not set by people placing orders; it is set by math
/// based on how much of each token sits in the pool (see Step 4 docs).
///
/// **Where it comes from:** Step 2 decodes this from the pool account's raw bytes,
/// every time Geyser tells us the account changed.
///
/// **Why the bot needs it:** the current price, the active liquidity, and the fee
/// are the three inputs to every quote and every arbitrage calculation.
///
/// "Token A" and "token B" are the DEX's own names for the two sides of the
/// pool. In today's SOL/USDC pools, token A = SOL and token B = USDC.
#[derive(Debug, Clone)]
pub struct ConcentratedLiquidityPoolState {
    /// The pool account's own address (its public key).
    pub pool_address: PublicKeyBytes,
    /// Which DEX program owns this pool.
    pub dex: DexProgram,

    /// Current price, stored as a square root in Q64.64 fixed-point format.
    ///
    /// **On-chain name:** `sqrt_price` (Orca) / `sqrt_price_x64` (Raydium).
    ///
    /// **What it is:** `sqrt(price) * 2^64`, stored as a whole number. Blockchains
    /// avoid decimal (floating-point) numbers because every validator must get
    /// bit-for-bit identical results, so the program multiplies by 2^64 and
    /// keeps an integer — the same trick as storing $12.34 as 1234 cents.
    ///
    /// **Why the square root:** it makes the swap math linear inside one tick
    /// range (proved in Step 5's formulas file).
    ///
    /// Price here is "raw token B units per raw token A unit" (micro-USDC per
    /// lamport), before adjusting for decimals.
    pub sqrt_price_q64_64: u128,

    /// Liquidity that is active at the current price.
    ///
    /// **On-chain name:** `liquidity`.
    ///
    /// **What it is:** a number (called `L`) describing how deep the pool is at
    /// this exact price. Deep pool = a big trade barely moves the price.
    /// Shallow pool = even a small trade moves the price a lot.
    ///
    /// It only counts liquidity providers whose chosen price range contains
    /// the current price, so it changes whenever the price crosses a tick
    /// where someone's range starts or ends.
    pub active_liquidity_at_current_price: u128,

    /// Index of the tick the current price sits in.
    ///
    /// **On-chain name:** `tick_current_index` (Orca) / `tick_current` (Raydium).
    ///
    /// **What a tick is:** the price line is cut into tiny steps. Tick `i` means
    /// price `1.0001^i`, so moving one tick changes price by 0.01% (1 basis point).
    pub current_tick_index: i32,

    /// Only every `tick_spacing`-th tick may be used as a range boundary.
    ///
    /// **Why it exists:** allowing every single tick would create too many
    /// accounts. Pools with spacing 64, for example, only allow liquidity
    /// ranges to start or end at ticks divisible by 64.
    pub tick_spacing: u16,

    /// Swap fee in millionths of the input amount.
    ///
    /// **On-chain name:** `fee_rate`.
    ///
    /// Example: `400` means 400 / 1,000,000 = 0.04% = 4 basis points. On a
    /// 1,000 USDC swap the pool keeps 0.40 USDC for its liquidity providers.
    pub fee_rate_in_millionths: u16,

    /// Mint (token type) address of token A. For our pools: the wrapped-SOL mint.
    pub token_a_mint: PublicKeyBytes,
    /// Mint (token type) address of token B. For our pools: the USDC mint.
    pub token_b_mint: PublicKeyBytes,
    /// The pool's token account that actually holds its token A.
    pub token_a_vault: PublicKeyBytes,
    /// The pool's token account that actually holds its token B.
    pub token_b_vault: PublicKeyBytes,
    /// How many decimal places token A uses (SOL = 9, so 1 SOL = 1,000,000,000 lamports).
    pub token_a_decimals: u8,
    /// How many decimal places token B uses (USDC = 6, so 1 USDC = 1,000,000 micro-USDC).
    pub token_b_decimals: u8,

    /// Extra accounts only one DEX needs when building a swap instruction (Step 6).
    pub dex_specific_swap_accounts: DexSpecificSwapAccounts,

    /// Slot (Solana's block-height clock, ~400ms per slot) when this state was seen.
    pub slot: u64,
    /// Geyser's counter for account writes. Higher = newer.
    ///
    /// **Why it exists:** several writes to one account can land in the same
    /// slot, and messages can arrive out of order. The cache uses this number
    /// to throw away an older write that arrives after a newer one.
    pub geyser_write_version_for_ordering: u64,
}

/// Accounts a swap instruction needs that are not shared by every DEX.
///
/// **Why an enum:** the shared fields above are enough to *quote* a swap, but
/// each DEX's swap instruction also asks for a few accounts of its own. Keeping
/// them here means Step 6 never has to re-read the raw pool bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DexSpecificSwapAccounts {
    /// Orca derives everything else (oracle, tick arrays) from the pool address.
    OrcaWhirlpool,
    RaydiumClmm {
        /// The `amm_config` account: holds the fee tier shared by many pools.
        fee_config_address: PublicKeyBytes,
        /// The `observation_key` account: a price-history ring buffer the
        /// program updates on every swap (used for time-weighted prices).
        price_observation_address: PublicKeyBytes,
    },
}

impl ConcentratedLiquidityPoolState {
    /// Human-readable price: how many token B per one whole token A
    /// (e.g. "150.23 USDC per SOL"). Used only for logs, never for trading math.
    ///
    /// Steps: undo the Q64.64 scaling, square to get the raw price, then shift
    /// by the difference in decimals so lamports/micro-USDC become SOL/USDC.
    pub fn human_readable_price_token_b_per_token_a(&self) -> f64 {
        let sqrt_price = (self.sqrt_price_q64_64 as f64) / 2f64.powi(64);
        let raw_price = sqrt_price * sqrt_price;
        let decimals_shift = i32::from(self.token_a_decimals) - i32::from(self.token_b_decimals);
        raw_price * 10f64.powi(decimals_shift)
    }
}

/// Address of one tick-array account we have decided to watch.
///
/// **What it is:** a tick array is a separate on-chain account that stores a
/// fixed-size window of ticks for one pool. Its address is a PDA (program
/// derived address): an address computed from the pool address and the
/// window's starting tick, so anyone can calculate it without asking the chain.
///
/// **Where it comes from:** [`super::tick_array_pda_derivation`] computes these
/// for the windows around the pool's current price.
///
/// **Why the bot needs it:** when Geyser later sends us bytes for this address,
/// we look it up here to know which pool, DEX, and tick window the bytes belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickArrayPdaToWatch {
    /// The tick-array account's address (a PDA).
    pub tick_array_address: PublicKeyBytes,
    /// The pool this tick array belongs to.
    pub pool_address: PublicKeyBytes,
    /// Which DEX program derived this address.
    pub dex: DexProgram,
    /// First tick index covered by this array's window.
    pub start_tick_index: i32,
    /// The pool's tick spacing (needed by Orca's decoder to compute tick indexes).
    pub tick_spacing: u16,
}

/// One tick where active liquidity changes.
///
/// **What it is:** when a liquidity provider deposits into range
/// `[lower_tick, upper_tick]`, the DEX marks both boundary ticks as
/// "initialized" and records how much liquidity turns on or off there.
/// Ticks nobody used are skipped entirely.
///
/// **Why the bot needs it:** when a swap pushes the price across one of these
/// ticks, the pool's active liquidity jumps, which changes how fast the price
/// moves from then on. Quotes that ignore this are wrong for large trades.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializedTickWithLiquidityChange {
    /// The tick index (price = `1.0001^tick_index`).
    pub tick_index: i32,

    /// How much active liquidity changes when price moves *up* across this tick.
    ///
    /// **On-chain name:** `liquidity_net`.
    ///
    /// Example: Alice provides `L = 500` in range [100, 200].
    /// - At tick 100 this value is `+500` (going up, Alice's liquidity turns on).
    /// - At tick 200 it is `-500` (going up, Alice's liquidity turns off).
    ///
    /// Moving *down* across a tick applies the opposite sign.
    pub liquidity_added_when_price_crosses_upward: i128,
}

/// Decoded contents of one tick-array account: which ticks in its window are
/// initialized, and how much liquidity changes at each.
///
/// **Where it comes from:** Step 2 decodes it from account bytes that arrive
/// either from Geyser (live updates) or from RPC (the initial one-time load).
///
/// **Why the bot needs it:** quoting a large swap means "walking" the price
/// across ticks; this is the map of where the liquidity steps are.
#[derive(Debug, Clone)]
pub struct TickArrayAccountWithInitializedTicks {
    /// This tick-array account's address.
    pub tick_array_address: PublicKeyBytes,
    /// The pool this array belongs to.
    pub pool_address: PublicKeyBytes,
    /// Which DEX program owns it.
    pub dex: DexProgram,
    /// First tick index in this array's window.
    pub start_tick_index: i32,
    /// Only the initialized ticks (empty slots are dropped while decoding).
    pub initialized_ticks: Vec<InitializedTickWithLiquidityChange>,
    /// Slot when these bytes were seen.
    pub slot: u64,
    /// Geyser write counter (0 when loaded via RPC). See the pool field of the same name.
    pub geyser_write_version_for_ordering: u64,
}
