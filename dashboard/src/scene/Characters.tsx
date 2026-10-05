import { motion } from "framer-motion";
import { useCity } from "../store";
import type { Actor } from "../types";

function feetStyle(x: number, y: number) {
  return {
    left: x,
    top: y,
    zIndex: 20 + Math.round(y / 200),
  };
}

export function Rbot() {
  const rbot = useCity((state) => state.rbot);
  const stamp = useCity((state) => state.stamp);
  const walking = rbot.pose === "walk" || rbot.pose === "carry";

  return (
    <motion.div
      className={`actor rbot pose-${rbot.pose}`}
      animate={feetStyle(rbot.x, rbot.y)}
      transition={{ duration: walking ? 0.68 : 0.25, ease: "easeInOut" }}
    >
      {rbot.bubble && (
        <div className={`bubble${rbot.x > 1300 ? " flip" : ""}`}>
          {rbot.bubble}
          {rbot.sandbox && <small className="sandbox">(sandbox)</small>}
        </div>
      )}
      {stamp && (
        <div className={`stamp tone-${stamp.tone}`}>
          {stamp.text}
          <small>{stamp.detail}</small>
        </div>
      )}
      <div className="nametag">RBOT</div>
      <svg className="sprite" viewBox="0 0 96 120" width="104" height="130">
        <ellipse className="shadow" cx="48" cy="116" rx="18" ry="4" fill="rgba(0,0,0,0.4)" />
        <g className="legs">
          <g className="leg-l">
            <rect x="30" y="88" width="12" height="16" rx="6" fill="#3c4a5e" />
            <ellipse cx="36" cy="106" rx="8" ry="4.5" fill="#2c3848" />
          </g>
          <g className="leg-r">
            <rect x="54" y="88" width="12" height="16" rx="6" fill="#3c4a5e" />
            <ellipse cx="60" cy="106" rx="8" ry="4.5" fill="#2c3848" />
          </g>
        </g>
        <g className="body">
          <rect x="16" y="64" width="10" height="18" rx="5" fill="#7ecfc4" />
          <rect x="70" y="62" width="10" height="18" rx="5" fill="#7ecfc4" />
          <rect x="26" y="52" width="44" height="40" rx="18" fill="#8ee0d4" />
          <rect x="34" y="62" width="28" height="18" rx="9" fill="#f6fffc" />
          <circle cx="48" cy="34" r="22" fill="#f7fbff" stroke="#8ee0d4" strokeWidth="3" />
          <line x1="48" y1="14" x2="48" y2="6" stroke="#5eb8ae" strokeWidth="2.2" strokeLinecap="round" />
          <circle cx="48" cy="5" r="3.2" fill="#ffb020" />
          <Face pose={rbot.pose} />
          <ellipse cx="30" cy="40" rx="3.2" ry="1.7" fill="#ffb3c4" opacity="0.85" />
          <ellipse cx="66" cy="40" rx="3.2" ry="1.7" fill="#ffb3c4" opacity="0.85" />
          {rbot.carry && <Held kind={rbot.carry} />}
        </g>
      </svg>
    </motion.div>
  );
}

function Face({ pose }: { pose: string }) {
  if (pose === "celebrate") {
    return (
      <>
        <path d="M36 34 Q40 29 44 34" fill="none" stroke="#243044" strokeWidth="2.2" strokeLinecap="round" />
        <path d="M52 34 Q56 29 60 34" fill="none" stroke="#243044" strokeWidth="2.2" strokeLinecap="round" />
        <path d="M41 42 Q48 48 55 42" fill="none" stroke="#243044" strokeWidth="1.8" strokeLinecap="round" />
      </>
    );
  }
  if (pose === "shrug") {
    return (
      <>
        <path d="M34 28 Q40 26 44 30" fill="none" stroke="#243044" strokeWidth="1.6" strokeLinecap="round" />
        <circle cx="39" cy="34" r="3" fill="#243044" />
        <circle cx="57" cy="33" r="3" fill="#243044" />
        <path d="M42 44 Q48 42 54 45" fill="none" stroke="#243044" strokeWidth="1.6" strokeLinecap="round" />
      </>
    );
  }
  return (
    <g className="eyes">
      <ellipse cx="39" cy="33" rx="4.2" ry="4.8" fill="#243044" />
      <ellipse cx="57" cy="33" rx="4.2" ry="4.8" fill="#243044" />
      <circle cx="40.6" cy="31.4" r="1.5" fill="#fff" />
      <circle cx="58.6" cy="31.4" r="1.5" fill="#fff" />
      <path d="M42 43 Q48 47 54 43" fill="none" stroke="#243044" strokeWidth="1.7" strokeLinecap="round" />
    </g>
  );
}

function Held({ kind }: { kind: "slip" | "notes" | "packet" }) {
  if (kind === "slip") {
    return (
      <g transform="translate(78, 48)">
        <rect width="28" height="32" rx="3" fill="#ffe56a" stroke="#1c2433" strokeWidth="1.4" />
        <path d="M6 10 H22 M6 16 H18 M6 22 H14" stroke="#1c2433" strokeWidth="1.4" strokeLinecap="round" />
      </g>
    );
  }
  if (kind === "notes") {
    return (
      <g transform="translate(76, 42)">
        <rect width="32" height="40" rx="3" fill="#f7f1e4" stroke="#1c2433" strokeWidth="1.4" />
        <rect x="10" y="-4" width="12" height="8" rx="2" fill="#8ee0d4" stroke="#1c2433" strokeWidth="1.2" />
        <path d="M6 12 H26 M6 19 H26 M6 26 H20" stroke="#1c2433" strokeWidth="1.5" strokeLinecap="round" />
      </g>
    );
  }
  return (
    <g transform="translate(76, 52)">
      <rect width="34" height="24" rx="2" fill="#f4efe4" stroke="#1c2433" strokeWidth="1.4" />
      <path d="M1 1 L17 14 L33 1" fill="none" stroke="#1c2433" strokeWidth="1.4" />
      <text x="17" y="20" textAnchor="middle" fill="#1c2433" fontSize="8" fontFamily="IBM Plex Mono, monospace">
        TX
      </text>
    </g>
  );
}

export function Actors() {
  const actors = useCity((state) => state.actors);
  return (
    <>
      {actors.map((actor) => (
        <Npc key={actor.id} actor={actor} />
      ))}
    </>
  );
}

function Npc({ actor }: { actor: Actor }) {
  const whale = actor.kind === "whale";
  return (
    <motion.div
      className={`actor ${actor.kind}`}
      animate={feetStyle(actor.x, actor.y)}
      transition={{ duration: whale ? 1.1 : 0.8, ease: "easeInOut" }}
    >
      <div className="nametag">{actor.label}</div>
      {whale ? <Whale /> : <Visitor color={actor.color} bag={actor.bag} />}
    </motion.div>
  );
}

function Visitor({ color, bag }: { color: string; bag: Actor["bag"] }) {
  return (
    <svg className="sprite" viewBox="0 0 64 80" width="58" height="72">
      <ellipse cx="32" cy="74" rx="14" ry="4" fill="rgba(0,0,0,0.35)" />
      <rect x="22" y="52" width="7" height="16" rx="3" fill="#1c2433" />
      <rect x="35" y="52" width="7" height="16" rx="3" fill="#1c2433" />
      <circle cx="32" cy="36" r="18" fill={color} />
      <circle cx="26" cy="34" r="2.2" fill="#102030" />
      <circle cx="38" cy="34" r="2.2" fill="#102030" />
      {bag === "sol" && <circle cx="50" cy="48" r="7" fill="#c8b6ff" />}
      {bag === "usdc" && <circle cx="50" cy="48" r="7" fill="#8fd18a" />}
    </svg>
  );
}

function Whale() {
  return (
    <svg className="sprite" viewBox="0 0 90 100" width="92" height="102">
      <ellipse cx="45" cy="94" rx="22" ry="5" fill="rgba(0,0,0,0.35)" />
      <rect x="28" y="70" width="12" height="18" rx="4" fill="#1a1f2b" />
      <rect x="50" y="70" width="12" height="18" rx="4" fill="#1a1f2b" />
      <ellipse cx="45" cy="52" rx="28" ry="24" fill="#243044" />
      <ellipse cx="45" cy="58" rx="16" ry="10" fill="#f2f4f8" />
      <circle cx="45" cy="28" r="14" fill="#e6c2a0" />
      <rect x="34" y="16" width="22" height="8" rx="2" fill="#1a1f2b" />
      <circle cx="40" cy="28" r="1.8" fill="#1a1f2b" />
      <circle cx="50" cy="28" r="1.8" fill="#1a1f2b" />
      <rect x="62" y="40" width="14" height="3" rx="1.5" fill="#c47a3a" transform="rotate(-20 62 40)" />
    </svg>
  );
}
