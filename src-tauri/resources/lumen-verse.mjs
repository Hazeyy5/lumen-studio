#!/usr/bin/env node
/**
 * Pont Lumen ↔ UEFN : compile le Verse dans l'UEFN ouvert, ou pousse les changements dans la session.
 * Passe par le serveur que UEFN ouvre pour l'extension Verse de VS Code (127.0.0.1:1962).
 *
 * Usage:
 *   node tools/lumen-verse.mjs build
 *   node tools/lumen-verse.mjs push
 *   node tools/lumen-verse.mjs status
 */

import net from "node:net";
import { readdirSync } from "node:fs";

const PORT = Number(process.env.LUMEN_VERSE_PORT || 1962);
const HOST = "127.0.0.1";
const BUILD_STATES = ["succès", "avertissements", "erreurs", "compilation en cours", "jamais compilé"];

function fail(message, code = 1) {
  console.error(message);
  process.exit(code);
}

function projectRoot() {
  return (process.env.LUMEN_PROJECT || process.cwd()).replace(/\\/g, "/").replace(/\/$/, "");
}

function projectName() {
  try {
    const file = readdirSync(projectRoot()).find((f) => f.toLowerCase().endsWith(".uefnproject"));
    return file ? file.slice(0, -".uefnproject".length) : "";
  } catch {
    return "";
  }
}

function connect() {
  return new Promise((resolve) => {
    const socket = net.createConnection(PORT, HOST);
    const timer = setTimeout(() => {
      socket.destroy();
      resolve(null);
    }, 4000);
    socket.on("connect", () => {
      clearTimeout(timer);
      resolve(socket);
    });
    socket.on("error", () => {
      clearTimeout(timer);
      resolve(null);
    });
  });
}

function session(socket) {
  let raw = Buffer.alloc(0);
  let seq = 1;
  const pending = new Map();
  const notes = [];
  socket.on("data", (data) => {
    raw = Buffer.concat([raw, data]);
    for (;;) {
      const head = raw.indexOf("\r\n\r\n");
      if (head < 0) return;
      const match = /Content-Length: *(\d+)/i.exec(raw.toString("utf8", 0, head));
      if (!match) {
        raw = raw.slice(head + 4);
        continue;
      }
      const length = Number(match[1]);
      if (raw.length < head + 4 + length) return;
      const body = raw.toString("utf8", head + 4, head + 4 + length);
      raw = raw.slice(head + 4 + length);
      let message;
      try {
        message = JSON.parse(body);
      } catch {
        continue;
      }
      if (message.type === 2 && pending.has(message.seq)) {
        pending.get(message.seq)(message);
        pending.delete(message.seq);
      } else if (message.type === 0) {
        notes.push(message);
      }
    }
  });
  return {
    notes,
    request(command, params, timeoutMs) {
      return new Promise((resolve) => {
        const id = seq++;
        const timer = setTimeout(() => {
          pending.delete(id);
          resolve({ timeout: true });
        }, timeoutMs);
        pending.set(id, (message) => {
          clearTimeout(timer);
          resolve(message);
        });
        const json = JSON.stringify({ seq: id, type: 1, command, params });
        socket.write(`Content-Length: ${Buffer.byteLength(json, "utf8")}\r\n\r\n${json}`, "utf8");
      });
    },
  };
}

async function open() {
  const socket = await connect();
  if (!socket) {
    fail(
      `UEFN n'est pas joignable (${HOST}:${PORT}). Demande à l'utilisateur d'ouvrir le projet ${projectName() || "UEFN"} dans UEFN, puis relance. Ne considère pas le code comme compilé.`,
      2,
    );
  }
  return { socket, client: session(socket) };
}

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function lastBuildState(notes) {
  const states = notes.filter((n) => n.command === "updateBuildState").map((n) => n.params);
  return states.length ? states[states.length - 1] : null;
}

// Les erreurs citent des chemins absolus : on les raccourcit, et on repère un autre projet ouvert dans UEFN.
function shorten(log) {
  const root = projectRoot();
  const lower = root.toLowerCase() + "/";
  const others = new Set();
  const lines = String(log || "")
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter(Boolean)
    .map((line) => {
      const path = /^([A-Za-z]:[\\/][^(]+?\.verse)\(/.exec(line);
      if (!path) return line;
      const file = path[1].replace(/\\/g, "/");
      if (file.toLowerCase().startsWith(lower)) return file.slice(lower.length) + line.slice(path[1].length);
      const project = /Fortnite Projects\/([^/]+)\//i.exec(file);
      if (project) others.add(project[1]);
      return line;
    });
  return { lines, others: [...others] };
}

async function build() {
  const { socket, client } = await open();
  await wait(300);
  console.error("Compilation Verse dans UEFN…");
  const response = await client.request("compileProject", {}, 10 * 60 * 1000);
  socket.end();
  if (response.timeout) fail("UEFN n'a pas répondu en 10 minutes. Demande à l'utilisateur de regarder UEFN.");
  if (response.error !== undefined) fail(`UEFN a refusé la compilation : ${response.error}`);
  const result = response.result || {};
  const errors = Number(result.numErrors || 0);
  const warnings = Number(result.numWarnings || 0);
  const { lines, others } = shorten(result.message);
  const name = projectName();
  if (others.length && name && !others.some((o) => o.toLowerCase() === name.toLowerCase())) {
    fail(
      `UEFN a compilé un autre projet (${others.join(", ")}), pas ${name}. Demande à l'utilisateur d'ouvrir ${name} dans UEFN, puis relance.`,
      3,
    );
  }
  console.log(`Verse : ${errors} erreur(s), ${warnings} avertissement(s).`);
  for (const line of lines) console.log(line);
  if (errors) {
    console.log("Corrige ces erreurs (fichier(ligne,colonne)), puis relance build jusqu'à 0 erreur.");
    process.exit(1);
  }
  console.log(
    "Compilé. Si une session de test tourne, `node tools/lumen-verse.mjs push` y envoie le nouveau Verse. Sinon, l'utilisateur lance la session depuis UEFN.",
  );
}

async function push() {
  const { socket, client } = await open();
  await wait(300);
  console.error("Envoi des changements Verse dans la session…");
  const response = await client.request("pushChanges", true, 5 * 60 * 1000);
  socket.end();
  if (response.timeout) fail("UEFN n'a pas répondu en 5 minutes.");
  if (response.error !== undefined) {
    fail(`Push impossible : ${response.error}. Il faut une session de test lancée depuis UEFN, et un build sans erreur.`);
  }
  console.log(String(response.result || "Changements envoyés."));
}

async function status() {
  const { socket, client } = await open();
  await wait(800);
  socket.end();
  const state = lastBuildState(client.notes);
  console.log(`UEFN joignable sur ${HOST}:${PORT}.`);
  if (state !== null) console.log(`Dernier build : ${BUILD_STATES[state] ?? state}.`);
  console.log("UEFN compile le projet ouvert dans l'éditeur : vérifie que c'est bien celui-ci.");
}

const cmd = process.argv[2];
if (cmd === "build" || cmd === "compile") await build();
else if (cmd === "push") await push();
else if (cmd === "status") await status();
else fail("Usage:\n  node tools/lumen-verse.mjs build\n  node tools/lumen-verse.mjs push\n  node tools/lumen-verse.mjs status");
