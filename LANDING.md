# Landing

What the first real send taught us, and what has to change before this bot can take a busy SOL/USDC arb.

The quote is already fast. The send is not. On 29 Sep 2026 the Raydium 4bp pool (`3ucN`) broke at 19:11:57.756. Step 5 had the best size 5 ms later. By 19:11:57.843, about 90 ms after the break, the price had bounced and the gap was smaller than the fees. Jito accepted the bundle at 19:11:58.046, about 290 ms after the break. A Solana slot is about 400 ms, so "land this slot" was already too late for this pair. The bundle was dropped. The wallet paid nothing.

`submitted via jito bundle` is only a queue receipt. Line `[send] not landed (dropped or expired)` means the signature never became confirmed and never came back with a program error. If a validator had included it and the swap had reverted, the log would say `landed but FAILED`. Jito simulates a bundle first and throws away one that would revert, so a lost race costs no tip and no signature fee. Search `>>> SEND <<<` for the pair, the uncapped best size, the size that was actually sent, and the net.

## Two different goals

**Quieter pools, with the bot as it is.** Gaps on thin pools often sit there for a second or more. A few hundred milliseconds is enough to take those. Profit per trade is smaller, and a faster bot still takes the obvious ones. This is the set of pools the current program can win.

**SOL/USDC on Orca and Raydium, the pools watched today.** Those gaps die in tens of milliseconds. Catching one means the bundle is in a leader's hands that fast, not after a public RPC round trip and a distant HTTP post. The items below are that work.

## Send path

Today `build_and_send` in `src/step_6_build_and_send_transactions/arbitrage_trade_executor.rs` does this on every trade, after the decision:

1. `getLatestBlockhash` on `RPC_URL` (publicnode).
2. Build and sign.
3. HTTP `sendBundle` to `JITO_BLOCK_ENGINE_URL` (`https://mainnet.block-engine.jito.wtf`).

That is the ~290 ms. The 5 ms quote is not the problem.

+ **Cache the blockhash off the send path.** Refresh it in the background, about once a slot. The send should sign with the cached hash and return. A blockhash is valid for about 60 seconds, so a refresh every slot is plenty.
- **Use a regional Jito engine.** The default host is a global URL. Send to the region closest to the machine that runs the bot (`amsterdam`, `frankfurt`, `london`, `ny`, `slc`, `tokyo`, `singapore`, each as `https://<region>.mainnet.block-engine.jito.wtf`). From India that is Tokyo or Singapore, not the global host.

Pin one region for now: Singapore, if this machine stays in India. Ping Tokyo once and keep whichever is lower. Do not use the global host. It adds a hop before it picks a region for you.

Serious bots do not stay on that one region. They call Jito's GetNextScheduledLeader / GetConnectedLeadersRegioned and send the bundle to the region that the next leader is connected to. A Solana leader schedule alone does not say that region. Most stake sits in the US and Europe, so a Singapore-only send still waits for a forward when the leader is in New York or Frankfurt.

That routing only matters after the bot itself is near an engine. From India, the flight to Singapore is already most of the 290 ms. Moving the sender into Singapore or Tokyo beats swapping regions from home.
- **Use an RPC and a Geyser close to that same machine.** Detection time is the Geyser hop before the 5 ms of math. Public endpoints add both the detect delay and the blockhash delay.
+ **Keep `RPC_SIMULATION` false on this path.** A simulate-before-send is another full round trip. The chain's minimum-output check already reverts a stale trade, and Jito drops that bundle.
- **Do not hold the in-flight lock across confirmation.** `trade_in_flight` used to stay set until `wait_for_confirmation` finished (~60 s). Releasing it at the exact moment of submit is also wrong: the same quote is still cached, so the next pool update would send it again. The lock now covers build and submit only, then a 1 s cooldown. Confirmation still polls in the background.
- **Do not buy speed by raising `SLIPPAGE_TOLERANCE_BPS`.** It is 0, so a stale price reverts instead of landing a loss. The fix is to arrive while the quote is still true.

## Tip

`JITO_TIP_LAMPORTS=10000` is 0.00001 SOL. That does not win an auction on these pools, and on the trade we sent it was most of the expected profit (net was +0.000022 SOL after the tip and the signature fee).

Size the tip from the expected profit of that trade, and skip the trade when the tip required to land it eats the profit. A flat 10,000 lamports is a placeholder.

## Size and funding

The send was Raydium 1bp (`8sLb`) → Raydium 4bp (`3ucN`), cut to 0.05 SOL by `MAX_TRADE_INPUT_LAMPORTS=50000000`. The uncapped best size on that same tick was **1.2829 SOL**, pool profit **+0.000487 SOL**. The other two approved routes were smaller nets after the same cap, so they were not sent. The 99.53 SOL Orca 4bp quote was math only.

- The 0.05 SOL cap is what made the sent trade tiny. Raising it only helps if the wallet or a loan can fund the new size. Leave room for the signature, the tip, and a one-time USDC token-account rent.
- `FUNDING_MODE=wallet`. A flash loan is what removes the wallet-balance ceiling. It does not remove `MAX_TRADE_INPUT_LAMPORTS`.
- The old `FLASH_LOAN_FEE_BPS` default of 10 was wrong: the Kamino reserve fee is 1 bp, and the default is now 1. At 1 bp, the uncapped 1.2829 SOL Raydium → Raydium quote nets about **+0.000344 SOL** after the loan fee, the signature, and the 10,000 lamport tip. At 10 bp that same quote is a loss. The 99.53 SOL quote at 1 bp nets about **+0.0167 SOL** before a competitive tip. Re-check the reserve before trusting the constant.
- Flash mode still needs `KAMINO_LENDING_MARKET`, `KAMINO_SOL_RESERVE`, and an address lookup table (`ADDRESS_LOOKUP_TABLES` is empty). Without the table the flash-loan transaction is likely too big.

## Order to do this

1. Quieter pools if the goal is a landed profit with the bot as it is.
2. Cached blockhash, regional Jito, closer RPC and Geyser.
3. Tip taken from expected profit.
4. Stop holding the in-flight lock across the confirm poll.
5. Set `FLASH_LOAN_FEE_BPS` to the reserve's real fee, then turn on flash mode with the Kamino accounts and a lookup table, and only then raise `MAX_TRADE_INPUT_LAMPORTS`.
