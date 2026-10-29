// État de l'écouteur MCP dans UEFN (tools/uefn_listener.py), vérifié toutes les 4 s tant qu'un composant l'écoute.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type UefnEditorStatus = { running: boolean; port: number | null; project: string | null };

const POLL_MS = 4000;
let last: UefnEditorStatus | null = null;
const listeners = new Set<(s: UefnEditorStatus) => void>();
let looping = false;

async function poll() {
  try {
    last = await invoke<UefnEditorStatus>("uefn_editor_status");
  } catch {
    last = { running: false, port: null, project: null };
  }
  listeners.forEach((fn) => fn(last!));
  if (listeners.size) window.setTimeout(poll, POLL_MS);
  else looping = false;
}

/** `null` tant que la première vérification n'a pas répondu. */
export function useUefnEditor(active: boolean): UefnEditorStatus | null {
  const [status, setStatus] = useState<UefnEditorStatus | null>(last);
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
}

/** Le dossier du projet UEFN porte le nom de la racine du contenu (/MonIle/). */
export function isSameUefnProject(status: UefnEditorStatus, projectPath: string) {
  const folder = projectPath.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? "";
  return !status.project || status.project.toLowerCase() === folder.toLowerCase();
}

export function uefnStatusLabel(status: UefnEditorStatus | null, projectPath: string) {
  if (!status) return { tone: "wait", text: "MCP UEFN : vérification…" };
  if (!status.running) return { tone: "off", text: "MCP UEFN : écouteur non lancé" };
  if (!isSameUefnProject(status, projectPath)) {
    return { tone: "warn", text: `MCP UEFN : UEFN a « ${status.project} » ouvert` };
  }
  return { tone: "on", text: "MCP UEFN : connecté" };
}
