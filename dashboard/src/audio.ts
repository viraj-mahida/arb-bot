let context: AudioContext | null = null;

function audio(): AudioContext | null {
  if (typeof window === "undefined") return null;
  if (!context) {
    const Ctx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    context = new Ctx();
  }
  if (context.state === "suspended") void context.resume();
  return context;
}

export function armAudio() {
  audio();
}

function tone(frequency: number, duration: number, type: OscillatorType, gain: number, delay = 0) {
  const ctx = audio();
  if (!ctx) return;
  const start = ctx.currentTime + delay;
  const oscillator = ctx.createOscillator();
  const amp = ctx.createGain();
  oscillator.type = type;
  oscillator.frequency.value = frequency;
  amp.gain.setValueAtTime(gain, start);
  amp.gain.exponentialRampToValueAtTime(0.0001, start + duration);
  oscillator.connect(amp);
  amp.connect(ctx.destination);
  oscillator.start(start);
  oscillator.stop(start + duration + 0.02);
}

export function blipSwap() {
  tone(520, 0.06, "square", 0.03);
}

export function blipGeyser() {
  tone(180, 0.04, "triangle", 0.02);
}

export function blipStamp(ok: boolean) {
  if (ok) {
    tone(523, 0.1, "triangle", 0.05);
    tone(659, 0.14, "triangle", 0.05, 0.08);
    tone(784, 0.2, "triangle", 0.05, 0.16);
  } else {
    tone(220, 0.16, "sawtooth", 0.03);
    tone(160, 0.2, "sawtooth", 0.03, 0.1);
  }
}
