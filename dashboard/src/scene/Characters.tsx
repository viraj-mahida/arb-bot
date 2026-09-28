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
  const gap = useCity((state) => state.gapBps);
  const stamp = useCity((state) => state.stamp);
  const walking = rbot.pose === "walk" || rbot.pose === "carry";
  const gapText = gap == null ? "—" : gap.toFixed(1);

  return (
    <motion.div
      className={`actor rbot pose-${rbot.pose}`}
      animate={feetStyle(rbot.x, rbot.y)}
      transition={{ duration: walking ? 0.68 : 0.25, ease: "easeInOut" }}
    >
      {rbot.bubble && <div className={`bubble${rbot.x > 1300 ? " flip" : ""}`}>{rbot.bubble}</div>}
      {stamp && (
        <div className={`stamp tone-${stamp.tone}`}>
          {stamp.text}
          <small>{stamp.detail}</small>
        </div>
      )}
      <div className="nametag">RBOT</div>
      <svg className="sprite" viewBox="0 0 80 108" width="108" height="146">
        <defs>
          <filter id="rbot-glow" x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="1.2" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
        <ellipse className="shadow" cx="40" cy="103" rx="16" ry="4" fill="rgba(0,0,0,0.5)" />
        <g className="legs">
          <path className="leg-l" d="M24 76 H38 L37 91 H23 Z" fill="#2a3c52" stroke="#7af0ff" strokeWidth="1" />
          <path className="leg-l" d="M22 91 H38 L36 104 H20 Z" fill="#1c2c40" stroke="#5ee7ff" strokeWidth="1" />
          <rect className="leg-l" x="26" y="89" width="10" height="2.5" fill="#5ee7ff" />
          <path className="leg-r" d="M42 76 H56 L57 91 H43 Z" fill="#2a3c52" stroke="#7af0ff" strokeWidth="1" />
          <path className="leg-r" d="M42 91 H58 L60 104 H44 Z" fill="#1c2c40" stroke="#5ee7ff" strokeWidth="1" />
          <rect className="leg-r" x="44" y="89" width="10" height="2.5" fill="#5ee7ff" />
        </g>
        <g className="body">
          <path d="M12 50 L6 64 L14 82 L30 86 H50 L66 82 L74 64 L68 50 Z" fill="#31465e" stroke="#9af6ff" strokeWidth="1.3" />
          <path d="M22 56 H58 L54 74 H26 Z" fill="#071018" stroke="#5ee7ff" strokeWidth="1.1" />
          <text x="40" y="68" textAnchor="middle" fill="#7dffa8" fontSize="10" fontFamily="IBM Plex Mono, monospace">
            {gapText}
          </text>
          <path d="M6 58 H16 L14 66 H4 Z" fill="#ffb020" filter="url(#rbot-glow)" />
          <path d="M64 58 H74 L76 66 H66 Z" fill="#5ee7ff" filter="url(#rbot-glow)" />
          <path d="M28 50 L24 40 H56 L52 50 Z" fill="#24364c" stroke="#7af0ff" strokeWidth="0.8" />
          <path d="M20 40 L16 24 L26 14 H54 L64 24 L60 40 Z" fill="#3a516c" stroke="#d7fbff" strokeWidth="1.2" />
          <rect x="24" y="22" width="32" height="9" fill="#041820" stroke="#5ee7ff" strokeWidth="0.8" />
          <rect x="26" y="24" width="28" height="4" fill="#7dfff0" filter="url(#rbot-glow)" />
          <path d="M34 14 L40 6 L46 14 Z" fill="#5ee7ff" filter="url(#rbot-glow)" />
          {rbot.carry === "bag" && <path d="M60 50 H74 L72 66 H62 Z" fill="#f0c14a" stroke="#1c2433" strokeWidth="1" />}
          {rbot.carry === "envelope" && <path d="M56 48 H74 L72 60 H58 Z" fill="#f4efe4" stroke="#1c2433" strokeWidth="1" />}
        </g>
      </svg>
    </motion.div>
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
