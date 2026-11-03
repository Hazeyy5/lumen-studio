// État du MCP des éditeurs (UEFN : tools/uefn_listener.py ; Unreal : plugin ModelContextProtocol),
// vérifié toutes les 4 s tant qu'un composant l'écoute.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type UefnEditorStatus = {
  running: boolean;
  port: number | null;
  project: string | null;
  projectPath: string | null;
};

export type UnrealEditorStatus = { running: boolean; server: string | null };

const POLL_MS = 4000;

function poller<T>(command: string, offline: T) {
  let last: T | null = null;
  let looping = false;
  const listeners = new Set<(s: T) => void>();

  async function poll() {
    try {
      last = await invoke<T>(command);
    } catch {
      last = offline;
    }
    listeners.forEach((fn) => fn(last!));
    if (listeners.size) window.setTimeout(poll, POLL_MS);
    else looping = false;
  }

  /** `null` tant que la première vérification n'a pas répondu. */
  return function useStatus(active: boolean): T | null {
    const [status, setStatus] = useState<T | null>(last);
    useEffect(() => {
      if (!active) return;
      listeners.add(setStatus);
      if (!looping) {
        looping = true;
        void poll();
      }
      return () => {
        listeners.delete(setStatus);
      };
    }, [active]);
    return active ? status : null;
  };
}

export const useUefnEditor = poller<UefnEditorStatus>("uefn_editor_status", {
  running: false,
  port: null,
  project: null,
  projectPath: null,
});

export const useUnrealEditor = poller<UnrealEditorStatus>("unreal_editor_status", { running: false, server: null });

const normPath = (p: string) => p.replace(/[\\/]+/g, "/").replace(/\/$/, "").toLowerCase();

/** Sans dossier identifié (projet hors Documents/Fortnite Projects), on ne signale rien. */
export function isSameUefnProject(status: UefnEditorStatus, projectPath: string) {
  return !status.projectPath || normPath(status.projectPath) === normPath(projectPath);
}

export function uefnStatusLabel(status: UefnEditorStatus | null, projectPath: string) {
  if (!status) return { tone: "wait", text: "MCP UEFN : vérification…" };
  if (!status.running) return { tone: "off", text: "MCP UEFN : écouteur non lancé" };
  if (!isSameUefnProject(status, projectPath)) {
    return { tone: "warn", text: `MCP UEFN : UEFN a « ${status.project} » ouvert` };
  }
  return { tone: "on", text: "MCP UEFN : connecté" };
}

export function unrealStatusLabel(status: UnrealEditorStatus | null) {
  if (!status) return { tone: "wait", text: "MCP Unreal : vérification…" };
  if (!status.running) return { tone: "off", text: "MCP Unreal : éditeur fermé" };
  return { tone: "on", text: "MCP Unreal : connecté" };
}
