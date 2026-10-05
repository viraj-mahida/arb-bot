# arb-bot

A Solana bot that watches six SOL/USDC pools on Orca and Raydium. When one pool's price is higher than another's, it quotes a round trip: sell SOL on the higher pool, buy that SOL back on the lower one. It sends the trade only if you turn sending on.

Sending is off by default. A quote that looks profitable can still lose money once it is on chain: other bots take the same gap, the transaction can fail to land, and you still pay fees on the ones that do. Read `.env.example` before you change anything.

## Try the visualizer

The visualizer is a separate page. It does not need a wallet.

Sample data, nothing on chain:

```bash
cd dashboard
npm install
npm run dev
```

Open http://localhost:5173/?preview

Live mainnet, after the bot below is running with `DASHBOARD=true`:

Open http://localhost:5173/

The page reads `ws://127.0.0.1:8787/ws`.

## Watch mainnet, without sending

You need a recent stable Rust (1.85 or newer) and your own Solana RPC URL plus a Yellowstone Geyser gRPC endpoint.

```bash
cp .env.example .env
```

Fill in `RPC_URL`, `GRPC_URL`, and `X_TOKEN`. Leave `WALLET_KEYPAIR_PATH` empty and leave `SEND_TRANSACTIONS=false`.

```bash
cargo run
```

The terminal prints each pool update and whether the gap is larger than both pool fees. With no wallet, that is the whole run: watch, decode, cache, quote.

`.env` and `bot-keypair.json` are gitignored. Do not commit a keypair or an API token.

## What a quote checks

1. Is the price gap bigger than both pool fees? If not, stop.
2. Walk the tick arrays and find the SOL size that uses the gap.
3. Quote that size with each pool's own swap math. If less SOL comes back, stop.
4. Subtract the signature fee, the Jito tip or the priority fee, and the minimum profit.

`FUNDING_MODE=wallet` uses SOL already in the wallet. `flash_loan` borrows it inside the transaction from Jupiter Lend (fee is 0, and the bot checks the live fee at startup) or from Kamino. A flash loan needs an address lookup table: `cargo run -- create-lookup-table`, then paste the address into `ADDRESS_LOOKUP_TABLES`.

## Sending

Set `WALLET_KEYPAIR_PATH` to a Solana keypair JSON file. Keep `SEND_TRANSACTIONS=false` until a quote is one you would actually pay for.

`SEND_TRANSACTIONS=true` signs and sends. The default route is a Jito bundle (`JITO_BLOCK_ENGINE_URL`). An empty Jito URL sends through RPC instead. The two are not sent together. `RPC_SIMULATION=true` dry-runs the transaction on your RPC before any send.

`MAX_TRADE_INPUT_LAMPORTS` defaults to 1 SOL. `MIN_PROFIT_LAMPORTS` defaults to 10,000 lamports. Every other knob is in `.env.example`.

## How to read the code

The folders are a reading order: [PIPELINE.md](PIPELINE.md). How the process is wired: [ARCHITECTURE.md](ARCHITECTURE.md). Terms: [GLOSSARY.md](GLOSSARY.md).

Two function prefixes. A name with neither prefix is a helper for the `main_` function in that file.

| Prefix | Meaning |
|---|---|
| `main_` | The job of that file. Start here. The comment at the top of the file names it. |
| `ignr_` | Not the trade. A log, the visualizer, or the demo price shift. Skip it when following the bot. |

An `ignr_` call in a step file is one line. The work is in `print_logs/` or `dashboard_events/`. A comment on that line means it also touches the bot (demo mode turns sending off) or the name alone does not say why it is there.

`print_logs::` without `ignr_` is still only output. The prefix is for a call that would otherwise look like trading logic, such as remembering route costs for the visualizer.

Folder prefix `step_1_` through `step_6_` is the same reading order as the pipeline. A `functions/` folder under a step holds helpers, not another stage. Tests live in `src/tests/` and are not part of the bot path.
