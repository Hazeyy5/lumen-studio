import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";

type ModelViewerEl = HTMLElement & {
  shadowRoot: ShadowRoot | null;
};

type IconOpts = {
  outline: boolean;
  color: string;
  thickness: number;
  shadow: boolean;
  opacity: number;
  blur: number;
  offsetY: number;
};

function composeIcon(source: CanvasImageSource, size: number, opts: IconOpts) {
  const pad = Math.ceil(opts.thickness + opts.blur + Math.abs(opts.offsetY)) + 8;
  const inner = Math.max(32, size - pad * 2);
  const model = document.createElement("canvas");
  model.width = size;
  model.height = size;
  const mctx = model.getContext("2d");
  if (!mctx) return model;
  mctx.drawImage(source, pad, pad, inner, inner);

  const outlined = document.createElement("canvas");
  outlined.width = size;
  outlined.height = size;
  const octx = outlined.getContext("2d");
  if (!octx) return model;
  if (opts.outline && opts.thickness > 0) {
    const stamp = document.createElement("canvas");
    stamp.width = size;
    stamp.height = size;
    const sctx = stamp.getContext("2d");
    if (sctx) {
      const steps = Math.max(12, Math.round(opts.thickness * 3));
      for (let i = 0; i < steps; i += 1) {
        const angle = (i / steps) * Math.PI * 2;
        sctx.drawImage(
          model,
          Math.cos(angle) * opts.thickness,
          Math.sin(angle) * opts.thickness,
        );
      }
      sctx.globalCompositeOperation = "source-in";
      sctx.fillStyle = opts.color;
      sctx.fillRect(0, 0, size, size);
      octx.drawImage(stamp, 0, 0);
    }
  }
  octx.drawImage(model, 0, 0);

  const out = document.createElement("canvas");
  out.width = size;
  out.height = size;
  const ctx = out.getContext("2d");
  if (!ctx) return outlined;
  if (opts.shadow) {
    ctx.save();
    ctx.filter = `blur(${opts.blur}px)`;
    ctx.globalAlpha = opts.opacity;
    ctx.drawImage(outlined, 0, opts.offsetY);
    ctx.restore();
  }
  ctx.drawImage(outlined, 0, 0);
  return out;
}

function blobUrlFromBase64(b64: string) {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return URL.createObjectURL(new Blob([bytes], { type: "model/gltf-binary" }));
}

async function loadModel(path: string) {
  const local =
    path.startsWith("https://") || path.startsWith("http://")
      ? await invoke<string>("cache_remote_asset", { url: path })
      : path;
  try {
    const b64 = await invoke<string>("read_lumen_file", { path: local });
    return { url: blobUrlFromBase64(b64), revoke: true };
  } catch {
    return { url: convertFileSrc(local), revoke: false };
  }
}

export function MeshToIcon({
  projectPath,
  initialPath,
  onClose,
}: {
  projectPath: string | null;
  initialPath?: string | null;
  onClose: () => void;
}) {
  const viewerRef = useRef<ModelViewerEl | null>(null);
  const previewRef = useRef<HTMLCanvasElement | null>(null);
  const [path, setPath] = useState(initialPath || "");
  const [src, setSrc] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [vertical, setVertical] = useState(0);
  const [outline, setOutline] = useState(true);
  const [color, setColor] = useState("#111111");
  const [thickness, setThickness] = useState(4);
  const [shadow, setShadow] = useState(true);
  const [opacity, setOpacity] = useState(0.35);
  const [blur, setBlur] = useState(16);
  const [offsetY, setOffsetY] = useState(12);
  const [resolution, setResolution] = useState(512);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState("");
  const [err, setErr] = useState("");

  const opts: IconOpts = { outline, color, thickness, shadow, opacity, blur, offsetY };

  useEffect(() => {
    if (!path) {
      setSrc(null);
      return;
    }
    let cancel = false;
    let revoke: string | null = null;
    void loadModel(path)
      .then((loaded) => {
        if (cancel) {
          if (loaded.revoke) URL.revokeObjectURL(loaded.url);
          return;
        }
        if (loaded.revoke) revoke = loaded.url;
        setSrc(loaded.url);
      })
      .catch((error) => {
        if (!cancel) setErr(String(error));
      });
    return () => {
      cancel = true;
      if (revoke) URL.revokeObjectURL(revoke);
    };
  }, [path]);

  useEffect(() => {
    const el = viewerRef.current;
    if (!el || !src) return;
    el.setAttribute("camera-orbit", `auto auto ${(2.4 / zoom).toFixed(2)}m`);
    el.setAttribute("camera-target", `auto auto ${vertical.toFixed(2)}m`);
  }, [zoom, vertical, src]);

  useEffect(() => {
    if (!src) return;
    const tick = () => {
      const source = viewerRef.current?.shadowRoot?.querySelector("canvas");
      const preview = previewRef.current;
      if (!source || !preview) return;
      const frame = composeIcon(source, 512, opts);
      const ctx = preview.getContext("2d");
      if (!ctx) return;
      ctx.clearRect(0, 0, preview.width, preview.height);
      ctx.drawImage(frame, 0, 0, preview.width, preview.height);
    };
    const id = window.setInterval(tick, 160);
    return () => window.clearInterval(id);
  }, [src, outline, color, thickness, shadow, opacity, blur, offsetY]);

  function currentFrame(size: number) {
    const source = viewerRef.current?.shadowRoot?.querySelector("canvas");
    if (!source) return null;
    return composeIcon(source, size, opts);
  }

  function download() {
    const frame = currentFrame(resolution);
    if (!frame) return;
    try {
      const link = document.createElement("a");
      link.href = frame.toDataURL("image/png");
      link.download = `icone-${resolution}.png`;
      link.click();
    } catch {
      setErr("Impossible d’exporter cette vue. Choisis un modèle déjà dans Lumen ou dans le projet.");
    }
  }

  async function save() {
    if (!projectPath) {
      setErr("Ouvre un projet pour enregistrer l’icône dans la banque.");
      return;
    }
    const frame = currentFrame(resolution);
    if (!frame) return;
    setBusy(true);
    setErr("");
    let dataUrl = "";
    try {
      dataUrl = frame.toDataURL("image/png");
    } catch {
      setBusy(false);
      setErr("Impossible d’exporter cette vue. Choisis un modèle déjà dans Lumen ou dans le projet.");
      return;
    }
    try {
      const saved = await invoke<{ code: string }>("save_image_to_project", {
        projectPath,
        dataUrl,
        filename: `icone-${Date.now()}.png`,
      });
      setNote(`Enregistré · ${saved.code}`);
    } catch (error) {
      setErr(String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="lightbox" onClick={onClose} role="presentation">
      <div
        className="lightbox-panel icon-dialog"
        role="dialog"
        aria-label="Modèle vers icône 2D"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="lightbox-head">
          <h2>Modèle vers icône 2D</h2>
          <button className="btn secondary" type="button" onClick={onClose}>
            Fermer
          </button>
        </div>
        <div className="icon-layout">
          <div className="icon-stage">
            <div className="icon-checker">
              {src ? (
                <model-viewer
                  ref={(node) => {
                    viewerRef.current = node as ModelViewerEl | null;
                  }}
                  className="icon-viewer"
                  src={src}
                  camera-controls
                  interaction-prompt="none"
                  shadow-intensity="0"
                  environment-image="neutral"
                />
              ) : (
                <p className="lede">Choisis un modèle .glb</p>
              )}
              <canvas ref={previewRef} className="icon-overlay" width={512} height={512} />
            </div>
            <p className="icon-hint">Glisser pour pivoter · les curseurs règlent le cadrage</p>
          </div>
          <div className="icon-controls">
            <button
              className="btn secondary"
              type="button"
              onClick={async () => {
                const picked = await open({
                  multiple: false,
                  filters: [{ name: "Modèle", extensions: ["glb", "gltf"] }],
                });
                if (typeof picked === "string") {
                  setPath(picked);
                  setNote("");
                  setErr("");
                }
              }}
            >
              Choisir un modèle
            </button>
            <label>
              Zoom
              <input
                type="range"
                min={0.6}
                max={2.4}
                step={0.05}
                value={zoom}
                onChange={(event) => setZoom(Number(event.target.value))}
              />
              <span>{zoom.toFixed(2)}</span>
            </label>
            <label>
              Décalage vertical
              <input
                type="range"
                min={-1}
                max={1}
                step={0.05}
                value={vertical}
                onChange={(event) => setVertical(Number(event.target.value))}
              />
              <span>{vertical.toFixed(2)}</span>
            </label>
            <label className="row">
              <input type="checkbox" checked={outline} onChange={(event) => setOutline(event.target.checked)} />
              Contour
            </label>
            {outline ? (
              <>
                <label>
                  Couleur
                  <input type="color" value={color} onChange={(event) => setColor(event.target.value)} />
                </label>
                <label>
                  Épaisseur
                  <input
                    type="range"
                    min={1}
                    max={16}
                    step={1}
                    value={thickness}
                    onChange={(event) => setThickness(Number(event.target.value))}
                  />
                  <span>{thickness} px</span>
                </label>
              </>
            ) : null}
            <label className="row">
              <input type="checkbox" checked={shadow} onChange={(event) => setShadow(event.target.checked)} />
              Ombre portée
            </label>
            {shadow ? (
              <>
                <label>
                  Opacité
                  <input
                    type="range"
                    min={0}
                    max={0.8}
                    step={0.05}
                    value={opacity}
                    onChange={(event) => setOpacity(Number(event.target.value))}
                  />
                  <span>{opacity.toFixed(2)}</span>
                </label>
                <label>
                  Flou
                  <input
                    type="range"
                    min={0}
                    max={32}
                    step={1}
                    value={blur}
                    onChange={(event) => setBlur(Number(event.target.value))}
                  />
                  <span>{blur} px</span>
                </label>
                <label>
                  Décalage Y
                  <input
                    type="range"
                    min={0}
                    max={40}
                    step={1}
                    value={offsetY}
                    onChange={(event) => setOffsetY(Number(event.target.value))}
                  />
                  <span>{offsetY} px</span>
                </label>
              </>
            ) : null}
            <div className="icon-res">
              <span>Résolution</span>
              {[256, 512, 1024].map((value) => (
                <button
                  key={value}
                  type="button"
                  className={`btn ${resolution === value ? "copper" : "secondary"}`}
                  onClick={() => setResolution(value)}
                >
                  {value}
                </button>
              ))}
            </div>
          </div>
        </div>
        <div className="icon-foot">
          {err ? <p className="err">{err}</p> : null}
          {note ? <p className="lede">{note}</p> : null}
          <button className="btn secondary" type="button" onClick={onClose}>
            Annuler
          </button>
          <button className="btn secondary" type="button" disabled={!src} onClick={download}>
            Télécharger
          </button>
          <button className="btn copper" type="button" disabled={!src || busy} onClick={() => void save()}>
            Enregistrer dans la banque
          </button>
        </div>
      </div>
    </div>
  );
}
