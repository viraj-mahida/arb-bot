# Pipeline

The numbered folders are a **reading order**. At runtime they nest: every Geyser write goes through listen → decode → store, and only a **pool** update then runs size/quote and maybe trade.

```mermaid
flowchart TD
  startup["main.rs: load .env, watch Orca + Raydium SOL/USDC"]
  conn["0. solana_connections<br/>Geyser stream + RPC HTTP"]
  loop["1.1 Loop forever on Geyser messages"]

  startup --> conn --> loop

  loop --> kind{What account changed?}

  kind -->|pool price or liquidity| p12["1.2 Handle pool"]
  kind -->|tick-array liquidity map| p13["1.3 Handle tick array"]

  p12 --> d2["2.1 Decode bytes → pool or tick structs"]
  p13 --> d2
  d2 --> s3["3.1 Store latest in the in-memory cache"]

  p12 --> maybeNew["Need new tick arrays near this price?"]
  maybeNew -->|yes, first time| rpc["RPC getMultipleAccounts<br/>Geyser only sends future writes"]
  rpc --> s3
  maybeNew -->|no| s3

  p13 --> loop
  s3 -->|tick array only| loop

  s3 -->|pool update| pair["5.2 Quote both directions<br/>Orca→Raydium and Raydium→Orca"]
  pair --> size["5.1 Walk both tick books → best size N"]
  size --> q1["4.1 Quote one swap of N"]
  q1 --> q2["4.2 Chain two quotes = round trip"]
  q2 --> logs["print_logs: spread / best size"]

  logs --> tradeQ{Wallet configured?}
  tradeQ -->|no| loop
  tradeQ -->|yes| e64["6.4 Executor: one trade at a time"]
  e64 --> d61["6.1 Decide: stale? costs? min profit?"]
  d61 -->|skip| loop
  d61 -->|approve| d62["6.2 Assemble + sign v0 tx"]
  d62 --> d63["6.3 Simulate on RPC"]
  d63 -->|SEND_TRANSACTIONS=false| loop
  d63 -->|true| send["Send Jito bundle or RPC, then confirm"]
  send --> loop
```

## How to read it

| When | What happens |
|---|---|
| Once | Connect, subscribe to the two pools, maybe load a wallet (`6.4 prepare`) |
| Every Geyser write | `1.1` → decode `2` → cache `3` |
| Tick-array write | Stop after cache. No quote. |
| Pool write | Then size `5` uses quote `4`. That is one job: *given live books, what N and what out?* |
| After that | `6.1` subtracts **network / priority / tip / flash** (DEX fees already in `4`–`5`). Simulate is always on; send only if `SEND_TRANSACTIONS=true` |

`print_logs` is not a step. Helpers under each `functions/` folder are DEX-specific or instruction builders, not extra pipeline stages.

## Folders

| Step | Folder | Sub-steps |
|---|---|---|
| 0 | `src/solana_connections` | Geyser gRPC, RPC HTTP, Jito |
| 1 | `src/step_1_listen_to_account_updates` | 1.1 loop, 1.2 pool, 1.3 tick array |
| 2 | `src/step_2_decode_account_bytes` | 2.1 pick Orca or Raydium decoder |
| 3 | `src/step_3_store_latest_pool_state` | 3.1 in-memory cache |
| 4 | `src/step_4_quote_swaps` | 4.1 one swap, 4.2 round trip |
| 5 | `src/step_5_find_best_arbitrage_size` | 5.1 best size, 5.2 both directions |
| 6 | `src/step_6_build_and_send_transactions` | 6.1 decide, 6.2 assemble, 6.3 simulate/send, 6.4 executor |
