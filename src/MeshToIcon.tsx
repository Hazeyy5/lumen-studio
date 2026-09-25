import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { MeshStill } from "./MeshPreview";
import type { BankItem } from "./types";

type ModelViewerEl = HTMLElement & {
  shadowRoot: ShadowRoot | null;
  cameraOrbit: string;
  cameraTarget: string;
  jumpCameraToGoal: () => void;
  getCameraOrbit: () => { theta: number; phi: number; radius: number };
  toBlob: (options?: { mimeType?: string }) => Promise<Blob>;
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

function applyCamera(el: ModelViewerEl, zoom: number, vertical: number, horizontal: number) {
  const radius = Math.min(24, Math.max(0.2, 2.6 / zoom));
  let theta = "auto";
  let phi = "auto";
  try {
    const orbit = el.getCameraOrbit();
    if (orbit && Number.isFinite(orbit.theta) && Number.isFinite(orbit.phi) && orbit.radius > 0.05) {
      theta = `${orbit.theta}rad`;
      phi = `${orbit.phi}rad`;
    }
  } catch {
    /* le modèle n’est pas encore cadré */
  }
  const y = vertical * radius * 0.35;
  const x = horizontal * radius * 0.35;
  el.cameraOrbit = `${theta} ${phi} ${radius.toFixed(3)}m`;
  el.cameraTarget = `${x.toFixed(3)}m ${y.toFixed(3)}m 0m`;
  el.jumpCameraToGoal?.();
}

function grabVisible(el: HTMLElement) {
  const root = el.shadowRoot;
  if (!root) return null;
  const webgl = root.querySelector("#webgl-canvas");
  const shown = root.querySelector("canvas.show");
  const canvas =
    webgl instanceof HTMLCanvasElement && webgl.classList.contains("show")
      ? webgl
      : shown instanceof HTMLCanvasElement
        ? shown
        : null;
  if (!canvas || canvas.width < 2 || canvas.height < 2) return null;
  const viewW = Math.max(1, el.clientWidth);
  const viewH = Math.max(1, el.clientHeight);
  const cssW = canvas.clientWidth || viewW;
  const cssH = canvas.clientHeight || viewH;
  const srcW = Math.min(canvas.width, Math.max(1, Math.round(viewW * (canvas.width / cssW))));
  const srcH = Math.min(canvas.height, Math.max(1, Math.round(viewH * (canvas.height / cssH))));
  const srcY = canvas.id === "webgl-canvas" ? Math.max(0, canvas.height - srcH) : 0;
  const shot = document.createElement("canvas");
  shot.width = srcW;
  shot.height = srcH;
  const ctx = shot.getContext("2d");
  if (!ctx) return null;
  ctx.drawImage(canvas, 0, srcY, srcW, srcH, 0, 0, srcW, srcH);
  return shot;
}

function paintFrame(frame: HTMLCanvasElement, preview: HTMLCanvasElement | null) {
  if (!preview) return;
  const ctx = preview.getContext("2d");
  if (!ctx) return;
  ctx.clearRect(0, 0, preview.width, preview.height);
  ctx.drawImage(frame, 0, 0, preview.width, preview.height);
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
  const stageRef = useRef<HTMLCanvasElement | null>(null);
  const previewRef = useRef<HTMLCanvasElement | null>(null);
  const [path, setPath] = useState(initialPath || "");
  const [src, setSrc] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [vertical, setVertical] = useState(0);
  const [horizontal, setHorizontal] = useState(0);
  const [outline, setOutline] = useState(true);
  const [color, setColor] = useState("#111111");
  const [thickness, setThickness] = useState(4);
  const [shadow, setShadow] = useState(true);
  const [opacity, setOpacity] = useState(0.35);
  const [blur, setBlur] = useState(16);
  const [offsetY, setOffsetY] = useState(12);
  const [resolution, setResolution] = useState(512);
  const [framed, setFramed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState("");
  const [err, setErr] = useState("");

  const opts: IconOpts = { outline, color, thickness, shadow, opacity, blur, offsetY };
  const optsRef = useRef(opts);
  optsRef.current = opts;

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
    const apply = () => applyCamera(el, zoom, vertical, horizontal);
    el.addEventListener("load", apply);
    apply();
    return () => el.removeEventListener("load", apply);
  }, [zoom, vertical, horizontal, src]);

  useEffect(() => {
    document.body.classList.add("icon-capture");
    return () => document.body.classList.remove("icon-capture");
  }, []);

  useEffect(() => {
    const el = viewerRef.current;
    if (!el || !src) return;
    let alive = true;
    let timer = 0;
    let pending = false;
    let again = false;
    const draw = () => {
      if (!alive) return;
      if (pending) {
        again = true;
        return;
      }
      const shot = grabVisible(el);
      if (!shot) return;
      pending = true;
      try {
        const frame = composeIcon(shot, 512, optsRef.current);
        paintFrame(frame, stageRef.current);
        paintFrame(frame, previewRef.current);
        if (alive) setFramed((value) => value || true);
      } finally {
        pending = false;
        if (again && alive) {
          again = false;
          timer = window.setTimeout(draw, 30);
        }
      }
    };
    const schedule = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(draw, 40);
    };
    el.addEventListener("camera-change", schedule);
    el.addEventListener("load", schedule);
    const kick = window.setTimeout(schedule, 120);
    return () => {
      alive = false;
      window.clearTimeout(timer);
      window.clearTimeout(kick);
      el.removeEventListener("camera-change", schedule);
      el.removeEventListener("load", schedule);
    };
  }, [src, zoom, vertical, horizontal, outline, color, thickness, shadow, opacity, blur, offsetY]);

  function framedIcon(size: number) {
    const el = viewerRef.current;
    if (!el) return null;
    const shot = grabVisible(el);
    if (!shot) return null;
    return composeIcon(shot, size, opts);
  }

  async function download() {
    try {
      const frame = framedIcon(resolution);
      if (!frame) return;
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
    setBusy(true);
    setErr("");
    let dataUrl = "";
    try {
      const frame = framedIcon(resolution);
      if (!frame) {
        setBusy(false);
        return;
      }
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
                  className={framed ? "icon-viewer is-hidden" : "icon-viewer"}
                  src={src}
                  camera-controls
                  disable-pan
                  interaction-prompt="none"
                  shadow-intensity="0"
                  environment-image="neutral"
                  min-camera-orbit="auto auto 0.2m"
                  max-camera-orbit="auto auto 24m"
                />
              ) : (
                <p className="lede">Choisis un modèle .glb</p>
              )}
              <canvas ref={stageRef} className="icon-overlay" width={512} height={512} />
            </div>
            <div className="icon-preview-row">
              <canvas ref={previewRef} className="icon-preview" width={256} height={256} />
              <p className="icon-hint">
                Glisse le modèle pour le tourner. Le grand aperçu et la miniature montrent le contour
                et l’ombre.
              </p>
            </div>
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
            <label>
              Décalage horizontal
              <input
                type="range"
                min={-1}
                max={1}
                step={0.05}
                value={horizontal}
                onChange={(event) => setHorizontal(Number(event.target.value))}
              />
              <span>{horizontal.toFixed(2)}</span>
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

const MODEL_PAGE = 15;

function ModelThumb({ item }: { item: BankItem }) {
  if (item.previewPath) {
    const src = item.previewPath.startsWith("http")
      ? item.previewPath
      : convertFileSrc(item.previewPath);
    return <img className="model-pick-img" src={src} alt="" loading="lazy" decoding="async" />;
  }
  if (!item.path.startsWith("http")) {
    return <MeshStill id={item.id} path={item.path} previewPath={item.previewPath} className="model-pick-still" />;
  }
  return <span className="model-pick-fallback">3D</span>;
}

export function ModelToIconPage({ projectPath }: { projectPath: string | null }) {
  const [query, setQuery] = useState("");
  const [needle, setNeedle] = useState("");
  const [items, setItems] = useState<BankItem[]>([]);
  const [available, setAvailable] = useState(0);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [err, setErr] = useState("");
  const [loading, setLoading] = useState(true);
  const [picked, setPicked] = useState<string | null>(null);

  useEffect(() => {
    const id = window.setTimeout(() => setNeedle(query.trim()), 220);
    return () => window.clearTimeout(id);
  }, [query]);

  useEffect(() => {
    setPage(0);
  }, [needle]);

  useEffect(() => {
    let cancel = false;
    void invoke<{ vibeMeshes: number }>("bank_counts")
      .then((counts) => {
        if (!cancel) setAvailable(counts.vibeMeshes);
      })
      .catch(() => {});
    return () => {
      cancel = true;
    };
  }, []);

  useEffect(() => {
    let cancel = false;
    setLoading(true);
    void invoke<{ total: number; items: BankItem[] }>("bank_page", {
      shelf: "vibestarter",
      kind: "mesh",
      query: needle,
      offset: page * MODEL_PAGE,
      limit: MODEL_PAGE,
    })
      .then((result) => {
        if (cancel) return;
        setItems(result.items);
        setTotal(result.total);
        setLoading(false);
      })
      .catch((error) => {
        if (cancel) return;
        setErr(String(error));
        setLoading(false);
      });
    return () => {
      cancel = true;
    };
  }, [needle, page]);

  const pages = Math.max(1, Math.ceil(total / MODEL_PAGE));

  return (
    <div className="model-page">
      <p className="atelier-kicker">Atelier · Conversion</p>
      <h1>Modèle → 2D</h1>
      <p className="lede model-lede">
        Rends un modèle 3D existant en image 2D. Choisis un modèle dans la bibliothèque pour démarrer.
      </p>
      <div className="model-count">
        <i />
        Modèles disponibles <strong>{available.toLocaleString("fr-FR")}</strong> assets
      </div>
      <label className="model-search">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="11" cy="11" r="6.5" />
          <path d="M16 16.5 20 20.5" />
        </svg>
        <input
          type="search"
          value={query}
          placeholder="Rechercher des assets (en anglais)..."
          onChange={(event) => setQuery(event.target.value)}
        />
      </label>
      {err ? <p className="err">{err}</p> : null}
      <div className="model-grid">
        {items.map((item) => (
          <button
            key={item.id}
            className="model-pick"
            type="button"
            onClick={() => setPicked(item.path)}
          >
            <span className="model-pick-thumb">
              <ModelThumb item={item} />
            </span>
            <span className="model-pick-name">{item.name}</span>
          </button>
        ))}
      </div>
      {!loading && items.length === 0 ? (
        <p className="lede">Aucun modèle pour cette recherche.</p>
      ) : null}
      {total > MODEL_PAGE ? (
        <div className="bank-pager">
          <button
            className="btn secondary"
            type="button"
            disabled={page === 0}
            onClick={() => setPage((value) => Math.max(0, value - 1))}
          >
            Précédent
          </button>
          <span>
            {page + 1} / {pages}
          </span>
          <button
            className="btn secondary"
            type="button"
            disabled={page + 1 >= pages}
            onClick={() => setPage((value) => value + 1)}
          >
            Suivant
          </button>
        </div>
      ) : null}
      {picked ? (
        <MeshToIcon
          projectPath={projectPath}
          initialPath={picked}
          onClose={() => setPicked(null)}
        />
      ) : null}
    </div>
  );
}
