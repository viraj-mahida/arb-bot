# Glossary

Every term used in this codebase. Each entry gives:
- a plain definition
- an everyday comparison
- where the thing comes from
- where it appears in the code

Terms are grouped in the order you meet them when reading the steps.

---

## Solana and Web3 basics

**Blockchain**
- **What it is:** a shared database that thousands of computers (validators) keep identical copies of. Nobody can quietly change a past entry.
- **Everyday comparison:** a public notebook that everyone copies, so no one can forge it.

**Validator**
- **What it is:** a computer that runs the Solana software, executes transactions, and votes on which blocks are valid.

**Account**
- **What it is:** Solana stores *everything* in accounts: wallets, token balances, pools, tick arrays, even programs. Each account has an address and a byte array of data.
- **Where in code:** every Geyser message is "account X now has these bytes".

**Public key / address**
- **What it is:** the 32-byte name of an account, written in base58 for humans, for example `Czfq3xZZ…44zE`.
- **Where in code:** `PublicKeyBytes`, `solana_public_key_helpers.rs`

**Base58**
- **What it is:** a text encoding for bytes that uses letters and digits but skips look-alikes (`0`/`O`, `l`/`I`).

**Program (smart contract)**
- **What it is:** an account holding executable code. A DEX is a program. Only the program that owns a data account may change that account's bytes.
- **Where in code:** `known_program_and_pool_addresses.rs`

**PDA (Program Derived Address)**
- **What it is:** an address computed from "seeds" plus a program address, instead of being randomly generated. Anyone who knows the seeds can calculate it.
- **Everyday comparison:** a house number you can work out from the street name and plot number.
- **Where in code:** `tick_array_pda_derivation.rs`

**Anchor / discriminator**
- **What it is:** Anchor is the most common framework for writing Solana programs. It puts an 8-byte type tag, the *discriminator*, at the start of every account so its type can be recognized.
- **Where in code:** `*_account_decoder.rs`

**Little-endian**
- **What it is:** multi-byte numbers are stored lowest byte first. The number 400 as 2 bytes is `90 01`.
- **Where in code:** `read_little_endian_numbers.rs`

**Slot**
- **What it is:** Solana's clock tick, about 400 ms. At most one block is produced per slot.

**Commitment (`processed` / `confirmed` / `finalized`)**
- **What it is:** how sure you are that data will stick.
  - `processed`: one validator has executed it. This is the fastest.
  - `confirmed`: most of the network voted for it.
  - `finalized`: it can no longer be rolled back.
- **Where in code:** we read at `processed` for speed.

**Lamport**
- **What it is:** SOL's smallest unit. 1 SOL = 1,000,000,000 lamports.
- **Everyday comparison:** cents for dollars, but with 9 decimal places.

**Mint / decimals**
- **What it is:** a *mint* is the account that defines a token type (SOL, USDC, …). *Decimals* says how many decimal places the token uses: SOL uses 9, USDC uses 6. All on-chain amounts are whole numbers in the smallest unit.

**Token account / vault**
- **What it is:** a token account holds a balance of one mint. A pool's *vaults* are the token accounts that hold its reserves.

**RPC (Remote Procedure Call)**
- **What it is:** request/response access to a Solana node over HTTP, for example `getMultipleAccounts`, `sendTransaction`, or `simulateTransaction`.
- **Everyday comparison:** phoning the bank to ask for your balance.
- **Where in code:** `solana_rpc_client.rs`

**Geyser / Yellowstone gRPC**
- **What it is:** a validator plugin that pushes every account change to subscribers in real time. Yellowstone is its popular gRPC version.
- **Everyday comparison:** the bank texting you on every transaction, instead of you phoning in.
- **Where in code:** `geyser_grpc_client.rs`

**gRPC**
- **What it is:** a fast binary protocol for streaming messages between programs, and the transport Geyser uses.

**Write version**
- **What it is:** Geyser's counter for account writes. A higher number means a newer write. It is used to ignore messages that arrive out of order.
- **Where in code:** `geyser_write_version_for_ordering`

---

## Trading basics

**DEX (Decentralized Exchange)**
- **What it is:** a program where anyone can swap tokens without a company in the middle. Here: Orca Whirlpool and Raydium CLMM.
- **Where in code:** `DexProgram`

**Pool**
- **What it is:** an account holding two tokens (token A and token B) that anyone can swap against. The price comes from math, not from people placing orders.
- **Everyday comparison:** a vending machine stocked with two kinds of coins.
- **Where in code:** `ConcentratedLiquidityPoolState`

**Token A / token B**
- **What it is:** the DEX's names for a pool's two sides. In our pools A = SOL and B = USDC. Price is quoted as B per A (USDC per SOL).

**Swap**
- **What it is:** trading one token for another through a pool.

**Exact input**
- **What it is:** a swap where you fix how much you *send*, and the pool decides how much you *get*.
- **Where in code:** `quote_swap_exact_input`

**Quote**
- **What it is:** a prediction of a swap's output, computed locally with the DEX's own math and without sending anything.
- **Where in code:** `SwapQuoteForExactInput`

**Swap direction (`a_to_b`)**
- **What it is:**
  - A → B means selling token A. Its price in that pool goes *down*.
  - B → A means buying token A. Its price goes *up*.
- **Where in code:** `SwapDirection`

**Fee / basis point (bp, bps)**
- **What it is:** every swap pays a small percentage of its input to the pool's liquidity providers. 1 bp = 0.01%.
- **Where in code:** DEX programs store the fee in *millionths*, so 4 bps = 400 millionths = 0.04% (`fee_rate_in_millionths`).

**Liquidity provider (LP)**
- **What it is:** someone who deposits tokens into a pool so others can trade against them, and earns the fees in return.

**Price impact / slippage**
- **What it is:** your own trade moves the pool's price, so each extra unit you buy costs a bit more. *Slippage* also means the price moving between quoting and executing.

**Spread / price gap**
- **What it is:** the difference between two pools' prices for the same pair.

**Arbitrage**
- **What it is:** buying where something is cheap and selling where it is dear at the same moment, keeping the difference. On-chain, this pulls pool prices back in line.

**Round trip**
- **What it is:** a two-leg arbitrage.
  - Leg 1: start token → bridge token on the sell pool.
  - Leg 2: bridge token → start token on the buy pool.
- **Where in code:** `TwoPoolArbitrageRoundTrip`

**Bridge token**
- **What it is:** the token held between the two legs. It is USDC here, and it is the amount that links both pools during sizing.

**PnL (Profit and Loss)**
- **What it is:** what you end with minus what you started with.
- **Where in code:** `profit_in_start_token()`

**Marginal profit**
- **What it is:** the profit from one *more* tiny unit of trade. The best trade size is where marginal profit reaches zero. Total profit is highest at that point.

**Partial fill**
- **What it is:** a swap that could not use its whole input, because the cached liquidity ran out.

---

## AMM and concentrated liquidity math

**AMM (Automated Market Maker)**
- **What it is:** a pool that sets prices by formula.
- **The classic formula:** `x * y = k` (constant product). The product of the two reserves stays constant, so price = `y / x`.

**CLMM (Concentrated Liquidity Market Maker)**
- **What it is:** an AMM where each liquidity provider chooses a price range to provide liquidity in, instead of spreading it over all prices. This gives much deeper liquidity near the current price. Orca Whirlpool and Raydium CLMM both follow the Uniswap v3 design.

**Liquidity (`L`)**
- **What it is:** a number describing how deep the pool is at the current price. Inside one tick range, `L = sqrt(x * y)`. A bigger `L` means a trade moves the price less.
- **Where in code:** `active_liquidity_at_current_price`

**Tick**
- **What it is:** one step on the price ladder. Tick `i` means price `1.0001^i`, so one tick is a 0.01% price move.
- **Where in code:** `current_tick_index`

**Tick spacing**
- **What it is:** liquidity ranges may only start and end on every `tick_spacing`-th tick. Wider spacing means fewer boundaries to track.

**Initialized tick / `liquidity_net`**
- **What it is:** a tick where some liquidity range starts or ends. `liquidity_net` is how much active liquidity is added when the price crosses the tick moving upward. Crossing downward subtracts it.
- **Where in code:** `InitializedTickWithLiquidityChange`, `liquidity_added_when_price_crosses_upward`

**`liquidity_gross`**
- **What it is:** the total liquidity that references a tick. If it is zero, the tick is uninitialized.

**Tick array**
- **What it is:** an account (a PDA) holding a fixed window of tick slots for one pool. Orca uses 88 slots per array and Raydium uses 60.
- **Where in code:** `TickArrayAccountWithInitializedTicks`

**Square-root price / Q64.64**
- **What it is:** pools store `sqrt(price) * 2^64` as a whole number.
  - **Why the square root:** it turns the swap formulas into straight lines. With `s = sqrt(P)`, `y = L * s` and `x = L / s`.
  - **Why `2^64`:** on-chain math must be exact integers. It is the same trick as storing $12.34 as 1234 cents.
- **Where in code:** `sqrt_price_q64_64`, with the derivation in `concentrated_liquidity_formulas.rs`

**Crossing a tick**
- **What it is:** a swap pushing the price past an initialized tick. Active liquidity jumps by `liquidity_net`, so the price moves at a different rate afterwards.

**Rounding direction**
- **What it is:** round *down* what a pool pays out and round *up* what you pay in, so a quote never promises more than the chain delivers.

---

## Not built yet (roadmap)

**Instruction**
- **What it is:** one call into a program, for example "swap 1 SOL on this pool".

**Transaction**
- **What it is:** a signed bundle of instructions that all succeed or all fail. Both arbitrage legs go into one transaction so you never end up holding half a trade.

**Blockhash**
- **What it is:** a recent block's ID that every transaction must include. It proves the transaction is fresh and makes it expire after about a minute.

**Compute units / priority fee**
- **What it is:** transactions are metered in compute units. A priority fee (price per compute unit) gets you processed ahead of others.

**Minimum amount out**
- **What it is:** the slippage guard in a swap instruction. If the output would be below this amount, the whole transaction fails instead of losing money.

**Flash loan**
- **What it is:** borrowing tokens and repaying them inside the same transaction. If you cannot repay, the whole transaction reverts. This lets a bot trade with little capital of its own.

**MEV (Maximal Extractable Value)**
- **What it is:** profit that block producers or other bots can capture by reordering, inserting, or copying transactions, for example front-running your arbitrage.

**Jito bundle / tip**
- **What it is:** Jito's block engine accepts *bundles*, which are groups of transactions included in order, all or nothing. A *tip* is the payment for inclusion. Bundles protect arbitrage from being front-run.

**Simulation**
- **What it is:** a dry run of a transaction through RPC (`simulateTransaction`) to see if it would succeed, without sending it.
