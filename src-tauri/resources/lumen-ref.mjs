#!/usr/bin/env node
/**
 * Pont Lumen : lit d'autres projets Lumen (références), sans les éditer.
 *
 * Usage:
 *   node tools/lumen-ref.mjs status
 *   node tools/lumen-ref.mjs files "Tycoon Test" src/client
 *   node tools/lumen-ref.mjs cat "Tycoon Test" src/client/ui.ts
 */

const BASE = process.env.LUMEN_ASSET_URL || "http://127.0.0.1:17422";
const FROM = process.env.LUMEN_PROJECT || process.cwd();

function fail(message) {
  console.error(message);
  process.exit(1);
}

function usage() {
  fail(
    "Usage:\n  node tools/lumen-ref.mjs status\n  node tools/lumen-ref.mjs files <projet> [src/client]\n  node tools/lumen-ref.mjs cat <projet> src/client/ui.ts",
  );
}

async function readJson(res) {
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    return { error: text || `HTTP ${res.status}` };
  }
}

async function call(path) {
  let res;
  try {
    res = await fetch(`${BASE}${path}`);
  } catch {
    fail("Lumen n'est pas joignable. Ouvre l'app Lumen, puis réessaie.");
  }
  const json = await readJson(res);
  if (!res.ok || json.error) {
    fail(json.error || JSON.stringify(json));
  }
  return json;
}

function qs(extra = {}) {
  const params = new URLSearchParams({ from: FROM, ...extra });
  return params.toString();
}

async function main() {
  const [cmd, ...rest] = process.argv.slice(2);
  if (!cmd || cmd === "status") {
    const json = await call(`/reference/status?${qs()}`);
    const projects = json.projects || [];
    if (!projects.length) {
      console.log("Aucun projet référence. Studio → S'inspirer de.");
      return;
    }
    for (const item of projects) {
      console.log(`${item.name}\t${item.path}`);
    }
    return;
  }
  if (cmd === "files" || cmd === "ls") {
    const ref = rest[0] || "";
    const prefix = rest.slice(1).join(" ").trim();
    if (!ref) usage();
    const extra = { ref };
    if (prefix) extra.prefix = prefix;
    const json = await call(`/reference/files?${qs(extra)}`);
    const files = json.files || [];
    console.log(`${json.name} — ${files.length} fichier(s)`);
    for (const file of files) console.log(file);
    return;
  }
  if (cmd === "cat" || cmd === "get" || cmd === "read") {
    const ref = rest[0] || "";
    const rel = rest.slice(1).join(" ").trim();
    if (!ref || !rel) usage();
    const json = await call(`/reference/file?${qs({ ref, path: rel })}`);
    console.log(`# ${json.name} :: ${json.path}`);
    console.log(json.text || "");
    return;
  }
  usage();
}

main().catch((err) => fail(String(err.message || err)));
