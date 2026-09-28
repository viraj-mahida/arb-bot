import { create } from "zustand";
import { blipGeyser, blipStamp, blipSwap } from "./audio";
import layout from "./scene/layout.json";
import { poolFeet, poolTitle } from "./scene/pools";
import type { Actor, BotEvent, Carry, CurvePoint, LaptopScreen, LinkMode, LogLine, PoolBoard, Pose, Stamp } from "./types";

const spots = layout.spots;

type Rbot = {
  x: number;
  y: number;
  pose: Pose;
  carry: Carry;
  bubble: string;
};

type CityState = {
  link: LinkMode;
  demoBps: number;
  sendTransactions: boolean;
  simulate: boolean;
  funding: string;
  wallet: string | null;
  walletSol: number | null;
  slot: number;
  gapBps: number | null;
  feesBps: number | null;
  beatsFees: boolean;
  pools: Record<string, PoolBoard>;
  chart: { time: number; prices: Record<string, number> }[];
  curve: CurvePoint[];
  curveSell: string | null;
  curveBuy: string | null;
  terminal: string[];
  terminalPulse: number;
  deskPulse: Record<string, number>;
  log: LogLine[];
  seen: number;
  skipped: number;
  simulated: number;
  bestProfitSol: number | null;
  instructions: string[];
  stepIndex: number;
  rbot: Rbot;
  laptop: LaptopScreen | null;
  stamp: Stamp | null;
  actors: Actor[];
  coins: number;
  take: BotEvent[];
  setLink: (link: LinkMode) => void;
  apply: (event: BotEvent) => void;
};

const seenIds = new Set<number>();
let logId = 1;
let actorId = 1;
let epoch = 0;
let queue: Promise<void> = Promise.resolve();
let scriptBusy = false;
let trading = false;
let lastSkipAt = 0;
let lastWhaleAt = 0;
let lastGeyserSound = 0;
let pendingResult: Stamp | null = null;
let routeReachesJito = false;
let awaitChainResult = false;

function poolName(id: string | undefined): string {
  if (!id) return "pool";
  return poolTitle(useCity.getState().pools[id], id);
}

function spotFor(id: string | undefined) {
  if (!id) return spots.raydium;
  if (!id.includes(":")) return id.includes("orca") ? spots.orca : spots.raydium;
  return poolFeet(id);
}

function prettyDirection(direction: string | undefined): string {
  if (!direction) return "";
  return direction
    .split("→")
    .map((part) => poolName(part))
    .join(" → ");
}

function widestGap(pools: PoolBoard[]): { gapBps: number; feesBps: number } | null {
  if (pools.length < 2) return null;
  let best: { gapBps: number; feesBps: number } | null = null;
  for (let left = 0; left < pools.length; left += 1) {
    for (let right = left + 1; right < pools.length; right += 1) {
      const a = pools[left];
      const b = pools[right];
      if (!a || !b) continue;
      const mid = (a.price + b.price) / 2;
      if (mid <= 0) continue;
      const gapBps = (Math.abs(a.price - b.price) / mid) * 10_000;
      const feesBps = a.feeBps + b.feeBps;
      if (!best || gapBps - feesBps > best.gapBps - best.feesBps) best = { gapBps, feesBps };
    }
  }
  return best;
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function runScript(exclusive: boolean, fn: (alive: () => boolean) => Promise<void>) {
  if (exclusive) epoch += 1;
  const mine = epoch;
  const alive = () => mine === epoch;
  queue = queue.then(() => fn(alive)).catch((error) => console.error(error));
}

async function walk(alive: () => boolean, x: number, y: number, pose: Pose, bubble?: string) {
  if (!alive()) return;
  useCity.setState((state) => ({
    rbot: { ...state.rbot, x, y, pose, bubble: bubble ?? state.rbot.bubble },
  }));
  await sleep(720);
}

function signedSol(value: number) {
  return `${value >= 0 ? "+" : ""}${value.toFixed(6)}`;
}

function showLaptop(laptop: LaptopScreen | null) {
  useCity.setState({ laptop });
}

async function goHome(alive: () => boolean) {
  if (!alive()) return;
  showLaptop(null);
  useCity.setState((state) => ({
    stamp: null,
    rbot: { ...state.rbot, x: spots.screen.x, y: spots.screen.y, pose: "walk", carry: null, bubble: "" },
  }));
  await sleep(720);
  if (!alive()) return;
  useCity.setState((state) => ({ rbot: { ...state.rbot, pose: "idle", bubble: "watching for a gap" } }));
}

async function playRoute(
  alive: () => boolean,
  plan: {
    sellDex?: string;
    buyDex?: string;
    flashLoan: boolean;
    grossSol?: number;
    feeSol?: number;
    tipSol?: number;
    prioritySol?: number;
    flashSol?: number;
    minProfitSol?: number | null;
    netSol?: number;
    worthIt: boolean;
    known: boolean;
    partial?: boolean;
    local: boolean;
    instructions: string[];
  },
) {
  const sell = spotFor(plan.sellDex);
  const buy = spotFor(plan.buyDex);
  const pair = prettyDirection(`${plan.sellDex}→${plan.buyDex}`);
  useCity.setState({ instructions: plan.instructions, stepIndex: -1, stamp: null, laptop: null });
  const setStep = (stepIndex: number, bubble: string, carry: Carry, pose: Pose = "carry") => {
    useCity.setState((state) => ({ stepIndex, rbot: { ...state.rbot, bubble, carry, pose } }));
  };

  await walk(alive, spots.screen.x, spots.screen.y, "walk", pair || "a gap on the desks");
  if (!alive()) return;

  await walk(alive, spots.laptop.x, spots.laptop.y, "walk", "opening my laptop");
  if (!alive()) return;
  showLaptop({
    mode: "costs",
    known: plan.known,
    grossSol: plan.grossSol ?? 0,
    feeSol: plan.feeSol ?? 0,
    tipSol: plan.tipSol ?? 0,
    prioritySol: plan.prioritySol ?? 0,
    flashSol: plan.flashSol ?? 0,
    minProfitSol: plan.minProfitSol ?? null,
    netSol: plan.netSol ?? 0,
    worth: plan.worthIt,
  });
  const verdict = !plan.known
    ? "no fee on this quote"
    : plan.partial
      ? "partial fill, leaving it"
      : plan.worthIt
        ? "still worth sending"
        : "not enough after the tip";
  useCity.setState((state) => ({ rbot: { ...state.rbot, pose: "idle", bubble: verdict } }));
  await sleep(1800);
  if (!alive()) return;

  if (!plan.worthIt) {
    useCity.setState({ stepIndex: -1 });
    await goHome(alive);
    return;
  }

  showLaptop(null);
  if (plan.flashLoan) {
    setStep(1, "borrowing SOL from the bank", "bag");
    await walk(alive, spots.bank.x, spots.bank.y, "carry");
  } else {
    setStep(1, "taking SOL from the locker", "bag");
    await walk(alive, spots.locker.x, spots.locker.y, "carry");
  }
  if (!alive()) return;

  setStep(2, `selling SOL at ${poolName(plan.sellDex)}`, "bag");
  await walk(alive, sell.x, sell.y, "carry");
  if (!alive()) return;

  setStep(3, `buying SOL back at ${poolName(plan.buyDex)}`, "bag");
  await walk(alive, buy.x, buy.y, "carry");
  if (!alive()) return;

  showLaptop({ mode: "build", lines: plan.instructions });
  setStep(4, "stacking the instructions", null, "idle");
  await walk(alive, spots.laptop.x, spots.laptop.y, "walk");
  await sleep(1100);
  if (!alive()) return;
  showLaptop(null);

  setStep(5, "posting the signed envelope", "envelope");
  await walk(alive, spots.jito.x, spots.jito.y, "carry");
  if (!alive()) return;

  const chain = awaitChainResult || !plan.local;
  const deadline = Date.now() + (chain ? 2200 : 400);
  while (!pendingResult && Date.now() < deadline) await sleep(80);
  if (!alive()) return;
  const net = plan.netSol;
  const stamp = pendingResult ?? {
    text: chain ? "SIGNED" : "LOCAL QUOTE",
    tone: "neutral" as const,
    detail: net != null ? `net ${signedSol(net)} SOL` : "built from the local quote",
  };
  pendingResult = null;
  blipStamp(stamp.tone !== "bad");
  useCity.setState((state) => ({
    stamp,
    coins: state.coins + (stamp.tone === "bad" ? 0 : 1),
    rbot: { ...state.rbot, pose: "celebrate", carry: null, bubble: stamp.detail },
  }));
  await sleep(1600);
  if (!alive()) return;
  await goHome(alive);
  if (!alive()) return;
  useCity.setState((state) => ({ stepIndex: state.instructions.length }));
}

function pushLog(tag: string, text: string, tone: string) {
  const line: LogLine = { id: logId++, tag, text, tone };
  useCity.setState((state) => ({ log: [line, ...state.log].slice(0, 40) }));
}

function spawnActor(actor: Omit<Actor, "id">, deskX: number, deskY: number) {
  const id = `a${actorId++}`;
  const created: Actor = { ...actor, id };
  useCity.setState((state) => ({ actors: [...state.actors, created] }));
  window.setTimeout(() => {
    useCity.setState((state) => ({
      actors: state.actors.map((item) => (item.id === id ? { ...item, x: deskX, y: deskY } : item)),
    }));
  }, 40);
  window.setTimeout(() => {
    useCity.setState((state) => ({
      actors: state.actors.map((item) =>
        item.id === id ? { ...item, x: spots.entrance.x - 80, y: spots.entrance.y } : item,
      ),
    }));
  }, 1700);
  window.setTimeout(() => {
    useCity.setState((state) => ({ actors: state.actors.filter((item) => item.id !== id) }));
  }, 2700);
}

const defaultInstructions = [
  "set compute budget",
  "fund the trade",
  "sell SOL on the expensive desk",
  "buy SOL on the cheap desk",
  "repay funding",
  "post the signed envelope",
];

export const useCity = create<CityState>((set, get) => ({
  link: "connecting",
  demoBps: 0,
  sendTransactions: false,
  simulate: false,
  funding: "wallet",
  wallet: null,
  walletSol: null,
  slot: 0,
  gapBps: null,
  feesBps: null,
  beatsFees: false,
  pools: {},
  chart: [],
  curve: [],
  curveSell: null,
  curveBuy: null,
  terminal: ["geyser link idle", "waiting for account writes…"],
  terminalPulse: 0,
  deskPulse: {},
  log: [],
  seen: 0,
  skipped: 0,
  simulated: 0,
  bestProfitSol: null,
  instructions: defaultInstructions,
  stepIndex: -1,
  rbot: { x: spots.screen.x, y: spots.screen.y, pose: "idle", carry: null, bubble: "watching for a gap" },
  laptop: null,
  stamp: null,
  actors: [],
  coins: 0,
  take: [],
  setLink: (link) => set({ link }),
  apply: (event) => {
    if (event.id != null) {
      if (seenIds.has(event.id)) return;
      seenIds.add(event.id);
    }
    set((state) => ({ take: [...state.take, event].slice(-2000) }));

    switch (event.type) {
      case "status":
        set({
          demoBps: event.demoBps ?? 0,
          sendTransactions: Boolean(event.sendTransactions),
          simulate: Boolean(event.simulate),
          funding: event.funding ?? "wallet",
          wallet: event.wallet ?? get().wallet,
          walletSol: event.walletSol ?? get().walletSol,
        });
        pushLog("LINK", event.demoBps ? `demo +${event.demoBps} bps on Orca` : "live prices", "info");
        break;
      case "wallet":
        set({ walletSol: event.walletSol ?? null });
        break;
      case "log":
        pushLog((event.kind ?? "log").toUpperCase(), event.line ?? "", "info");
        break;
      case "geyser": {
        const line = event.line ?? "";
        set((state) => ({
          terminal: [...state.terminal, line].slice(-8),
          terminalPulse: Date.now(),
          slot: event.slot && event.slot > 0 ? event.slot : state.slot,
        }));
        if (event.kind === "pool" && Date.now() - lastGeyserSound > 700) {
          lastGeyserSound = Date.now();
          blipGeyser();
        }
        break;
      }
      case "swap": {
        const pool = event.pool ?? event.dex;
        const solIn = event.direction === "solToUsdc";
        pushLog("SWAP", `${poolName(pool)} ${solIn ? "SOL → USDC" : "USDC → SOL"}  ${event.price?.toFixed(4) ?? ""}`, "swap");
        blipSwap();
        set((state) => ({
          deskPulse: { ...state.deskPulse, [pool ?? ""]: Date.now() },
        }));
        const visitors = get().actors.filter((actor) => actor.kind === "visitor").length;
        if (visitors < 2) {
          const desk = spotFor(pool);
          spawnActor(
            {
              kind: "visitor",
              x: spots.entrance.x,
              y: spots.entrance.y,
              color: solIn ? "#7ee0ff" : "#ffd56a",
              label: "trader",
              bag: solIn ? "sol" : "usdc",
            },
            desk.x + (pool?.includes("orca") ? -40 : 40),
            600,
          );
        }
        break;
      }
      case "liquidity": {
        const pool = event.pool ?? event.dex;
        pushLog("LP", `${poolName(pool)} liquidity moved${event.tickCount ? ` · ${event.tickCount} ticks` : ""}`, "lp");
        if (get().actors.some((actor) => actor.kind === "whale") || Date.now() - lastWhaleAt < 8000) break;
        lastWhaleAt = Date.now();
        const desk = spotFor(pool);
        spawnActor(
          {
            kind: "whale",
            x: spots.entrance.x + 40,
            y: spots.entrance.y,
            color: "#d7c4a3",
            label: "LP",
            bag: null,
          },
          desk.x,
          615,
        );
        break;
      }
      case "boards": {
        const pools: Record<string, PoolBoard> = { ...get().pools };
        for (const pool of event.pools ?? []) {
          const id = pool.id || pool.dex;
          pools[id] = { ...pool, id };
        }
        const prices: Record<string, number> = {};
        for (const pool of Object.values(pools)) prices[pool.id] = pool.price;
        const widest = widestGap(Object.values(pools));
        const stampMs = event.t ?? Date.now();
        const time = Math.floor((stampMs > 1_000_000_000_000 ? stampMs : Date.now()) / 1000);
        const chart = [...get().chart];
        const point = { time, prices };
        const last = chart[chart.length - 1];
        if (last && last.time === time) chart[chart.length - 1] = { time, prices: { ...last.prices, ...prices } };
        else chart.push(point);
        const slot = Math.max(get().slot, ...(event.pools ?? []).map((pool) => pool.slot));
        set({
          pools,
          chart: chart.slice(-360),
          gapBps: widest?.gapBps ?? event.gapBps ?? null,
          feesBps: widest?.feesBps ?? event.feesBps ?? null,
          beatsFees: widest ? widest.gapBps > widest.feesBps : Boolean(event.beatsFees),
          slot,
        });
        break;
      }
      case "quote": {
        set((state) => ({ seen: state.seen + 1 }));
        const profit = event.profitSol ?? 0;
        const netNote = event.netSol == null ? "" : `  net ${signedSol(event.netSol)}`;
        pushLog(
          "QUOTE",
          `${prettyDirection(event.direction)}  ${signedSol(profit)} SOL${netNote}`,
          event.worthIt ? "good" : "dim",
        );
        if (!event.profitable || trading || scriptBusy) break;
        const direction = event.direction ?? "";
        const [sellDex, buyDex] = direction.split("→");
        const known = event.feeSol != null && event.netSol != null;
        const worthIt = known && event.worthIt === true;
        if (worthIt) routeReachesJito = true;
        scriptBusy = true;
        const flashLoan = get().funding === "flash_loan";
        runScript(false, async (alive) => {
          try {
            await sleep(380);
            if (!alive()) return;
            await playRoute(alive, {
              sellDex,
              buyDex,
              flashLoan,
              grossSol: event.profitSol,
              feeSol: event.feeSol,
              tipSol: event.tipSol,
              prioritySol: event.prioritySol,
              flashSol: event.flashSol,
              minProfitSol: event.minProfitSol,
              netSol: event.netSol,
              worthIt,
              known,
              partial: event.partial,
              local: true,
              instructions: [
                "set compute budget",
                flashLoan ? "borrow SOL from the bank" : "open the locker and wrap SOL",
                `sell SOL at ${poolName(sellDex)}`,
                `buy SOL at ${poolName(buyDex)}`,
                flashLoan ? "repay the bank" : "unwrap SOL back into the locker",
                "post the signed envelope (Jito tip)",
              ],
            });
          } finally {
            scriptBusy = false;
            routeReachesJito = false;
            if (trading) trading = false;
            awaitChainResult = false;
          }
        });
        break;
      }
      case "curve":
        set({
          curve: event.points ?? [],
          curveSell: event.sellPool ?? null,
          curveBuy: event.buyPool ?? null,
        });
        break;
      case "skip": {
        set((state) => ({ skipped: state.skipped + 1 }));
        const reason = event.reason ?? "skipped";
        if (reason.length < 80) pushLog("SKIP", `${prettyDirection(event.direction)}  ${reason}`, "warn");
        if (trading || scriptBusy || Date.now() - lastSkipAt < 4500) break;
        lastSkipAt = Date.now();
        scriptBusy = true;
        runScript(false, async (alive) => {
          if (!alive()) {
            scriptBusy = false;
            return;
          }
          set((state) => ({ rbot: { ...state.rbot, pose: "shrug", bubble: reason } }));
          await sleep(1500);
          if (alive()) {
            set((state) => ({ rbot: { ...state.rbot, pose: "idle", bubble: "watching for a gap" } }));
          }
          scriptBusy = false;
        });
        break;
      }
      case "approved": {
        trading = true;
        pendingResult = null;
        pushLog(
          "TRADE",
          `${poolName(event.sellDex)} → ${poolName(event.buyDex)}  in ${event.inputSol?.toFixed(4)} SOL  net ${event.profitSol?.toFixed(6)}`,
          "good",
        );
        const feeSol = (event.costs?.network ?? 0) / 1e9;
        const tipSol = (event.costs?.tip ?? 0) / 1e9;
        const prioritySol = (event.costs?.priority ?? 0) / 1e9;
        const flashSol = (event.costs?.flash ?? 0) / 1e9;
        const netSol = event.profitSol ?? 0;
        if (scriptBusy && routeReachesJito) {
          awaitChainResult = true;
          break;
        }
        runScript(true, async (alive) => {
          scriptBusy = true;
          routeReachesJito = true;
          awaitChainResult = true;
          try {
            await playRoute(alive, {
              sellDex: event.sellDex,
              buyDex: event.buyDex,
              flashLoan: Boolean(event.flashLoan),
              grossSol: netSol + feeSol + tipSol + prioritySol + flashSol,
              feeSol,
              tipSol,
              prioritySol,
              flashSol,
              minProfitSol: null,
              netSol,
              worthIt: true,
              known: event.costs != null,
              local: false,
              instructions: event.instructions ?? defaultInstructions,
            });
          } finally {
            trading = false;
            scriptBusy = false;
            routeReachesJito = false;
            awaitChainResult = false;
          }
        });
        break;
      }
      case "simulation": {
        set((state) => ({
          simulated: state.simulated + 1,
          bestProfitSol:
            event.walletChangeSol != null
              ? Math.max(state.bestProfitSol ?? -Infinity, event.walletChangeSol)
              : state.bestProfitSol,
        }));
        const ok = Boolean(event.ok);
        pendingResult = {
          text: ok ? "SIMULATED" : "REVERTED",
          tone: ok ? "good" : "bad",
          detail: ok
            ? `${event.computeUnits ?? "?"} CU · wallet ${event.walletChangeSol != null ? `${event.walletChangeSol >= 0 ? "+" : ""}${event.walletChangeSol.toFixed(6)} SOL` : "?"}`
            : (event.error ?? "simulation failed"),
        };
        pushLog(ok ? "SIM" : "SIM", pendingResult.detail, ok ? "good" : "bad");
        break;
      }
      case "sendSkipped":
        if (!pendingResult) {
          pendingResult = { text: "SIMULATE ONLY", tone: "neutral", detail: "SEND_TRANSACTIONS is off" };
        }
        pushLog("SEND", "dry run — not sent", "info");
        break;
      case "sent":
        pushLog("SENT", `${event.route}  ${event.signature?.slice(0, 12) ?? ""}…`, "good");
        pendingResult = { text: "SENT", tone: "good", detail: event.route ?? "" };
        break;
      case "landed": {
        const ok = Boolean(event.ok);
        const detail = ok
          ? `confirmed ${event.walletChangeSol != null ? `${event.walletChangeSol >= 0 ? "+" : ""}${event.walletChangeSol.toFixed(6)} SOL` : ""}`
          : (event.error ?? "not landed");
        pendingResult = { text: ok ? "LANDED" : "FAILED", tone: ok ? "good" : "bad", detail };
        pushLog(ok ? "LAND" : "FAIL", detail, ok ? "good" : "bad");
        if (event.walletChangeSol != null && ok) {
          set((state) => ({
            bestProfitSol: Math.max(state.bestProfitSol ?? -Infinity, event.walletChangeSol ?? 0),
            walletSol: state.walletSol != null ? state.walletSol + (event.walletChangeSol ?? 0) : state.walletSol,
          }));
        }
        break;
      }
      default:
        break;
    }
  },
}));
