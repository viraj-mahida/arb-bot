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
  sandbox: boolean;
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
type SendOutcome =
  | { kind: "sent"; route: string }
  | { kind: "sandbox"; route: string }
  | { kind: "reverted"; detail: string };
let sendOutcome: SendOutcome | null = null;
let deferredLand: { ok: boolean; detail: string } | null = null;
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

async function go(
  alive: () => boolean,
  stepIndex: number,
  x: number,
  y: number,
  bubble: string,
  carry: Carry,
  pose: Pose,
) {
  if (!alive()) return;
  useCity.setState((state) => ({
    stepIndex,
    rbot: { ...state.rbot, x, y, pose, carry, bubble },
  }));
  await sleep(720);
}

async function hold(alive: () => boolean, bubble: string, carry: Carry, pose: Pose, ms: number) {
  if (!alive()) return;
  useCity.setState((state) => ({
    rbot: { ...state.rbot, bubble, carry, pose },
  }));
  await sleep(ms);
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
    rbot: {
      ...state.rbot,
      x: spots.screen.x,
      y: spots.screen.y,
      pose: "walk",
      carry: null,
      bubble: "",
      sandbox: false,
    },
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
  },
) {
  const sell = spotFor(plan.sellDex);
  const buy = spotFor(plan.buyDex);
  const funding = plan.flashLoan ? spots.bank : spots.locker;
  const fundingStep = plan.flashLoan ? "check the flash bank" : "check the wallet can cover it";
  useCity.setState({
    instructions: [
      "watch the boards",
      "read the sell pool",
      "read the buy pool",
      fundingStep,
      "run the local quote",
      "send the quote to Jito",
    ],
    stepIndex: -1,
    stamp: null,
    laptop: null,
  });

  await go(alive, 0, spots.screen.x, spots.screen.y, "watching the boards", null, "walk");
  if (!alive()) return;

  await go(alive, 1, sell.x, sell.y, "reading the sell pool", null, "walk");
  if (!alive()) return;
  await hold(alive, "price, fee, liquidity, tick", "slip", "idle", 520);
  if (!alive()) return;

  await go(alive, 2, buy.x, buy.y, "reading the buy pool", "slip", "carry");
  if (!alive()) return;
  await hold(alive, "both pools noted", "notes", "idle", 520);
  if (!alive()) return;

  await go(
    alive,
    3,
    funding.x,
    funding.y,
    plan.flashLoan ? "funding is the flash bank" : "does the wallet cover this?",
    "notes",
    "carry",
  );
  if (!alive()) return;
  await hold(alive, plan.flashLoan ? "flash bank checked" : "wallet checked", "notes", "idle", 420);
  if (!alive()) return;

  await go(alive, 4, spots.laptop.x, spots.laptop.y, "running the local quote", "notes", "carry");
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
    ? "can't price the fees yet"
    : plan.partial
      ? "partial fill — leaving it"
      : plan.worthIt
        ? "quote pays — I'll send it"
        : "not enough after fees";
  await hold(alive, verdict, "notes", "idle", 1800);
  if (!alive()) return;

  if (!plan.worthIt) {
    showLaptop(null);
    await hold(alive, verdict, null, "shrug", 900);
    if (!alive()) return;
    await goHome(alive);
    return;
  }

  showLaptop(null);
  await go(alive, 5, spots.jito.x, spots.jito.y, "taking the quote to be sent", "packet", "carry");
  if (!alive()) return;

  const chain = awaitChainResult || !plan.local;
  const deadline = Date.now() + (chain ? 2200 : 500);
  while (!sendOutcome && Date.now() < deadline) await sleep(80);
  if (!alive()) return;
  const outcome = sendOutcome;
  sendOutcome = null;

  if (outcome?.kind === "reverted") {
    useCity.setState((state) => ({
      stamp: { text: "REVERTED", tone: "bad", detail: outcome.detail },
      rbot: { ...state.rbot, pose: "shrug", carry: null, bubble: "the quote reverted", sandbox: false },
    }));
    await sleep(1200);
    if (!alive()) return;
    await goHome(alive);
    return;
  }

  if (outcome?.kind === "sent" || outcome?.kind === "sandbox") {
    const where = routeLabel(outcome.route);
    useCity.setState((state) => ({
      stamp: null,
      rbot: {
        ...state.rbot,
        pose: "idle",
        carry: null,
        bubble: `Txn sent to ${where}`,
        sandbox: outcome.kind === "sandbox",
      },
    }));
    await sleep(1400);
    if (!alive()) return;
  }

  await goHome(alive);
  if (!alive()) return;
  useCity.setState((state) => ({ stepIndex: state.instructions.length }));
}

function releaseScript() {
  scriptBusy = false;
  const landed = deferredLand;
  if (!landed) return;
  deferredLand = null;
  scriptBusy = true;
  runScript(false, async (alive) => {
    try {
      await showLanding(alive, landed.ok, landed.detail);
    } finally {
      releaseScript();
    }
  });
}

function routeLabel(route: string): string {
  return /rpc/i.test(route) && !/jito/i.test(route) ? "RPC" : "Jito";
}

async function showLanding(alive: () => boolean, ok: boolean, detail: string) {
  if (!alive()) return;
  if (ok) {
    blipStamp(true);
    useCity.setState((state) => ({
      stamp: { text: "LANDED", tone: "good", detail },
      coins: state.coins + 1,
      rbot: { ...state.rbot, pose: "celebrate", carry: null, bubble: "it landed", sandbox: false },
    }));
    await sleep(1600);
  } else {
    useCity.setState((state) => ({
      stamp: { text: "FAILED", tone: "bad", detail },
      rbot: { ...state.rbot, pose: "shrug", carry: null, bubble: "it didn't land", sandbox: false },
    }));
    await sleep(1200);
  }
  if (!alive()) return;
  useCity.setState((state) => ({
    stamp: null,
    rbot: { ...state.rbot, pose: "idle", bubble: "watching for a gap", carry: null, sandbox: false },
  }));
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
  instructions: [
    "watch the boards",
    "read the sell pool",
    "read the buy pool",
    "check funding",
    "run the local quote",
    "send the quote to Jito",
  ],
  stepIndex: -1,
  rbot: { x: spots.screen.x, y: spots.screen.y, pose: "idle", carry: null, bubble: "watching for a gap", sandbox: false },
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
            });
          } finally {
            routeReachesJito = false;
            if (trading) trading = false;
            awaitChainResult = false;
            releaseScript();
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
            releaseScript();
            return;
          }
          set((state) => ({ rbot: { ...state.rbot, pose: "shrug", bubble: reason } }));
          await sleep(1500);
          if (alive()) {
            set((state) => ({ rbot: { ...state.rbot, pose: "idle", bubble: "watching for a gap" } }));
          }
          releaseScript();
        });
        break;
      }
      case "approved": {
        trading = true;
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
            });
          } finally {
            trading = false;
            routeReachesJito = false;
            awaitChainResult = false;
            releaseScript();
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
        const detail = ok
          ? `${event.computeUnits ?? "?"} CU · wallet ${event.walletChangeSol != null ? `${event.walletChangeSol >= 0 ? "+" : ""}${event.walletChangeSol.toFixed(6)} SOL` : "?"}`
          : (event.error ?? "simulation failed");
        if (!ok) sendOutcome = { kind: "reverted", detail };
        pushLog("SIM", detail, ok ? "good" : "bad");
        break;
      }
      case "sendSkipped":
        if (sendOutcome?.kind !== "reverted") {
          sendOutcome = { kind: "sandbox", route: event.route ?? "jito" };
        }
        pushLog("SEND", "sandbox — not broadcast", "info");
        break;
      case "sent":
        sendOutcome = { kind: "sent", route: event.route ?? "jito" };
        pushLog("SENT", `${routeLabel(event.route ?? "")}  ${event.signature?.slice(0, 12) ?? ""}…`, "good");
        break;
      case "landed": {
        const ok = Boolean(event.ok);
        const detail = ok
          ? `${event.walletChangeSol != null ? `${event.walletChangeSol >= 0 ? "+" : ""}${event.walletChangeSol.toFixed(6)} SOL` : "confirmed"}`
          : (event.error ?? "not landed");
        pushLog(ok ? "LAND" : "FAIL", detail, ok ? "good" : "bad");
        if (event.walletChangeSol != null && ok) {
          set((state) => ({
            bestProfitSol: Math.max(state.bestProfitSol ?? -Infinity, event.walletChangeSol ?? 0),
            walletSol: state.walletSol != null ? state.walletSol + (event.walletChangeSol ?? 0) : state.walletSol,
          }));
        }
        deferredLand = { ok, detail };
        if (!scriptBusy) releaseScript();
        break;
      }
      default:
        break;
    }
  },
}));
