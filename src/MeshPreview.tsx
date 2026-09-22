import { useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import "@google/model-viewer";

declare module "react" {
  namespace JSX {
    interface IntrinsicElements {
      "model-viewer": React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement> & {
        src?: string;
        "camera-controls"?: boolean;
        "auto-rotate"?: boolean;
        "shadow-intensity"?: string;
        "interaction-prompt"?: string;
        "camera-orbit"?: string;
        "field-of-view"?: string;
        "environment-image"?: string;
        reveal?: string;
      };
    }
  }
}

type ModelViewerEl = HTMLElement & {
  src: string;
  toBlob?: (opts?: { mimeType?: string; idealAspect?: boolean }) => Promise<Blob>;
  updateComplete?: Promise<void>;
};

function blobFromBase64(b64: string, type: string) {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return new Blob([bytes], { type });
}

async function blobUrlFromDisk(path: string) {
  const b64 = await invoke<string>("read_lumen_file", { path });
  return URL.createObjectURL(blobFromBase64(b64, "model/gltf-binary"));
}

async function resolveModelUrl(path: string) {
  if (path.startsWith("https://") || path.startsWith("http://")) {
    return { url: path, revoke: false };
  }
  const url = convertFileSrc(path);
  if (url) return { url, revoke: false };
  return { url: await blobUrlFromDisk(path), revoke: true };
}

function blobToBase64(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const text = String(reader.result ?? "");
      const comma = text.indexOf(",");
      resolve(comma >= 0 ? text.slice(comma + 1) : text);
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

function frames(count: number) {
  return new Promise<void>((resolve) => {
    const step = (left: number) => {
      if (left <= 0) {
        resolve();
        return;
      }
      requestAnimationFrame(() => step(left - 1));
    };
    step(count);
  });
}

function canvasLooksEmpty(canvas: HTMLCanvasElement) {
  try {
    const tmp = document.createElement("canvas");
    tmp.width = 24;
    tmp.height = 24;
    const ctx = tmp.getContext("2d");
    if (!ctx) return false;
    ctx.drawImage(canvas, 0, 0, 24, 24);
    const data = ctx.getImageData(0, 0, 24, 24).data;
    let visible = 0;
    for (let i = 0; i < data.length; i += 4) {
      if (data[i + 3] > 16 && data[i] + data[i + 1] + data[i + 2] > 30) {
        visible += 1;
      }
    }
    return visible < 10;
  } catch {
    return false;
  }
}

async function grabStill(el: ModelViewerEl) {
  if (el.updateComplete) {
    await el.updateComplete;
  }
  await frames(6);
  await new Promise((resolve) => window.setTimeout(resolve, 180));
  const canvas = el.shadowRoot?.querySelector("canvas") as HTMLCanvasElement | null;
  if (canvas && canvas.width > 4 && !canvasLooksEmpty(canvas)) {
    const fromCanvas = await new Promise<Blob | null>((resolve) => {
      canvas.toBlob((blob) => resolve(blob), "image/png");
    });
    if (fromCanvas && fromCanvas.size > 1200) return fromCanvas;
  }
  if (typeof el.toBlob === "function") {
    const blob = await el.toBlob({ mimeType: "image/png", idealAspect: true });
    if (blob && blob.size > 1200) return blob;
  }
  return null;
}

export function MeshPreview({
  path,
  className = "bank-thumb bank-mesh",
}: {
  path: string;
  className?: string;
}) {
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let revoke: string | null = null;
    let cancel = false;
    void (async () => {
      try {
        const first = await resolveModelUrl(path);
        if (cancel) {
          if (first.revoke) URL.revokeObjectURL(first.url);
          return;
        }
        setSrc(first.url);
        if (first.revoke) revoke = first.url;
      } catch {
        try {
          const url = await blobUrlFromDisk(path);
          if (cancel) {
            URL.revokeObjectURL(url);
            return;
          }
          revoke = url;
          setSrc(url);
        } catch {
          if (!cancel) setFailed(true);
        }
      }
    })();
    return () => {
      cancel = true;
      if (revoke) URL.revokeObjectURL(revoke);
    };
  }, [path]);

  if (failed) {
    return <div className={`${className} mesh-skel`}>Aperçu indisponible</div>;
  }
  if (!src) {
    return <div className={`${className} mesh-skel`} />;
  }
  return (
    <model-viewer
      className={className}
      src={src}
      camera-controls
      auto-rotate
      interaction-prompt="none"
      shadow-intensity="0.55"
      environment-image="neutral"
    />
  );
}

const waiters: Array<() => void> = [];
let liveViewers = 0;
const MAX_LIVE = 4;

function acquireViewer() {
  if (liveViewers < MAX_LIVE) {
    liveViewers += 1;
    return Promise.resolve();
  }
  return new Promise<void>((resolve) => {
    waiters.push(() => {
      liveViewers += 1;
      resolve();
    });
  });
}

function releaseViewer() {
  liveViewers = Math.max(0, liveViewers - 1);
  waiters.shift()?.();
}

export function MeshStill({
  id,
  path,
  previewPath,
  className = "bank-thumb bank-mesh",
  onReady,
}: {
  id: string;
  path: string;
  previewPath?: string | null;
  className?: string;
  onReady?: (previewPath: string) => void;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewerRef = useRef<ModelViewerEl | null>(null);
  const [still, setStill] = useState<string | null>(
    previewPath ? convertFileSrc(previewPath) : null,
  );
  const [src, setSrc] = useState<string | null>(null);
  const [showViewer, setShowViewer] = useState(false);

  useEffect(() => {
    if (previewPath) setStill(convertFileSrc(previewPath));
  }, [previewPath]);

  useEffect(() => {
    if (still) return;
    const host = hostRef.current;
    if (!host) return;
    let alive = true;
    let held = false;
    let hideTimer: number | null = null;
    const io = new IntersectionObserver(
      (entries) => {
        const visible = entries.some((entry) => entry.isIntersecting);
        if (hideTimer) {
          window.clearTimeout(hideTimer);
          hideTimer = null;
        }
        if (visible) {
          if (held || !alive) return;
          void acquireViewer().then(() => {
            if (!alive) {
              releaseViewer();
              return;
            }
            held = true;
            setShowViewer(true);
          });
          return;
        }
        hideTimer = window.setTimeout(() => {
          if (!alive || still) return;
          if (held) {
            held = false;
            releaseViewer();
          }
          setShowViewer(false);
          setSrc(null);
        }, 900);
      },
      { rootMargin: "120px" },
    );
    io.observe(host);
    return () => {
      alive = false;
      io.disconnect();
      if (hideTimer) window.clearTimeout(hideTimer);
      if (held) releaseViewer();
    };
  }, [path, still]);

  useEffect(() => {
    if (!showViewer || still) return;
    let revoke: string | null = null;
    let cancel = false;
    void (async () => {
      try {
        const first = await resolveModelUrl(path);
        if (cancel) {
          if (first.revoke) URL.revokeObjectURL(first.url);
          return;
        }
        setSrc(first.url);
        if (first.revoke) revoke = first.url;
      } catch {
        try {
          const url = await blobUrlFromDisk(path);
          if (cancel) {
            URL.revokeObjectURL(url);
            return;
          }
          revoke = url;
          setSrc(url);
        } catch {
          if (!cancel) setShowViewer(false);
        }
      }
    })();
    return () => {
      cancel = true;
      if (revoke) URL.revokeObjectURL(revoke);
    };
  }, [path, showViewer, still]);

  useEffect(() => {
    const el = viewerRef.current;
    if (!el || !src || still) return;
    let cancel = false;
    const take = async () => {
      try {
        const blob = await grabStill(el);
        if (cancel || !blob) return;
        const local = URL.createObjectURL(blob);
        setStill(local);
        const png = await blobToBase64(blob);
        const saved = await invoke<string>("save_mesh_preview", {
          id,
          pngBase64: png,
        });
        onReady?.(saved);
      } catch {
        /* le model-viewer reste affiché, figé */
      }
    };
    const onLoad = () => {
      void take();
    };
    const onError = () => {
      void (async () => {
        try {
          const url = await blobUrlFromDisk(path);
          if (!cancel) setSrc(url);
        } catch {
          /* placeholder */
        }
      })();
    };
    el.addEventListener("load", onLoad);
    el.addEventListener("error", onError);
    const retry = window.setTimeout(() => {
      void take();
    }, 1600);
    return () => {
      cancel = true;
      el.removeEventListener("load", onLoad);
      el.removeEventListener("error", onError);
      window.clearTimeout(retry);
    };
  }, [id, path, src, still]);

  return (
    <div ref={hostRef} className={`${className} mesh-still-host`}>
      {showViewer && src && !still ? (
        <model-viewer
          ref={(node) => {
            viewerRef.current = node as ModelViewerEl | null;
          }}
          className="bank-mesh-still"
          src={src}
          interaction-prompt="none"
          shadow-intensity="0.55"
          camera-orbit="30deg 72deg auto"
          field-of-view="auto"
          environment-image="neutral"
          reveal="auto"
        />
      ) : null}
      {still ? <img className="mesh-still-img" src={still} alt="" /> : null}
      {!still && !src ? <div className="mesh-skel" /> : null}
    </div>
  );
}
