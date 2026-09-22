import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { AssetCode, bankImageSrc } from "./AssetLightbox";
import { MeshPreview, MeshStill } from "./MeshPreview";

export type ReviewPayload = {
  id: string;
  status: "ready" | "generating" | string;
  kind: string;
  origin?: string;
  name?: string;
  prompt: string;
  code?: string | null;
  path: string;
  previewPath?: string | null;
  robloxAssetId?: string | null;
  canRetry?: boolean;
  error?: string | null;
};

function isImagePath(path?: string | null) {
  const lower = (path || "").toLowerCase();
  return [".png", ".jpg", ".jpeg", ".webp", ".gif"].some((ext) => lower.endsWith(ext));
}

async function ensureNotifyPermission() {
  try {
    const mod = await import("@tauri-apps/plugin-notification");
    if (!(await mod.isPermissionGranted())) {
      await mod.requestPermission();
    }
  } catch {
    /* Le toast Windows part déjà du backend Lumen. */
  }
}

export type LibraryProposeItem = {
  code: string;
  name: string;
  kind: string;
  path: string;
  previewPath?: string | null;
  robloxAssetId?: string | null;
};

export type LibraryProposal = {
  id: string;
  query: string;
  purpose?: string;
  items: LibraryProposeItem[];
};

export function AssetReviewHost() {
  const [queue, setQueue] = useState<ReviewPayload[]>([]);
  const [proposals, setProposals] = useState<LibraryProposal[]>([]);
  const [generating, setGenerating] = useState<{ kind: string; prompt: string } | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void ensureNotifyPermission();
    }, 1200);
    void invoke<ReviewPayload[]>("pending_asset_reviews")
      .then((items) => {
        setQueue(items);
        if (items[0]) {
          setPrompt(items[0].prompt);
        }
      })
      .catch(() => undefined);
    void invoke<LibraryProposal[]>("pending_library_choices")
      .then((items) => {
        if (items.length) setProposals(items);
      })
      .catch(() => undefined);

    const unlisten = listen<ReviewPayload>("asset-review", (event) => {
      const next = event.payload;
      if (next.status === "generating") {
        setGenerating({ kind: next.kind, prompt: next.prompt });
        setPrompt(next.prompt);
        setBusy(false);
        setErr("");
        return;
      }
      setGenerating(null);
      setQueue((prev) => {
        const rest = prev.filter((item) => item.id !== next.id);
        return [...rest, next];
      });
      setPrompt(next.prompt);
      setBusy(false);
      setErr("");
    });
    const unlistenPropose = listen<LibraryProposal>("library-propose", (event) => {
      setProposals((prev) =>
        [event.payload, ...prev.filter((item) => item.id !== event.payload.id)].slice(0, 2),
      );
    });
    return () => {
      window.clearTimeout(timer);
      void unlisten.then((fn) => fn());
      void unlistenPropose.then((fn) => fn());
    };
  }, []);

  const current = queue[0] ?? null;

  useEffect(() => {
    if (!current) return;
    setCollapsed(false);
    setPrompt(current.prompt);
    setErr("");
  }, [current?.id]);

  async function resolveChoice(proposal: LibraryProposal, code: string | null) {
    if (busy) return;
    setBusy(true);
    setErr("");
    try {
      await invoke("resolve_library_choice", { id: proposal.id, code });
      setProposals((prev) => prev.filter((entry) => entry.id !== proposal.id));
      setBusy(false);
    } catch (error) {
      setErr(String(error));
      setBusy(false);
    }
  }

  async function resolve(item: ReviewPayload, action: "approve" | "retry" | "reject") {
    if (busy) return;
    setBusy(true);
    setErr("");
    try {
      await invoke("resolve_asset_review", {
        id: item.id,
        action,
        prompt: action === "retry" ? prompt : null,
      });
      setQueue((prev) => prev.filter((entry) => entry.id !== item.id));
      if (action === "retry") {
        setGenerating({ kind: item.kind, prompt });
      }
      setBusy(false);
    } catch (error) {
      setErr(String(error));
      setBusy(false);
    }
  }

  if (!queue.length && !generating && !proposals.length) return null;

  return (
    <div className="review-dock" aria-live="polite">
      {generating ? (
        <div className="review-toast is-wait">
          <div className="review-spinner" />
          <div>
            <p className="review-kicker">L’agent attend</p>
            <strong>Génération {generating.kind === "mesh" ? "mesh 3D" : "image"}</strong>
            <small>{generating.prompt}</small>
          </div>
        </div>
      ) : current && collapsed ? (
        <button
          type="button"
          className="review-toast"
          onClick={() => setCollapsed(false)}
        >
          <ToastThumb item={current} />
          <div>
            <p className="review-kicker">
              {current.origin === "inspiration"
                ? "Inspiration"
                : current.origin === "library"
                  ? "Banque"
                  : "L’agent attend"}
              {queue.length > 1 ? ` · 1/${queue.length}` : ""}
            </p>
            <strong>
              {current.origin === "inspiration"
                ? "S’inspirer de cette capture"
                : current.kind === "mesh"
                  ? "Valider ce mesh 3D"
                  : "Valider cette image"}
            </strong>
            <small>{current.code || current.name || current.prompt}</small>
          </div>
        </button>
      ) : current ? (
        <ReviewCard
          key={current.id}
          item={current}
          index={1}
          total={queue.length}
          prompt={prompt}
          busy={busy}
          err={err}
          onPrompt={setPrompt}
          onClose={() => setCollapsed(true)}
          onResolve={(action) => void resolve(current, action)}
        />
      ) : proposals[0] ? (
        <ProposeCard
          key={proposals[0].id}
          proposal={proposals[0]}
          busy={busy}
          err={err}
          onPick={(code) => void resolveChoice(proposals[0], code)}
          onDismiss={() => void resolveChoice(proposals[0], null)}
        />
      ) : null}
    </div>
  );
}

function ProposeThumb({
  item,
  className,
}: {
  item: LibraryProposeItem;
  className?: string;
}) {
  const src = bankImageSrc(item);
  if (src) {
    return <img className={className} src={src} alt="" />;
  }
  return (
    <div className={`${className ?? ""} review-toast-fallback`.trim()}>
      {item.kind === "mesh" ? "3D" : "IMG"}
    </div>
  );
}

function ProposeCard({
  proposal,
  busy,
  err,
  onPick,
  onDismiss,
}: {
  proposal: LibraryProposal;
  busy: boolean;
  err: string;
  onPick: (code: string) => void;
  onDismiss: () => void;
}) {
  const [index, setIndex] = useState(0);
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const item = proposal.items[index] ?? proposal.items[0];

  useEffect(() => {
    if (!open) return;
    function onDoc(event: MouseEvent) {
      if (!menuRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  if (!item) return null;
  const preview = item.previewPath || item.path;
  const imageSrc = bankImageSrc(item);
  const showImage = item.kind !== "mesh" || isImagePath(preview) || Boolean(imageSrc);

  return (
    <div className="review-card propose-card">
      <div className="review-head">
        <div>
          <p className="review-kicker">
            Choisis 1 sur {proposal.items.length}
          </p>
          <h2>Propositions</h2>
          <p className="review-purpose">
            {proposal.purpose?.trim() ? proposal.purpose : proposal.query}
          </p>
          {proposal.purpose?.trim() && proposal.query && proposal.query !== "banque" ? (
            <small className="review-meta">Recherche : {proposal.query}</small>
          ) : null}
        </div>
        <button
          className="review-close"
          type="button"
          disabled={busy}
          onClick={onDismiss}
          aria-label="Aucune"
        >
          ×
        </button>
      </div>
      <div className="propose-select" ref={menuRef}>
        <button
          type="button"
          className="propose-select-btn"
          disabled={busy}
          aria-expanded={open}
          onClick={() => setOpen((value) => !value)}
        >
          <ProposeThumb item={item} className="propose-select-thumb" />
          <span>
            <strong>{item.name}</strong>
            <small>{item.code}</small>
          </span>
          <em>{open ? "▲" : "▼"}</em>
        </button>
        {open ? (
          <ul className="propose-select-list" role="listbox">
            {proposal.items.map((row, i) => (
              <li key={row.code}>
                <button
                  type="button"
                  className={i === index ? "on" : ""}
                  role="option"
                  aria-selected={i === index}
                  onClick={() => {
                    setIndex(i);
                    setOpen(false);
                  }}
                >
                  <ProposeThumb item={row} className="propose-select-thumb" />
                  <span>
                    <strong>{row.name}</strong>
                    <small>{row.code}</small>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
      <div className="review-preview">
        {showImage && imageSrc ? (
          <img src={imageSrc} alt={item.name} />
        ) : (
          <MeshPreview path={item.path} className="asset-hero bank-mesh" />
        )}
      </div>
      <div className="asset-ids">
        <AssetCode code={item.code} />
      </div>
      <p className="review-meta">{item.name}</p>
      {err ? <p className="err">{err}</p> : null}
      <div className="review-actions">
        <button className="btn secondary" type="button" disabled={busy} onClick={onDismiss}>
          Aucune
        </button>
        <button className="btn copper" type="button" disabled={busy} onClick={() => onPick(item.code)}>
          Choisir celle-ci
        </button>
      </div>
    </div>
  );
}

function ToastThumb({ item }: { item: ReviewPayload }) {
  const preview = item.previewPath || (item.kind === "image" ? item.path : "");
  if (preview && isImagePath(preview)) {
    return <img className="review-toast-thumb" src={convertFileSrc(preview)} alt="" />;
  }
  if (item.kind === "mesh" && item.path) {
    return (
      <MeshStill
        id={item.code || item.id}
        path={item.path}
        previewPath={item.previewPath}
        className="review-toast-thumb"
      />
    );
  }
  return <div className="review-toast-thumb review-toast-fallback">{item.kind === "mesh" ? "3D" : "IMG"}</div>;
}

function ReviewCard({
  item,
  index = 1,
  total = 1,
  prompt,
  busy,
  err,
  onPrompt,
  onClose,
  onResolve,
}: {
  item: ReviewPayload;
  index?: number;
  total?: number;
  prompt: string;
  busy: boolean;
  err: string;
  onPrompt: (value: string) => void;
  onClose: () => void;
  onResolve: (action: "approve" | "retry" | "reject") => void;
}) {
  const kindLabel = item.origin === "inspiration" ? "Inspiration" : item.kind === "mesh" ? "Mesh 3D" : "Image";
  const fromBank = item.origin === "library" || item.origin === "inspiration";
  const preview = item.previewPath || item.path;
  const showImage = item.kind === "image" || isImagePath(item.previewPath);

  return (
    <div className="review-card" role="dialog" aria-label={`Valider ${kindLabel}`}>
      <div className="review-head">
        <div>
          <p className="review-kicker">
            {item.origin === "inspiration" ? "Inspiration" : fromBank ? "Asset banque" : "L’agent attend"}
            {total > 1 ? ` · ${index}/${total}` : ""}
          </p>
          <h2>
            {item.origin === "inspiration" ? "S’inspirer de cette capture" : `Valider ce ${kindLabel.toLowerCase()}`}
          </h2>
        </div>
        <button className="review-close" type="button" onClick={onClose} aria-label="Réduire">
          ×
        </button>
      </div>
      <div className="asset-ids">
        {item.code ? <AssetCode code={item.code} /> : null}
        {item.origin === "inspiration" ? null : (
          <AssetCode code={item.robloxAssetId} variant="roblox" />
        )}
      </div>
      <div className="review-preview">
        {showImage && preview ? (
          <img src={convertFileSrc(preview)} alt={item.name || item.prompt} />
        ) : (
          <MeshPreview path={item.path} className="asset-hero bank-mesh" />
        )}
      </div>
      {fromBank ? (
        <p className="review-meta">{item.name || item.prompt}</p>
      ) : (
        <label className="review-prompt">
          Prompt
          <textarea value={prompt} onChange={(event) => onPrompt(event.target.value)} />
        </label>
      )}
      {item.origin === "inspiration" ? (
        <small className="review-meta">Référence visuelle — pas envoyée sur Roblox.</small>
      ) : item.robloxAssetId ? null : (
        <small className="review-meta">Pas encore d’ID Roblox — Lumen publiera après validation.</small>
      )}
      {err ? <p className="err">{err}</p> : null}
      <div className="review-actions">
        {item.canRetry !== false && !fromBank ? (
          <button className="btn secondary" disabled={busy} type="button" onClick={() => onResolve("retry")}>
            Générer un autre
          </button>
        ) : (
          <button className="btn secondary" disabled={busy} type="button" onClick={() => onResolve("reject")}>
            Refuser
          </button>
        )}
        <button className="btn copper" disabled={busy} type="button" onClick={() => onResolve("approve")}>
          Valider et continuer
        </button>
      </div>
    </div>
  );
}
