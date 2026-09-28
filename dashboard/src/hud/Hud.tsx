import { downloadTake } from "../events";
import { poolTitle } from "../scene/pools";
import { useCity } from "../store";
import { ProfitCurve } from "./ProfitCurve";

export function Hud() {
  const link = useCity((state) => state.link);
  const demoBps = useCity((state) => state.demoBps);
  const sendTransactions = useCity((state) => state.sendTransactions);
  const simulate = useCity((state) => state.simulate);
  const walletSol = useCity((state) => state.walletSol);
  const slot = useCity((state) => state.slot);
  const gapBps = useCity((state) => state.gapBps);
  const beatsFees = useCity((state) => state.beatsFees);
  const seen = useCity((state) => state.seen);
  const skipped = useCity((state) => state.skipped);
  const simulated = useCity((state) => state.simulated);
  const best = useCity((state) => state.bestProfitSol);
  const log = useCity((state) => state.log);
  const instructions = useCity((state) => state.instructions);
  const stepIndex = useCity((state) => state.stepIndex);
  const pose = useCity((state) => state.rbot.pose);
  const pools = useCity((state) => state.pools);
  const curveSell = useCity((state) => state.curveSell);
  const curveBuy = useCity((state) => state.curveBuy);
  const curvePair =
    curveSell && curveBuy
      ? `${poolTitle(pools[curveSell], curveSell)} → ${poolTitle(pools[curveBuy], curveBuy)}`
      : null;

  return (
    <>
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">R</span>
          <div>
            <strong>Rbot</strong>
            <em>Arb City</em>
          </div>
        </div>
        <div className="badges">
          <span className={`badge ${link === "live" ? "live" : "idle"}`}>
            {link === "live" ? "LIVE MAINNET DATA" : link === "preview" ? "PREVIEW TAKE" : link === "replay" ? "REPLAY" : link === "connecting" ? "CONNECTING" : "BOT OFFLINE"}
          </span>
          {demoBps > 0 && <span className="badge demo">DEMO SCENARIO +{demoBps} bps</span>}
          {!sendTransactions && <span className="badge sim">{simulate ? "SIMULATE ONLY" : "WATCHING"}</span>}
        </div>
        <div className="stats">
          <Stat label="wallet" value={walletSol == null ? "—" : `${walletSol.toFixed(3)} SOL`} />
          <Stat label="slot" value={slot ? String(slot) : "—"} />
          <Stat label="gap" value={gapBps == null ? "—" : `${gapBps.toFixed(2)} bp`} hot={beatsFees} />
          <Stat label="seen / skip / sim" value={`${seen} / ${skipped} / ${simulated}`} />
          <Stat label="best sim" value={best == null ? "—" : `${best >= 0 ? "+" : ""}${best.toFixed(6)}`} />
        </div>
        <button className="ghost" type="button" onClick={() => downloadTake()}>
          save take
        </button>
      </header>

      <aside className="log-panel">
        <div className="panel-title">activity</div>
        <ul>
          {log.map((line) => (
            <li key={line.id} className={`tone-${line.tone}`}>
              <span>{line.tag}</span>
              {line.text}
            </li>
          ))}
          {log.length === 0 && <li className="tone-dim">Rbot is waiting for the Geyser stream.</li>}
        </ul>
      </aside>

      <footer className="bottombar">
        <section>
          <div className="panel-title">
            trade anatomy <em className="pose">{pose}</em>
          </div>
          <ol className="steps">
            {instructions.map((step, index) => (
              <li key={step} className={index === stepIndex ? "on" : index < stepIndex ? "done" : ""}>
                <i>{index + 1}</i>
                {step}
              </li>
            ))}
          </ol>
        </section>
        <section className="curve-section">
          <div className="panel-title">
            profit (SOL) vs size (SOL)
            {curvePair && <span className="curve-pair">{curvePair}</span>}
          </div>
          <div className="curve-host">
            <ProfitCurve />
          </div>
        </section>
      </footer>
    </>
  );
}

function Stat({ label, value, hot }: { label: string; value: string; hot?: boolean }) {
  return (
    <div className={`stat ${hot ? "hot" : ""}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
