import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { MeshPreview } from "./MeshPreview";
import type { BankItem } from "./types";

export type LightboxAsset =
  | { kind: "image"; title: string; src: string; code?: string; robloxAssetId?: string }
  | { kind: "mesh"; title: string; path: string; code?: string; robloxAssetId?: string };

export function robloxThumbUrl(id?: string | null) {
  const n = (id || "").replace(/\D/g, "");
  if (!n) return "";
  return `https://www.roblox.com/asset-thumbnail/image?assetId=${n}&width=420&height=420&format=png`;
}

export function bankImageSrc(item: {
  path?: string;
  previewPath?: string | null;
  robloxAssetId?: string | null;
}) {
  const file = item.previewPath || item.path || "";
  if (/\.(png|jpe?g|webp|gif|bmp)$/i.test(file) || /studio-thumbs/i.test(file)) {
    return convertFileSrc(file);
  }
  return robloxThumbUrl(item.robloxAssetId);
}

export function lightboxFromBank(item: BankItem): LightboxAsset | null {
  if (item.kind === "image") {
    const src = bankImageSrc(item);
    if (!src) return null;
    return {
      kind: "image",
      title: item.name,
      src,
      code: item.code,
      robloxAssetId: item.robloxAssetId ?? undefined,
    };
  }
  if (item.kind === "mesh") {
    return {
      kind: "mesh",
      title: item.name,
      path: item.path,
      code: item.code,
      robloxAssetId: item.robloxAssetId ?? undefined,
    };
  }
  return null;
}

export function AssetCode({
  code,
  variant = "lumen",
}: {
  code?: string | null;
  variant?: "lumen" | "roblox";
}) {
  const [copied, setCopied] = useState(false);
  if (!code) return null;
  const label = variant === "roblox"
    ? /^rbxasset/i.test(code)
      ? code
      : `rbxassetid://${code}`
    : code;
  return (
    <button
      type="button"
      className={`asset-code${variant === "roblox" ? " roblox" : ""}`}
      title={variant === "roblox" ? "Copier rbxassetid" : "Copier l’ID Lumen"}
      onClick={(event) => {
        event.stopPropagation();
        void navigator.clipboard.writeText(label).then(() => {
          setCopied(true);
          window.setTimeout(() => setCopied(false), 1200);
        });
      }}
    >
      {copied ? "Copié" : label}
    </button>
  );
}

export function AssetLightbox({
  asset,
  onClose,
}: {
  asset: LightboxAsset;
  onClose: () => void;
}) {
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="lightbox" onClick={onClose} role="presentation">
      <div
        className="lightbox-panel"
        role="dialog"
        aria-modal="true"
        aria-label={asset.title}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="lightbox-head">
          <div className="lightbox-head-meta">
            <h2>{asset.title}</h2>
            <div className="asset-ids">
              <AssetCode code={asset.code} />
              <AssetCode code={asset.robloxAssetId} variant="roblox" />
            </div>
          </div>
          <button className="btn secondary" type="button" onClick={onClose}>
            Fermer
          </button>
        </div>
        <div className="lightbox-body">
          {asset.kind === "image" ? (
            <img src={asset.src} alt={asset.title} />
          ) : (
            <MeshPreview path={asset.path} className="asset-hero bank-mesh" />
          )}
        </div>
        {asset.kind === "mesh" ? (
          <p className="lightbox-hint">Glisse pour tourner le modèle · molette pour zoomer</p>
        ) : null}
      </div>
    </div>
  );
}
