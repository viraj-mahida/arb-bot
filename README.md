# arb-bot

Educational Solana arbitrage watcher. It watches SOL/USDC pools on Orca and Raydium, quotes a round trip with each pool's own math, and sends only when `SEND_TRANSACTIONS=true`.

The folders are a reading order: [PIPELINE.md](PIPELINE.md). How the process is wired: [ARCHITECTURE.md](ARCHITECTURE.md). Terms: [GLOSSARY.md](GLOSSARY.md). Settings: `.env.example`.

## How to read the code

Two function prefixes. A name with neither prefix is a helper for the `main_` function in that file.

| Prefix | Meaning |
|---|---|
| `main_` | The job of that file. Start here. The comment at the top of the file names it. |
| `ignr_` | Not the trade. A log, the visualizer, or the demo price shift. Skip it when following the bot. |

An `ignr_` call in a step file is one line. The work is in `print_logs/` or `dashboard_events/`. A comment on that line means it also touches the bot (demo mode turns sending off) or the name alone does not say why it is there.

`print_logs::` without `ignr_` is still only output. The prefix is for a call that would otherwise look like trading logic, such as remembering route costs for the visualizer.

Folder prefix `step_1_` through `step_6_` is the same reading order as the pipeline. A `functions/` folder under a step holds helpers, not another stage. Tests live in `src/tests/` and are not part of the bot path.
