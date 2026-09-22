import { createReadStream, readFileSync, writeFileSync, existsSync } from "node:fs";
import { readdir, stat } from "node:fs/promises";
import path from "node:path";
import { Readable } from "node:stream";

const root = process.argv[2];
const base = process.argv[3];
const key = readFileSync(new URL("./.upload-key", import.meta.url), "utf8").trim();
const progressPath = new URL("./upload-progress.json", import.meta.url);
const done = new Set(existsSync(progressPath) ? JSON.parse(readFileSync(progressPath, "utf8")) : []);
const concurrency = 12;

function contentType(file) {
  const ext = path.extname(file).toLowerCase();
  if (ext === ".png") return "image/png";
  if (ext === ".jpg" || ext === ".jpeg") return "image/jpeg";
  if (ext === ".webp") return "image/webp";
  if (ext === ".gif") return "image/gif";
  if (ext === ".glb") return "model/gltf-binary";
  if (ext === ".gltf") return "model/gltf+json";
  if (ext === ".json") return "application/json";
  return "application/octet-stream";
}

async function walk(dir, out) {
  for (const name of await readdir(dir)) {
    const full = path.join(dir, name);
    const info = await stat(full);
    if (info.isDirectory()) await walk(full, out);
    else out.push(full);
  }
}

async function putFile(rel, full) {
  const body = Readable.toWeb(createReadStream(full));
  const response = await fetch(`${base}/vibe/${rel.split("/").map(encodeURIComponent).join("/")}`, {
    method: "PUT",
    headers: {
      "X-Lumen-Upload": key,
      "Content-Type": contentType(full),
    },
    body,
    duplex: "half",
  });
  if (!response.ok) {
    throw new Error(`${rel} HTTP ${response.status}`);
  }
}

const files = [];
await walk(root, files);
const isMesh = (file) => [".glb", ".gltf"].includes(path.extname(file).toLowerCase());
files.sort((a, b) => Number(isMesh(a)) - Number(isMesh(b)));
const manifest = [];
let sent = 0;
let cursor = 0;

async function publishManifest() {
  const response = await fetch(`${base}/manifest.json`, {
    method: "PUT",
    headers: {
      "X-Lumen-Upload": key,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(manifest),
  });
  if (!response.ok) throw new Error(`manifest HTTP ${response.status}`);
}

async function worker() {
  while (cursor < files.length) {
    const index = cursor;
    cursor += 1;
    const full = files[index];
    const rel = path.relative(root, full).replaceAll("\\", "/");
    const ext = path.extname(full).toLowerCase();
    const kind = [".glb", ".gltf", ".fbx", ".obj"].includes(ext) ? "mesh" : "image";
    if (![".png", ".jpg", ".jpeg", ".webp", ".gif", ".glb", ".gltf"].includes(ext)) continue;
    const entry = {
      path: rel,
      kind,
      name: path.basename(full, ext).replaceAll("_", " ").replaceAll("-", " "),
    };
    if (done.has(rel)) {
      manifest.push(entry);
      continue;
    }
    for (let attempt = 1; attempt <= 4; attempt += 1) {
      try {
        await putFile(rel, full);
        done.add(rel);
        manifest.push(entry);
        sent += 1;
        if (sent % 200 === 0) {
          writeFileSync(progressPath, JSON.stringify([...done]));
          await publishManifest();
          console.log(`${done.size}/${files.length}`);
        }
        break;
      } catch (error) {
        if (attempt === 4) console.error(String(error));
        else await new Promise((resolve) => setTimeout(resolve, 500 * attempt));
      }
    }
  }
}

await Promise.all(Array.from({ length: concurrency }, () => worker()));
writeFileSync(progressPath, JSON.stringify([...done]));
await publishManifest();
console.log(`terminé ${done.size} fichiers`);
