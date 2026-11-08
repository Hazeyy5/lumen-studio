// Synthèse de bruitages Lumen (partagée par l'onglet Sons de Lumen et tools/lumen-sfx.mjs).
// Un son = des couches d'oscillateurs/bruit avec enveloppe, glissé de fréquence, vibrato, arpège et filtres.

export const SAMPLE_RATE = 44100;

/** Générateur pseudo-aléatoire reproductible. */
export function rng(seed) {
  let a = seed >>> 0 || 1;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const pair = (v, d) => (Array.isArray(v) ? [Number(v[0] ?? d), Number(v[1] ?? v[0] ?? d)] : [Number(v ?? d), Number(v ?? d)]);

/** Durée totale d'un son (secondes), écho compris. */
export function sfxDuration(spec) {
  let end = 0;
  for (const l of spec.layers || []) {
    const len = (l.attack ?? 0.005) + (l.hold ?? 0.05) + (l.decay ?? 0.2);
    end = Math.max(end, (l.start ?? 0) + len);
  }
  const echo = spec.echo;
  if (echo && echo[0] > 0 && echo[1] > 0) end += echo[0] * Math.min(8, Math.ceil(Math.log(0.02) / Math.log(echo[1])));
  return Math.min(Math.max(end, 0.02), 6);
}

function renderLayer(l, out, rate, seed) {
  const rand = rng(seed);
  const attack = Math.max(0.0005, l.attack ?? 0.005);
  const hold = Math.max(0, l.hold ?? 0.05);
  const decay = Math.max(0.001, l.decay ?? 0.2);
  const len = attack + hold + decay;
  const start = Math.max(0, l.start ?? 0);
  const [f0, f1] = pair(l.freq, 440);
  const exp = (l.curve ?? "exp") === "exp" && f0 > 0 && f1 > 0;
  const [vibDepth, vibSpeed] = pair(l.vibrato, 0);
  const [d0, d1] = pair(l.duty, 0.5);
  const [lp0, lp1] = pair(l.lowpass, 20000);
  const hp = Number(l.highpass ?? 0);
  const q = Math.max(0.5, Number(l.resonance ?? 0.7));
  const arp = (l.arp || []).slice().sort((a, b) => a[0] - b[0]);
  const repeat = Number(l.repeat ?? 0);
  const punch = Number(l.punch ?? 0);
  const vol = Number(l.volume ?? 0.8);
  const bits = Number(l.bits ?? 0);
  const wave = l.wave ?? "square";
  const first = Math.floor(start * rate);
  const count = Math.floor(len * rate);
  let phase = 0;
  let noiseVal = rand() * 2 - 1;
  let noisePhase = 0;
  let lpLow = 0;
  let lpBand = 0;
  let hpPrev = 0;
  let hpOut = 0;
  const hpA = hp > 0 ? 1 / (1 + 2 * Math.PI * hp / rate) : 0;
  for (let i = 0; i < count && first + i < out.length; i++) {
    const t = i / rate;
    const tf = repeat > 0 ? t % repeat : t;
    const u = Math.min(1, tf / len);
    let f = exp ? f0 * Math.pow(f1 / f0, u) : f0 + (f1 - f0) * u;
    for (const [at, mult] of arp) if (tf >= at) f *= mult;
    if (vibDepth) f *= Math.pow(2, (vibDepth * Math.sin(2 * Math.PI * vibSpeed * t)) / 12);
    f = Math.max(1, Math.min(f, rate * 0.45));
    phase += f / rate;
    if (phase >= 1) phase -= Math.floor(phase);
    let s;
    switch (wave) {
      case "sine":
        s = Math.sin(2 * Math.PI * phase);
        break;
      case "triangle":
        s = 1 - 4 * Math.abs(phase - 0.5);
        break;
      case "saw":
        s = 2 * phase - 1;
        break;
      case "noise":
        // Bruit échantillonné-bloqué : fréquence basse = grondement, haute = souffle.
        noisePhase += (f * 8) / rate;
        if (noisePhase >= 1) {
          noisePhase -= Math.floor(noisePhase);
          noiseVal = rand() * 2 - 1;
        }
        s = noiseVal;
        break;
      default: {
        const duty = d0 + (d1 - d0) * u;
        s = phase < duty ? 1 : -1;
      }
    }
    let env;
    if (t < attack) env = t / attack;
    else if (t < attack + hold) env = 1 + punch * (1 - (t - attack) / Math.max(hold, 1e-6));
    else {
      const x = (t - attack - hold) / decay;
      env = (1 - x) * (1 - x);
    }
    // Filtre passe-bas résonant (état variable de Chamberlin).
    const cut = lp0 + (lp1 - lp0) * u;
    if (cut < 19000) {
      const fc = 2 * Math.sin(Math.PI * Math.min(cut, rate / 6) / rate);
      lpLow += fc * lpBand;
      const high = s - lpLow - lpBand / q;
      lpBand += fc * high;
      s = lpLow;
    }
    if (hp > 0) {
      hpOut = hpA * (hpOut + s - hpPrev);
      hpPrev = s;
      s = hpOut;
    }
    if (bits > 0) {
      const steps = Math.pow(2, bits - 1);
      s = Math.round(s * steps) / steps;
    }
    out[first + i] += s * env * vol;
  }
}

/** Rend un son en échantillons mono flottants (-1..1), normalisés à `volume`. */
export function renderSfx(spec, rate = SAMPLE_RATE) {
  const n = Math.max(1, Math.floor(sfxDuration(spec) * rate));
  const out = new Float32Array(n);
  (spec.layers || []).forEach((l, i) => renderLayer(l, out, rate, (spec.seed ?? 1) * 7919 + i * 104729));
  const echo = spec.echo;
  if (echo && echo[0] > 0 && echo[1] > 0) {
    const d = Math.floor(echo[0] * rate);
    const mix = echo[2] ?? 0.35;
    const wet = new Float32Array(n);
    for (let i = d; i < n; i++) wet[i] = (out[i - d] + wet[i - d] * echo[1]);
    for (let i = 0; i < n; i++) out[i] += wet[i] * mix;
  }
  let dc = 0;
  for (let i = 0; i < n; i++) dc += out[i];
  dc /= n;
  let peak = 0;
  for (let i = 0; i < n; i++) {
    out[i] -= dc;
    peak = Math.max(peak, Math.abs(out[i]));
  }
  const gain = peak > 0 ? (spec.volume ?? 0.9) / peak : 0;
  const fade = Math.min(n, Math.floor(rate * 0.004));
  for (let i = 0; i < n; i++) {
    let g = gain;
    if (i < 64) g *= i / 64;
    if (i >= n - fade) g *= (n - i) / fade;
    out[i] *= g;
  }
  return out;
}

/** WAV PCM 16 bits mono. */
export function encodeWav(samples, rate = SAMPLE_RATE) {
  const buf = new ArrayBuffer(44 + samples.length * 2);
  const v = new DataView(buf);
  const str = (o, s) => {
    for (let i = 0; i < s.length; i++) v.setUint8(o + i, s.charCodeAt(i));
  };
  str(0, "RIFF");
  v.setUint32(4, 36 + samples.length * 2, true);
  str(8, "WAVE");
  str(12, "fmt ");
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, rate, true);
  v.setUint32(28, rate * 2, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  str(36, "data");
  v.setUint32(40, samples.length * 2, true);
  for (let i = 0; i < samples.length; i++) {
    const s = Math.max(-1, Math.min(1, samples[i]));
    v.setInt16(44 + i * 2, s < 0 ? s * 0x8000 : s * 0x7fff, true);
  }
  return new Uint8Array(buf);
}

// ---------------------------------------------------------------------------------------------
// Préréglages : chaque appel avec une graine différente donne une variante du même type de son.

const between = (r, a, b) => a + (b - a) * r();
const pick = (r, list) => list[Math.floor(r() * list.length) % list.length];
const NOTE = (n) => 440 * Math.pow(2, (n - 69) / 12);

export const PRESETS = {
  coin: { label: "Pièce / collecte", build: (r) => {
    const base = between(r, 900, 1400);
    const step = pick(r, [4 / 3, 3 / 2, 5 / 4]);
    return { layers: [{ wave: pick(r, ["square", "square", "triangle"]), duty: [0.25, 0.25], attack: 0.002, hold: between(r, 0.03, 0.06), decay: between(r, 0.18, 0.3), punch: between(r, 0.2, 0.5), freq: [base, base], arp: [[between(r, 0.05, 0.08), step]], volume: 0.8 }] };
  } },
  pickup: { label: "Objet ramassé", build: (r) => {
    const f = between(r, 500, 900);
    return { layers: [{ wave: pick(r, ["square", "sine", "triangle"]), duty: [0.4, 0.4], attack: 0.003, hold: 0.04, decay: between(r, 0.12, 0.2), freq: [f, f * between(r, 1.6, 2.4)], curve: "exp", volume: 0.8 }] };
  } },
  powerup: { label: "Bonus / power-up", build: (r) => {
    const f = between(r, 250, 450);
    return { layers: [{ wave: pick(r, ["square", "saw"]), duty: [0.3, 0.5], attack: 0.005, hold: 0.25, decay: 0.25, freq: [f, f * between(r, 3, 4.5)], vibrato: [between(r, 0.2, 0.6), between(r, 12, 20)], repeat: pick(r, [0, 0.12, 0.16]), lowpass: [6000, 9000], volume: 0.7 }] };
  } },
  levelup: { label: "Niveau gagné / succès", build: (r) => {
    const root = NOTE(Math.floor(between(r, 67, 75)));
    const notes = pick(r, [[0, 4, 7, 12], [0, 5, 9, 12], [0, 4, 7, 11, 12]]);
    const gap = between(r, 0.07, 0.1);
    return { echo: [0.09, 0.3, 0.25], layers: notes.map((n, i) => ({ start: i * gap, wave: "square", duty: [0.5, 0.5], attack: 0.003, hold: 0.05, decay: i === notes.length - 1 ? 0.45 : 0.12, freq: root * Math.pow(2, n / 12), vibrato: i === notes.length - 1 ? [0.25, 6] : [0, 0], lowpass: [5000, 5000], volume: 0.6 })) };
  } },
  jump: { label: "Saut", build: (r) => {
    const f = between(r, 200, 330);
    return { layers: [{ wave: pick(r, ["square", "square", "triangle"]), duty: [0.5, 0.3], attack: 0.005, hold: 0.03, decay: between(r, 0.15, 0.25), freq: [f, f * between(r, 2.2, 3)], curve: "exp", lowpass: [4000, 8000], volume: 0.8 }] };
  } },
  bounce: { label: "Rebond / boing", build: (r) => {
    const f = between(r, 120, 200);
    return { layers: [{ wave: "sine", attack: 0.003, hold: 0.02, decay: between(r, 0.3, 0.45), freq: [f * 3, f], curve: "exp", vibrato: [between(r, 1, 2), between(r, 10, 16)], volume: 0.9 }] };
  } },
  laser: { label: "Laser / tir rétro", build: (r) => {
    const f = between(r, 1200, 2200);
    return { layers: [{ wave: pick(r, ["square", "saw"]), duty: [0.2, 0.5], attack: 0.001, hold: 0.02, decay: between(r, 0.12, 0.22), freq: [f, f * between(r, 0.12, 0.25)], curve: "exp", punch: 0.3, volume: 0.7 }] };
  } },
  shoot: { label: "Tir / coup de feu", build: (r) => ({
    layers: [
      { wave: "noise", attack: 0.001, hold: 0.01, decay: between(r, 0.12, 0.2), freq: [between(r, 3000, 5000), 600], punch: 0.6, lowpass: [9000, 1500], volume: 0.9 },
      { wave: "sine", attack: 0.001, hold: 0.01, decay: 0.08, freq: [between(r, 180, 240), 50], volume: 0.7 },
    ],
  }) },
  explosion: { label: "Explosion", build: (r) => ({
    layers: [
      { wave: "noise", attack: 0.002, hold: between(r, 0.05, 0.12), decay: between(r, 0.6, 1.1), freq: [between(r, 600, 1200), between(r, 60, 120)], punch: 0.8, lowpass: [6000, 300], resonance: 0.9, volume: 1 },
      { wave: "sine", attack: 0.002, hold: 0.04, decay: between(r, 0.3, 0.5), freq: [between(r, 90, 130), 30], volume: 0.8 },
    ],
  }) },
  hit: { label: "Coup / impact", build: (r) => ({
    layers: [
      { wave: "noise", attack: 0.001, hold: 0.005, decay: between(r, 0.06, 0.12), freq: [between(r, 2000, 4000), 800], lowpass: [7000, 2000], punch: 0.5, volume: 0.8 },
      { wave: pick(r, ["sine", "triangle"]), attack: 0.001, hold: 0.01, decay: between(r, 0.08, 0.14), freq: [between(r, 160, 260), between(r, 50, 80)], curve: "exp", volume: 0.9 },
    ],
  }) },
  hurt: { label: "Dégât reçu", build: (r) => {
    const f = between(r, 300, 500);
    return { layers: [
      { wave: pick(r, ["saw", "square"]), duty: [0.5, 0.5], attack: 0.002, hold: 0.03, decay: between(r, 0.15, 0.25), freq: [f, f * 0.45], curve: "exp", lowpass: [3500, 1200], volume: 0.8 },
      { wave: "noise", attack: 0.001, hold: 0.01, decay: 0.08, freq: [3000, 1500], lowpass: [5000, 2000], volume: 0.4 },
    ] };
  } },
  click: { label: "Clic d'interface", build: (r) => ({
    layers: [{ wave: pick(r, ["sine", "triangle", "square"]), duty: [0.5, 0.5], attack: 0.0008, hold: 0.004, decay: between(r, 0.025, 0.05), freq: [between(r, 1500, 2600), between(r, 900, 1400)], lowpass: [8000, 8000], volume: 0.7 }],
  }) },
  hover: { label: "Survol d'interface", build: (r) => ({
    layers: [{ wave: "sine", attack: 0.004, hold: 0.01, decay: between(r, 0.04, 0.07), freq: [between(r, 2200, 3200), between(r, 2600, 3800)], volume: 0.5 }],
  }) },
  confirm: { label: "Validation / achat", build: (r) => {
    const a = NOTE(Math.floor(between(r, 76, 84)));
    const up = pick(r, [5, 7, 12]);
    return { layers: [
      { wave: "sine", attack: 0.002, hold: 0.04, decay: 0.12, freq: a, volume: 0.7 },
      { start: 0.07, wave: "sine", attack: 0.002, hold: 0.05, decay: 0.3, freq: a * Math.pow(2, up / 12), vibrato: [0.15, 6], volume: 0.75 },
      { start: 0.07, wave: "triangle", attack: 0.002, hold: 0.03, decay: 0.2, freq: a * Math.pow(2, up / 12) * 2, volume: 0.2 },
    ] };
  } },
  error: { label: "Erreur / refus", build: (r) => {
    const f = between(r, 140, 220);
    return { layers: [
      { wave: "square", duty: [0.5, 0.5], attack: 0.002, hold: 0.08, decay: 0.05, freq: f, lowpass: [2500, 2500], volume: 0.6 },
      { start: 0.14, wave: "square", duty: [0.5, 0.5], attack: 0.002, hold: 0.12, decay: 0.08, freq: f * pick(r, [0.84, 0.75]), lowpass: [2500, 2500], volume: 0.6 },
    ] };
  } },
  whoosh: { label: "Whoosh / attaque à l'arme", build: (r) => {
    const len = between(r, 0.18, 0.35);
    return { layers: [{ wave: "noise", attack: len * 0.55, hold: 0, decay: len * 0.45, freq: [20000, 20000], lowpass: [between(r, 400, 800), between(r, 3000, 5000)], resonance: between(r, 1.5, 3), highpass: 200, volume: 0.9 }] };
  } },
  blip: { label: "Bip / dialogue", build: (r) => ({
    layers: [{ wave: pick(r, ["square", "triangle"]), duty: [0.5, 0.5], attack: 0.002, hold: 0.02, decay: 0.04, freq: NOTE(Math.floor(between(r, 72, 86))), volume: 0.6 }],
  }) },
  footstep: { label: "Pas", build: (r) => ({
    layers: [
      { wave: "noise", attack: 0.002, hold: 0.01, decay: between(r, 0.05, 0.09), freq: [between(r, 1500, 3000), 500], lowpass: [between(r, 1500, 2500), 600], volume: 0.8 },
      { wave: "sine", attack: 0.001, hold: 0.005, decay: 0.05, freq: [between(r, 90, 130), 60], volume: 0.6 },
    ],
  }) },
  cash: { label: "Caisse / argent", build: (r) => {
    const f = between(r, 1800, 2400);
    return { echo: [0.06, 0.25, 0.2], layers: [
      { wave: "noise", attack: 0.001, hold: 0.01, decay: 0.05, freq: [6000, 3000], highpass: 2000, volume: 0.4 },
      { start: 0.03, wave: "sine", attack: 0.001, hold: 0.02, decay: 0.5, freq: f, vibrato: [0.1, 7], volume: 0.7 },
      { start: 0.03, wave: "sine", attack: 0.001, hold: 0.02, decay: 0.35, freq: f * 2.76, volume: 0.25 },
    ] };
  } },
  magic: { label: "Sort / magie", build: (r) => {
    const f = between(r, 600, 1000);
    return { echo: [0.11, 0.4, 0.3], layers: [
      { wave: "sine", attack: 0.02, hold: 0.2, decay: 0.4, freq: [f, f * between(r, 1.5, 2.5)], vibrato: [0.5, between(r, 8, 14)], volume: 0.6 },
      { wave: "noise", attack: 0.05, hold: 0.15, decay: 0.3, freq: [20000, 20000], lowpass: [3000, 9000], highpass: 2500, volume: 0.25 },
    ] };
  } },
};

/** Spécification complète d'un préréglage (`pitch` multiplie les fréquences, `length` les durées). */
export function makePreset(preset, { seed = 1, pitch = 1, length = 1, name } = {}) {
  const p = PRESETS[preset];
  if (!p) throw new Error(`Préréglage inconnu : ${preset} (${Object.keys(PRESETS).join(", ")})`);
  const spec = p.build(rng(seed * 2654435761));
  const scaleF = (v) => (Array.isArray(v) ? v.map((x) => x * pitch) : v * pitch);
  for (const l of spec.layers) {
    l.freq = scaleF(l.freq);
    for (const k of ["start", "attack", "hold", "decay", "repeat"]) if (l[k] !== undefined) l[k] = +(l[k] * length).toFixed(4);
    for (const k of Object.keys(l)) {
      const v = l[k];
      if (typeof v === "number") l[k] = +v.toFixed(4);
      else if (Array.isArray(v)) l[k] = JSON.parse(JSON.stringify(v, (_, x) => (typeof x === "number" ? +x.toFixed(4) : x)));
    }
  }
  return { name: name ?? preset, preset, seed, pitch, length, volume: 0.9, ...(spec.echo ? { echo: spec.echo } : {}), layers: spec.layers };
}
