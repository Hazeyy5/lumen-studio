import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  encodeWav,
  makePreset,
  PRESETS,
  renderSfx,
  SAMPLE_RATE,
  type SfxSpec,
} from "../src-tauri/resources/lumen-sfx-core.mjs";

type SfxFile = {
  name: string;
  text: string;
  modified: number;
  wavPath?: string | null;
  assetId?: string | null;
  stale: boolean;
};

let audioCtx: AudioContext | null = null;

function play(samples: Float32Array) {
  audioCtx ??= new AudioContext();
  const buffer = audioCtx.createBuffer(1, samples.length, SAMPLE_RATE);
  buffer.copyToChannel(new Float32Array(samples), 0);
  const src = audioCtx.createBufferSource();
  src.buffer = buffer;
  src.connect(audioCtx.destination);
  src.start();
}

function toBase64(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}

function Waveform({ samples }: { samples: Float32Array | null }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const w = (canvas.width = canvas.clientWidth * devicePixelRatio);
    const h = (canvas.height = canvas.clientHeight * devicePixelRatio);
    const g = canvas.getContext("2d");
    if (!g) return;
    g.clearRect(0, 0, w, h);
    g.fillStyle = getComputedStyle(canvas).color;
    if (!samples || !samples.length) return;
    const step = samples.length / w;
    for (let x = 0; x < w; x++) {
      let lo = 0;
      let hi = 0;
      const end = Math.min(samples.length, Math.floor((x + 1) * step));
      for (let i = Math.floor(x * step); i < end; i++) {
        if (samples[i] < lo) lo = samples[i];
        if (samples[i] > hi) hi = samples[i];
      }
      g.fillRect(x, h / 2 - (hi * h) / 2, 1, Math.max(1, ((hi - lo) * h) / 2));
    }
  }, [samples]);
  return <canvas className="sfx-wave" ref={ref} />;
}

const PRESET_KEYS = Object.keys(PRESETS);

export function SfxPage({ projectPath, engine }: { projectPath: string; engine: string }) {
  const roblox = engine !== "uefn" && engine !== "unreal";
  const editorName = engine === "uefn" ? "UEFN" : "Unreal";
  const [files, setFiles] = useState<SfxFile[]>([]);
  const [selected, setSelected] = useState("");
  const [busy, setBusy] = useState("");
  const [msg, setMsg] = useState("");
  // Générateur
  const [preset, setPreset] = useState("coin");
  const [seed, setSeed] = useState(() => Math.floor(Math.random() * 1e6));
  const [pitch, setPitch] = useState(1);
  const [length, setLength] = useState(1);
  const [name, setName] = useState("");

  useEffect(() => {
    let alive = true;
    const load = () =>
      invoke<SfxFile[]>("list_sfx", { projectPath })
        .then((list) => {
          if (!alive) return;
          setFiles((prev) => (JSON.stringify(prev) === JSON.stringify(list) ? prev : list));
        })
        .catch(() => undefined);
    void load();
    const timer = window.setInterval(load, 1500);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [projectPath]);

  const current = files.find((f) => f.name === selected) ?? null;
  const currentRender = useMemo(() => {
    if (!current) return { samples: null as Float32Array | null, error: "" };
    try {
      return { samples: renderSfx(JSON.parse(current.text) as SfxSpec), error: "" };
    } catch (error) {
      return { samples: null, error: String(error) };
    }
  }, [current?.text]);

  const draft = useMemo(() => makePreset(preset, { seed, pitch, length }), [preset, seed, pitch, length]);
  const draftSamples = useMemo(() => renderSfx(draft), [draft]);

  const flash = (text: string) => {
    setMsg(text);
    window.setTimeout(() => setMsg((cur) => (cur === text ? "" : cur)), 6000);
  };

  const reroll = () => {
    const next = Math.floor(Math.random() * 1e6);
    setSeed(next);
    play(renderSfx(makePreset(preset, { seed: next, pitch, length })));
  };

  async function saveDraft() {
    const clean = name.trim();
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(clean)) {
      flash("Nom : lettres, chiffres et _, en commençant par une lettre (ex. CoinPickup).");
      return;
    }
    if (files.some((f) => f.name === clean) && !window.confirm(`Remplacer le son « ${clean} » ?`)) return;
    setBusy("save");
    try {
      const spec = { ...draft, name: clean };
      await invoke("save_sfx", {
        projectPath,
        name: clean,
        spec: JSON.stringify(spec, null, 2),
        wavB64: toBase64(encodeWav(draftSamples)),
      });
      setSelected(clean);
      flash(`${clean} enregistré dans assets/sfx/.`);
    } catch (error) {
      flash(String(error));
    } finally {
      setBusy("");
    }
  }

  async function publish(file: SfxFile) {
    setBusy("publish");
    try {
      if (!roblox) {
        const asset = await invoke<string>("import_sfx", { projectPath, name: file.name });
        flash(`${file.name} importé dans ${editorName} : ${asset}`);
        return;
      }
      const id = await invoke<string>("publish_sfx", { projectPath, name: file.name });
      flash(`${file.name} publié : ${id}. Les agents l'utilisent après « node tools/lumen-sfx.mjs sync ».`);
    } catch (error) {
      flash(String(error));
    } finally {
      setBusy("");
    }
  }

  async function remove(file: SfxFile) {
    if (!window.confirm(`Supprimer « ${file.name} » (JSON et WAV) ?`)) return;
    await invoke("delete_sfx", { projectPath, name: file.name }).catch((e) => flash(String(e)));
    setSelected("");
  }

  const seconds = (s: Float32Array | null) => (s ? (s.length / SAMPLE_RATE).toFixed(2) : "0");

  return (
    <div className="vfx-page sfx-page">
      <h1>Sons</h1>
      <p className="lede">
        Bruitages du projet (<code>assets/sfx/</code>), générés par Lumen : écoute ceux des agents, crée les tiens,
        {roblox ? "publie-les sur Roblox" : `importe-les dans ${editorName} (l'éditeur doit être ouvert sur ce projet)`}. Les variantes <code>_v1</code>, <code>_v2</code>… sont des propositions à départager.
      </p>
      <div className="vfx-layout">
        <aside className="vfx-list">
          {files.length === 0 ? <p className="muted">Aucun son dans assets/sfx/.</p> : null}
          {files.map((f) => (
            <button
              key={f.name}
              type="button"
              className={f.name === selected ? "on" : ""}
              onClick={() => {
                setSelected(f.name);
                try {
                  play(renderSfx(JSON.parse(f.text) as SfxSpec));
                } catch {
                  /* JSON invalide : affiché à droite */
                }
              }}
            >
              ▶ {f.name}
              {f.assetId ? <small className="sfx-tag">{f.stale ? " · modifié" : roblox ? " · publié" : " · importé"}</small> : null}
            </button>
          ))}
        </aside>
        <div className="vfx-stage-wrap">
          {current ? (
            <section className="sfx-card">
              <header>
                <strong>{current.name}</strong>
                <span className="muted">{seconds(currentRender.samples)} s</span>
              </header>
              {currentRender.error ? <p className="vfx-error">JSON invalide : {currentRender.error}</p> : null}
              <Waveform samples={currentRender.samples} />
              <div className="vfx-controls">
                <button
                  type="button"
                  className="btn copper"
                  disabled={!currentRender.samples}
                  onClick={() => currentRender.samples && play(currentRender.samples)}
                >
                  ▶ Écouter
                </button>
                <button
                  type="button"
                  className="btn secondary"
                  disabled={busy !== "" || !current.wavPath || /_v\d+$/.test(current.name)}
                  title={
                    /_v\d+$/.test(current.name)
                      ? "Garde d'abord la variante choisie (l'agent fait « keep »)"
                      : roblox
                        ? "Publier le WAV sur Roblox"
                        : `Importer le WAV dans le projet ${editorName} ouvert (Lumen/Sfx)`
                  }
                  onClick={() => void publish(current)}
                >
                  {busy === "publish"
                    ? roblox
                      ? "Publication…"
                      : "Import…"
                    : roblox
                      ? current.assetId && !current.stale
                        ? "Republier"
                        : "Publier sur Roblox"
                      : current.assetId && !current.stale
                        ? "Réimporter"
                        : "Importer dans le projet"}
                </button>
                {current.assetId ? (
                  <button
                    type="button"
                    className="btn secondary"
                    title="Copier l'id"
                    onClick={() => void navigator.clipboard.writeText(current.assetId ?? "")}
                  >
                    {current.assetId}
                    {current.stale ? " (ancien)" : ""}
                  </button>
                ) : null}
                <button type="button" className="btn secondary" onClick={() => void remove(current)}>
                  Supprimer
                </button>
              </div>
            </section>
          ) : (
            <p className="muted">Choisis un son à gauche pour l'écouter.</p>
          )}

          <section className="sfx-card">
            <header>
              <strong>Créer un bruitage</strong>
              <span className="muted">{seconds(draftSamples)} s</span>
            </header>
            <div className="sfx-presets">
              {PRESET_KEYS.map((k) => (
                <button
                  key={k}
                  type="button"
                  className={`btn secondary ${k === preset ? "on" : ""}`}
                  onClick={() => {
                    setPreset(k);
                    play(renderSfx(makePreset(k, { seed, pitch, length })));
                  }}
                >
                  {PRESETS[k].label}
                </button>
              ))}
            </div>
            <Waveform samples={draftSamples} />
            <div className="sfx-sliders">
              <label>
                Hauteur ×{pitch.toFixed(2)}
                <input type="range" min={0.5} max={2} step={0.05} value={pitch} onChange={(e) => setPitch(Number(e.target.value))} onPointerUp={() => play(draftSamples)} />
              </label>
              <label>
                Durée ×{length.toFixed(2)}
                <input type="range" min={0.4} max={2} step={0.05} value={length} onChange={(e) => setLength(Number(e.target.value))} onPointerUp={() => play(draftSamples)} />
              </label>
            </div>
            <div className="vfx-controls">
              <button type="button" className="btn secondary" onClick={() => play(draftSamples)}>
                ▶ Écouter
              </button>
              <button type="button" className="btn secondary" onClick={reroll}>
                🎲 Autre variante
              </button>
              <input
                className="sfx-name"
                value={name}
                placeholder="Nom (ex. CoinPickup)"
                onChange={(e) => setName(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && void saveDraft()}
              />
              <button type="button" className="btn copper" disabled={busy !== ""} onClick={() => void saveDraft()}>
                {busy === "save" ? "…" : "Enregistrer"}
              </button>
            </div>
          </section>
          {msg ? <p className="sfx-msg">{msg}</p> : null}
        </div>
      </div>
    </div>
  );
}
