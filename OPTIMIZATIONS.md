# Optimizations

Notes from the landing and send-path discussion. What is already in the bot, what is not worth doing, and what to do later.

## Already done

- **Blockhash.** `blocks_meta` rides on the same Geyser connection as account updates. Trades read that cache. HTTP `getLatestBlockhash` runs only if the cache is empty or older than 30 seconds.
- **One gRPC connection.** Every tick-array resubscribe sends `accounts` and `blocks_meta` together. A new subscribe request replaces the old filters, so leaving `blocks_meta` off would drop the blockhash feed. A second connection would be billed again for the same data.
- **In-flight lock.** Held only while building and submitting. Then a 1-second cooldown, so the same cached quote is not sent again on the next pool update. Confirmation still polls in the background and does not block the next trade. Releasing the lock at the exact moment of submit would resend the same bundle. Holding it until confirmation (~60 s) skips every later gap.
- **Slippage.** `SLIPPAGE_TOLERANCE_BPS` is 0. Leave it. A higher value lets a stale price land, including a loss. Speed is arriving while the quote is still true.

## Not worth doing

- Lookup-table creation can keep using HTTP RPC. It is a one-off command, and each transaction waits on confirmation. The blockhash call is noise next to that.
- The startup blockhash seed is one gRPC `GetLatestBlockhash` on the same connection. `blocks_meta` takes over within about one slot. Removing the seed does not speed up trades.
- Startup HTTP RPC is the wallet balance, each address-lookup table, and one flash-loan account when flash loans are on. None of that is the send path.

## Do next, no new servers

- **Jito region.** Stop using `https://mainnet.block-engine.jito.wtf`. That host adds a hop, then picks a region. From India, set `JITO_BLOCK_ENGINE_URL` to Singapore (`https://singapore.mainnet.block-engine.jito.wtf`), ping Tokyo once, and keep whichever is lower.
- **Dynamic tip.** Replace the flat 10,000 lamports.
  1. Quote the trade. Subtract the signature, the flash-loan fee, and `MIN_PROFIT_LAMPORTS`. What remains is the most the tip can be.
  2. Tip = a share of that remainder. About half is the right default. Contested searchers give up roughly 50–70% when they decide to win.
  3. Also read `https://bundles.jito.wtf/api/v1/bundles/tip_floor` (or the websocket tip stream). For a normal send, bid near the 50th or 75th percentile. For a fight, bid at least the 95th, and still no more than the profit share.
  4. Skip the trade if the tip would leave less than `MIN_PROFIT_LAMPORTS`.
  5. Keep the tip inside leg 2's minimum output, so a swap that cannot pay the tip reverts.
- The tip is a SOL transfer inside the same transaction. It is paid only if the bundle lands and the transaction succeeds. Jito drops a bundle that would revert, so a lost race pays no tip and no signature fee.
- The auction is a sealed bid. You cannot see other tips in the slot and raise yours. To bid more you must sign a new transaction.
- Percentiles are tips that already landed, republished all day. They are a live price list, not a chance of landing. The 75th does not mean a 75% chance. Losers are missing from the list. The 95th and 99th are the fights; a busy SOL/USDC race sits there, and half of a tiny quote is still a tiny tip.
- The auction ranks tip against compute units. A smaller transaction with the same tip beats a heavier one.
- Keep `RPC_SIMULATION` false. A simulate-before-send is another round trip. The on-chain minimum output already reverts a stale trade.

## Later, on the hot path

The tick walk and the quotes are microseconds. These two sit between a pool update and the trade decision, and both wait on something slower than the math.

- **The tick-array load blocks the stream.** When the price moves into tick arrays the bot is not watching yet, `main_handle_pool_account_update` waits on the Geyser resubscribe and the RPC `getMultipleAccounts` before it quotes. Nothing else from the stream is processed until both finish, and that is when the price is moving. Quote and decide first from the tick arrays already in the cache. A quote that needs a missing array already fails with `TickArrayForCurrentPriceNotCachedYet`. Then `tokio::spawn` the subscribe and the RPC load. The cache is already `Arc` plus locks, so the background task can write into it.
- **Logs run before the decision, and each line waits on disk.** `pool_snapshot` and `print_arbitrage_quotes` run before `main_consider_trading`. Every line is a `println!` plus a write and `flush()` in `append_to_file`. Call `main_consider_trading` first. Wrap the log file in a `BufWriter`, or send lines to a background thread through a channel, so a flush is not on the trading path.

## When the machine can move

Sending only to Singapore does not wait for a validator in Singapore, and it does not only run when that city produces the block. Each Jito leader is tied to the regional engine its relayer uses. The Singapore engine auctions immediately for leaders connected there. For a leader connected to New York, it must forward the bundle, and that forward is usually too slow. The current slot's auction is already closed by the time the bundle arrives. The bundle is for the next leader. If it misses that leader too, it can still be tried on the one after, until it expires. It is not held for a later Singapore leader.

- **One server.** Call `GetNextScheduledLeader` / `GetConnectedLeadersRegioned` and send to the region that the next leader is connected to. A Solana leader schedule alone does not say the region. Most stake is in the US and Europe, so a Singapore-only send still waits on a forward when the leader is in New York or Frankfurt. This only matters after the bot itself is near an engine. From India, the flight to Singapore is already most of the delay.
- **Three servers,** in New York, Frankfurt, and Tokyo. Those cover most leaders. Singapore matters only while the only machine is in India. Each box uses a Yellowstone endpoint in that same city and sends only to that city's Jito engine. The quote and the send both happen on that box. The box next to the next leader wins, even if another box saw the pool a few milliseconds earlier. Jito drops the late copies when they would revert, so the extra shots do not double-fill. Looking up the next leader is the right trick for a single server. Once a server is in each region, sending locally is that lookup.
- **Local Geyser** means a hosted Yellowstone in that city, the same `GRPC_URL` setup pointed at a nearby region. It does not mean installing Yellowstone yourself. Your own Solana node in that datacenter is a later, small gain and needs a large machine. Shreds arrive earlier than account updates, while the block is still being built. This bot reads pool accounts, not shreds. Decoding shreds is a different pipeline; do it after regional Yellowstone is in place.
- Cost is thousands of dollars a month for a few servers and feeds, not millions, and not a validator. It still does not make you first on busy SOL/USDC. Those gaps die in tens of milliseconds. Quieter pools, where a gap lasts a second or more, are the ones this bot can take without that fleet.

## Do not chase

- You do not need to own a validator or know the operators. During its own slots a validator can take an arbitrage itself. Those slots are a fraction of the day unless it has a very large stake. Every other slot is an auction.
- Validators are not paid mainly for running a searcher bot. The steady income is commission on inflation rewards (new SOL minted for stakers; the validator keeps a cut). While they are leader they also earn priority fees and a commission on other people's Jito tips. Most of each tip is passed to their stakers.
- Nobody wins every slot. The winner of one slot is whoever is closest to that leader and bids enough. The next slot is often a different city and a different bot.
