export type PoolBoard = {
  id: string;
  dex: string;
  price: number;
  tick: number;
  liquidity: string;
  feeBps: number;
  slot: number;
  sqrtPrice: string;
};

export type CurvePoint = { inputSol: number; profitSol: number };

export type BotEvent = {
  id?: number;
  t?: number;
  type: string;
  slot?: number;
  dex?: string;
  pool?: string;
  kind?: string;
  line?: string;
  direction?: string;
  previousPrice?: number;
  price?: number;
  tickCount?: number;
  startTick?: number;
  source?: string;
  gapBps?: number;
  feesBps?: number;
  beatsFees?: boolean;
  pools?: PoolBoard[];
  inputSol?: number;
  bridgeUsdc?: number;
  outputSol?: number;
  profitSol?: number;
  profitable?: boolean;
  partial?: boolean;
  feeSol?: number;
  tipSol?: number;
  prioritySol?: number;
  flashSol?: number;
  minProfitSol?: number;
  netSol?: number;
  worthIt?: boolean;
  points?: CurvePoint[];
  sellPool?: string;
  buyPool?: string;
  reason?: string;
  sellDex?: string;
  buyDex?: string;
  expectedOutSol?: number;
  costs?: { network: number; priority: number; tip: number; flash: number };
  flashLoan?: boolean;
  capped?: boolean;
  instructions?: string[];
  ok?: boolean;
  computeUnits?: number | null;
  walletChangeSol?: number | null;
  error?: string | null;
  route?: string;
  signature?: string;
  demoBps?: number;
  sendTransactions?: boolean;
  simulate?: boolean;
  funding?: string;
  wallet?: string | null;
  walletSol?: number | null;
};

export type Pose = "idle" | "walk" | "carry" | "shrug" | "celebrate";

export type Carry = "bag" | "envelope" | null;

export type Actor = {
  id: string;
  kind: "visitor" | "whale";
  x: number;
  y: number;
  color: string;
  label: string;
  bag: "sol" | "usdc" | null;
};

export type Stamp = { text: string; tone: "good" | "bad" | "neutral"; detail: string };

export type LaptopScreen =
  | {
      mode: "costs";
      known: boolean;
      grossSol: number;
      feeSol: number;
      tipSol: number;
      prioritySol: number;
      flashSol: number;
      minProfitSol: number | null;
      netSol: number;
      worth: boolean;
    }
  | { mode: "build"; lines: string[] };

export type LogLine = { id: number; tag: string; text: string; tone: string };

export type LinkMode = "connecting" | "live" | "preview" | "replay" | "offline";
