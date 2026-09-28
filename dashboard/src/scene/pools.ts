import type { PoolBoard } from "../types";

/** One window per watched pool. The two counters stay Orca and Raydium; each fee tier is its own board. */
const WINDOWS: { id: string; x: number; y: number }[] = [
  { id: "orca_whirlpool:Czfq", x: 418, y: 408 },
  { id: "orca_whirlpool:FpCM", x: 540, y: 408 },
  { id: "orca_whirlpool:83v8", x: 662, y: 408 },
  { id: "raydium_clmm:3ucN", x: 824, y: 408 },
  { id: "raydium_clmm:CYbD", x: 946, y: 408 },
  { id: "raydium_clmm:8sLb", x: 1068, y: 408 },
];

const COLORS = ["#3ee0b0", "#8dffc4", "#1f8f78", "#ff8a3d", "#ffd56a", "#e25b45"];

export function poolColor(id: string): string {
  const known = WINDOWS.findIndex((window) => window.id === id);
  if (known >= 0) return COLORS[known] ?? COLORS[0];
  let hash = 0;
  for (const char of id) hash = (hash * 31 + char.charCodeAt(0)) % COLORS.length;
  return COLORS[hash] ?? COLORS[0];
}

export function poolWindow(id: string): { x: number; y: number } {
  const known = WINDOWS.find((window) => window.id === id);
  if (known) return known;
  const index = Math.abs(hash(id)) % 6;
  return { x: 418 + (index % 3) * 122 + (id.includes("raydium") ? 406 : 0), y: 408 };
}

export function poolFeet(id: string | undefined): { x: number; y: number } {
  if (!id) return { x: 1010, y: 710 };
  const window = poolWindow(id);
  return { x: window.x + 64, y: 710 };
}

export function poolTitle(pool: PoolBoard | undefined, id: string | undefined): string {
  const label = id ?? pool?.id ?? "";
  const venue = label.includes("orca") ? "Orca" : label.includes("raydium") ? "Raydium" : label.split(":")[0] || "pool";
  if (pool) return `${venue} ${trimFee(pool.feeBps)}`;
  const short = label.split(":")[1];
  return short ? `${venue} ${short}` : venue;
}

export function isOrca(id: string | undefined): boolean {
  return Boolean(id?.includes("orca"));
}

function trimFee(feeBps: number): string {
  return `${Number.isInteger(feeBps) ? feeBps.toFixed(0) : feeBps.toFixed(2)}bp`;
}

function hash(value: string): number {
  let result = 0;
  for (const char of value) result = (result * 33 + char.charCodeAt(0)) | 0;
  return result;
}
