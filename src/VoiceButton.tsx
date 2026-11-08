import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type VoiceStatus = { ready: boolean; engine: boolean; model: boolean };
type Phase = "idle" | "installing" | "recording" | "working";

const RATE = 16000;
const MAX_SECONDS = 120;

/** WAV PCM 16 bits mono. */
function encodeWav(samples: Float32Array): Uint8Array {
  const buf = new ArrayBuffer(44 + samples.length * 2);
  const v = new DataView(buf);
  const str = (o: number, s: string) => [...s].forEach((c, i) => v.setUint8(o + i, c.charCodeAt(0)));
  str(0, "RIFF");
  v.setUint32(4, 36 + samples.length * 2, true);
  str(8, "WAVE");
  str(12, "fmt ");
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, RATE, true);
  v.setUint32(28, RATE * 2, true);
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

function toBase64(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(bin);
}

type Recorder = { stop: () => Float32Array };

async function startRecorder(onLevel: (level: number) => void): Promise<Recorder> {
  const stream = await navigator.mediaDevices.getUserMedia({
    audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
  });
  // Le navigateur rééchantillonne le micro en 16 kHz, le format attendu par Whisper.
  const ctx = new AudioContext({ sampleRate: RATE });
  const source = ctx.createMediaStreamSource(stream);
  const node = ctx.createScriptProcessor(4096, 1, 1);
  const chunks: Float32Array[] = [];
  node.onaudioprocess = (e) => {
    const data = e.inputBuffer.getChannelData(0);
    chunks.push(new Float32Array(data));
    let peak = 0;
    for (let i = 0; i < data.length; i += 16) peak = Math.max(peak, Math.abs(data[i]));
    onLevel(peak);
  };
  source.connect(node);
  node.connect(ctx.destination);
  return {
    stop() {
      node.disconnect();
      source.disconnect();
      stream.getTracks().forEach((t) => t.stop());
      void ctx.close();
      const out = new Float32Array(chunks.reduce((n, c) => n + c.length, 0));
      let o = 0;
      for (const c of chunks) {
        out.set(c, o);
        o += c.length;
      }
      return out;
    },
  };
}

/**
 * Bouton de dictée : clic pour parler, re-clic pour arrêter. Transcription locale (Whisper, français).
 * Au premier usage, propose de télécharger le moteur (~570 Mo).
 */
export function VoiceButton({
  label,
  title,
  onText,
  disabled,
}: {
  label: string;
  title: string;
  onText: (text: string) => void | Promise<void>;
  disabled?: boolean;
}) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [info, setInfo] = useState("");
  const [level, setLevel] = useState(0);
  const [seconds, setSeconds] = useState(0);
  const rec = useRef<Recorder | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const startedAt = useRef(0);

  useEffect(
    () => () => {
      window.clearInterval(timer.current);
      rec.current?.stop();
    },
    [],
  );

  const flash = (text: string) => {
    setInfo(text);
    window.setTimeout(() => setInfo((cur) => (cur === text ? "" : cur)), 5000);
  };

  const ensureInstalled = async () => {
    const status = await invoke<VoiceStatus>("voice_status");
    if (status.ready) return true;
    const ok = window.confirm(
      "La dictée tourne en local sur ton PC (Whisper, très bon en français).\n\n" +
        "Il faut télécharger le moteur une seule fois : environ 570 Mo (whisper.cpp + modèle large-v3-turbo).\n\nTélécharger maintenant ?",
    );
    if (!ok) return false;
    setPhase("installing");
    setInfo("Téléchargement…");
    const unlisten = await listen<{ stage: string; done: number; total: number }>("voice-install-progress", (e) => {
      const { stage, done, total } = e.payload;
      if (stage === "engine") setInfo("Moteur…");
      else setInfo(total ? `Modèle ${Math.round((done / total) * 100)} %` : `Modèle ${Math.round(done / 1e6)} Mo`);
    });
    try {
      await invoke("voice_install");
      setInfo("");
      return true;
    } catch (error) {
      flash(`Échec : ${String(error)}`);
      return false;
    } finally {
      unlisten();
      setPhase("idle");
    }
  };

  const stop = async () => {
    const r = rec.current;
    rec.current = null;
    window.clearInterval(timer.current);
    if (!r) return;
    const samples = r.stop();
    const duration = samples.length / RATE;
    let peak = 0;
    for (let i = 0; i < samples.length; i += 8) peak = Math.max(peak, Math.abs(samples[i]));
    if (duration < 0.4 || peak < 0.02) {
      setPhase("idle");
      flash("Rien entendu (micro coupé ?)");
      return;
    }
    setPhase("working");
    try {
      const text = await invoke<string>("voice_transcribe", {
        wavB64: toBase64(encodeWav(samples)),
        seconds: duration,
      });
      if (text.trim()) await onText(text.trim());
      else flash("Rien compris, réessaie");
    } catch (error) {
      flash(String(error));
    } finally {
      setPhase("idle");
    }
  };

  const start = async () => {
    if (!(await ensureInstalled())) return;
    try {
      rec.current = await startRecorder(setLevel);
    } catch (error) {
      flash(`Micro indisponible : ${String(error)}`);
      return;
    }
    startedAt.current = Date.now();
    setSeconds(0);
    setPhase("recording");
    timer.current = window.setInterval(() => {
      const s = (Date.now() - startedAt.current) / 1000;
      setSeconds(Math.floor(s));
      if (s >= MAX_SECONDS) void stop();
    }, 250);
  };

  const click = () => {
    if (phase === "recording") void stop();
    else if (phase === "idle") void start();
  };

  const text =
    phase === "recording"
      ? `■ ${seconds}s`
      : phase === "working"
        ? "Transcription…"
        : phase === "installing"
          ? info || "Installation…"
          : label;

  return (
    <button
      type="button"
      className={`btn voice-btn ${phase}`}
      title={info || (phase === "recording" ? "Clique pour arrêter et transcrire" : title)}
      disabled={disabled || phase === "working" || phase === "installing"}
      onClick={click}
      style={phase === "recording" ? { boxShadow: `0 0 0 ${2 + Math.round(level * 10)}px rgba(196, 92, 74, 0.35)` } : undefined}
    >
      {text}
      {info && phase === "idle" ? <span className="voice-info">{info}</span> : null}
    </button>
  );
}
