import { memo, useEffect, useRef } from "react";
import layout from "./layout.json";
import { useCity } from "../store";
import { Actors, Rbot } from "./Characters";
import { PriceChart } from "../hud/PriceChart";
import { poolColor, poolTitle, poolWindow } from "./pools";

function abbreviate(value: string | undefined): string {
  if (!value) return "—";
  const number = Number(value);
  if (!Number.isFinite(number)) return value;
  if (number >= 1e15) return `${(number / 1e15).toFixed(2)}P`;
  if (number >= 1e12) return `${(number / 1e12).toFixed(2)}T`;
  if (number >= 1e9) return `${(number / 1e9).toFixed(2)}B`;
  if (number >= 1e6) return `${(number / 1e6).toFixed(number >= 1e8 ? 0 : 1)}M`;
  return number.toFixed(0);
}

export function Stage() {
  const pools = useCity((state) => state.pools);
  const walletSol = useCity((state) => state.walletSol);
  const terminal = useCity((state) => state.terminal);
  const terminalPulse = useCity((state) => state.terminalPulse);
  const gapBps = useCity((state) => state.gapBps);
  const poolList = Object.values(pools).sort((a, b) => a.id.localeCompare(b.id));
  const screen = layout.wallScreen;
  const crt = layout.terminalScreen;

  return (
    <div className="room">
      <img className="room-bg" src="/assets/room-bg.png?v=2" alt="" />
      <BlockChain />
      <Cables />

      <div className="wall-screen" style={{ left: screen.x, top: screen.y, width: screen.w, height: screen.h }}>
        <div className="screen-caption">
          {poolList.length === 0 && <span>waiting for pools</span>}
          {poolList.map((pool) => (
            <span key={pool.id}>
              <i className="dot" style={{ background: poolColor(pool.id) }} /> {poolTitle(pool, pool.id)}
            </span>
          ))}
          <span className="caption-gap">{gapBps == null ? "waiting for two prices" : `widest ${gapBps.toFixed(2)} bp`}</span>
        </div>
        <PriceChart />
      </div>

      {layout.props.map((prop) => (
        <div
          key={prop.id}
          className="prop"
          style={{ left: prop.x, top: prop.y, width: prop.w, height: prop.h, zIndex: 4 + Math.round(prop.y / 400) }}
        >
          <img src={prop.src} alt="" />
          {prop.label && (
            <div
              className="prop-label"
              style={"labelY" in prop && typeof prop.labelY === "number" ? { top: prop.labelY - prop.y, bottom: "auto" } : undefined}
            >
              {prop.label}
            </div>
          )}
        </div>
      ))}

      <div
        key={terminalPulse}
        className="crt pulse"
        style={{ left: crt.x, top: crt.y, width: crt.w, height: crt.h }}
      >
        {terminal.slice(-6).map((line, index) => (
          <div key={`${index}-${line}`}>{line}</div>
        ))}
        <span className="caret">▌</span>
      </div>

      {poolList.map((pool) => {
        const spot = poolWindow(pool.id);
        return (
          <DeskBoard
            key={pool.id}
            title={`${Number.isInteger(pool.feeBps) ? pool.feeBps.toFixed(0) : pool.feeBps.toFixed(2)}bp`}
            pool={pool}
            accent={pool.id.includes("orca") ? "orca" : "ray"}
            x={spot.x}
            y={spot.y}
          />
        );
      })}

      <div className="locker-readout" style={{ left: layout.lockerReadout.x, top: layout.lockerReadout.y, width: layout.lockerReadout.w }}>
        {walletSol == null ? "—.—— SOL" : `${walletSol.toFixed(3)} SOL`}
      </div>

      <Laptop />

      <Actors />
      <Rbot />
      <Coins />
    </div>
  );
}

const CHAIN_PITCH = 40;
const CHAIN_COUNT = 22;

const BlockChain = memo(function BlockChain() {
  const scroller = useRef<HTMLDivElement>(null);
  const start = useRef(useCity.getState().slot || 312440510).current;
  useEffect(() => {
    const root = scroller.current;
    if (!root) return;
    let shift = 0;
    let last = performance.now();
    let frame = 0;
    const tick = (now: number) => {
      const dt = Math.min(32, now - last);
      last = now;
      shift += (dt / 1000) * 46;
      while (shift >= CHAIN_PITCH) {
        shift -= CHAIN_PITCH;
        const first = root.firstElementChild;
        const previous = root.lastElementChild?.querySelector("span");
        const previousSlot = Number(previous?.getAttribute("data-slot") ?? start);
        if (first) {
          root.appendChild(first);
          const label = first.querySelector("span");
          const next = previousSlot - 1;
          if (label) {
            label.setAttribute("data-slot", String(next));
            label.textContent = String(next).slice(-4);
          }
        }
      }
      root.style.transform = `translate3d(0, ${-shift}px, 0)`;
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [start]);
  return (
    <div className="chain">
      <div className="chain-label">CHAIN</div>
      <div className="chain-track">
        <div className="chain-scroll" ref={scroller}>
          {Array.from({ length: CHAIN_COUNT }, (_, index) => start - index).map((height) => (
            <div className="chain-block" key={height}>
              <i className="cube-top" />
              <i className="cube-side" />
              <i className="cube-front">
                <span data-slot={height}>{String(height).slice(-4)}</span>
              </i>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
});

function Cables() {
  const screen = layout.wallScreen;
  const lines = [
    { id: "tap", color: "#5ee7ff", d: "M 78 310 C 120 310, 180 330, 208 348", from: [78, 310], to: [208, 348] },
    { id: "screen", color: "#7dffe8", d: `M 378 428 C 500 428, 590 380, ${screen.x} ${screen.y + screen.h - 16}`, from: [378, 428], to: [screen.x, screen.y + screen.h - 16] },
    { id: "jito", color: "#ffb020", d: "M 168 780 C 120 810, 80 828, 52 840", from: [168, 780], to: [52, 840] },
  ];
  return (
    <svg className="cables" viewBox="0 0 1920 1080">
      {lines.map((line) => (
        <g key={line.id} style={{ color: line.color }}>
          <path className="cable-base" d={line.d} />
          <path className="cable-pulse" d={line.d} stroke={line.color} />
          <circle className="cable-dot" cx={line.from[0]} cy={line.from[1]} r="4" fill={line.color} />
          <circle className="cable-dot" cx={line.to[0]} cy={line.to[1]} r="4" fill={line.color} />
        </g>
      ))}
    </svg>
  );
}

function DeskBoard({
  title,
  pool,
  accent,
  x,
  y,
}: {
  title: string;
  pool: { price: number; tick: number; liquidity: string; feeBps: number; slot: number } | undefined;
  accent: string;
  x: number;
  y: number;
}) {
  return (
    <div className={`board compact desk-screen accent-${accent}`} style={{ left: x, top: y }}>
      <div className="board-title">{title}</div>
      <div className="board-price">{pool ? pool.price.toFixed(4) : "—.——"}</div>
      <div className="board-meta">
        <span>tick {pool ? pool.tick : "—"}</span>
        <span>L {abbreviate(pool?.liquidity)}</span>
      </div>
      <TickStrip seed={pool?.tick ?? 0} liquidity={pool?.liquidity ?? "0"} />
    </div>
  );
}

function TickStrip({ seed, liquidity }: { seed: number; liquidity: string }) {
  const bars = Array.from({ length: 14 }, (_, index) => {
    const n = Math.abs(Math.sin(seed * 0.017 + index * 1.7 + liquidity.length) * 100);
    return 20 + (n % 80);
  });
  return (
    <div className="ticks">
      {bars.map((height, index) => (
        <i key={index} style={{ height: `${height}%` }} />
      ))}
    </div>
  );
}

function Laptop() {
  const screen = useCity((state) => state.laptop);
  const box = layout.laptop;
  return (
    <div className="pc" style={{ left: box.x, top: box.y, width: box.w, height: box.h }}>
      <div className="pc-lid">
        <i className="pc-cam" />
        <div className="pc-screen">
          {screen?.mode === "costs" && screen.known && (
            <>
              <div><span>pool</span><b>{signedSol(screen.grossSol)}</b></div>
              <div><span>fee</span><b className="minus">{minusSol(screen.feeSol)}</b></div>
              <div><span>tip</span><b className="minus">{minusSol(screen.tipSol)}</b></div>
              {screen.prioritySol > 0 && (
                <div><span>priority</span><b className="minus">{minusSol(screen.prioritySol)}</b></div>
              )}
              {screen.flashSol > 0 && (
                <div><span>flash</span><b className="minus">{minusSol(screen.flashSol)}</b></div>
              )}
              {screen.minProfitSol != null && (
                <div><span>min</span><b>{screen.minProfitSol.toFixed(6)}</b></div>
              )}
              <div>
                <span>net</span>
                <b className={screen.worth ? "net-good" : "net-bad"}>{signedSol(screen.netSol)}</b>
              </div>
            </>
          )}
          {screen?.mode === "costs" && !screen.known && <div className="pc-idle">no fee on this quote</div>}
          {screen?.mode === "build" &&
            screen.lines.map((line) => (
              <div key={line} className="pc-line">
                {line}
              </div>
            ))}
          {!screen && (
            <div className="pc-idle">
              rbot@local <span className="caret">▌</span>
            </div>
          )}
        </div>
      </div>
      <div className="pc-hinge" />
      <div className="pc-base">
        <div className="pc-keys" />
        <i className="pc-pad" />
      </div>
      <div className="pc-name">Rbot's PC</div>
    </div>
  );
}

function signedSol(value: number) {
  return `${value >= 0 ? "+" : ""}${value.toFixed(6)}`;
}

function minusSol(value: number) {
  return `-${Math.abs(value).toFixed(6)}`;
}

function Coins() {
  const coins = useCity((state) => state.coins);
  if (coins === 0) return null;
  return (
    <div className="coin-burst" key={coins}>
      {Array.from({ length: 8 }, (_, index) => (
        <span key={index} style={{ ["--i" as string]: index }} />
      ))}
    </div>
  );
}
