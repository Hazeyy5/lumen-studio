const CORS = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "GET, HEAD, OPTIONS",
  "access-control-allow-headers": "range",
  "access-control-expose-headers": "etag, content-length, content-type, content-range, accept-ranges",
};

const GALLERY_COOKIE = "lumen_galerie";
const GROUPS_KEY = "private/gallery-groups.json";

function withCors(response) {
  const headers = new Headers(response.headers);
  for (const [name, value] of Object.entries(CORS)) headers.set(name, value);
  return new Response(response.body, { status: response.status, headers });
}

function parseCookies(header) {
  const out = new Map();
  if (!header) return out;
  for (const part of header.split(";")) {
    const index = part.indexOf("=");
    if (index === -1) continue;
    out.set(part.slice(0, index).trim(), decodeURIComponent(part.slice(index + 1).trim()));
  }
  return out;
}

async function passwordDigest(password) {
  const bytes = new TextEncoder().encode(`lumen-galerie\0${password}`);
  return new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
}

async function passwordMatches(input, expected) {
  const left = await passwordDigest(input);
  const right = await passwordDigest(expected);
  return crypto.subtle.timingSafeEqual(left, right);
}

async function galleryAuthorized(request, env) {
  if (!env.GALLERY_PASSWORD) return false;
  const token = parseCookies(request.headers.get("cookie")).get(GALLERY_COOKIE) || "";
  const expected = [...(await passwordDigest(env.GALLERY_PASSWORD))]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
  const left = new TextEncoder().encode(token);
  const right = new TextEncoder().encode(expected);
  if (left.byteLength !== right.byteLength) return false;
  return crypto.subtle.timingSafeEqual(left, right);
}

function galleryCookie(token) {
  const value = token
    ? `${GALLERY_COOKIE}=${token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=2592000`
    : `${GALLERY_COOKIE}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0`;
  return value;
}

function htmlResponse(body, status, extraHeaders) {
  const headers = new Headers({
    "content-type": "text/html; charset=utf-8",
    "cache-control": "no-store",
    "x-robots-tag": "noindex, nofollow",
    "referrer-policy": "no-referrer",
  });
  if (extraHeaders) {
    for (const [name, value] of extraHeaders) headers.set(name, value);
  }
  return new Response(body, { status, headers });
}

function loginPage(error) {
  const message = error ? "<p class=\"error\">Mot de passe incorrect.</p>" : "";
  return `<!doctype html>
<html lang="fr">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="robots" content="noindex, nofollow">
  <title>Galerie Lumen</title>
  <style>
    :root { color-scheme: light; --paper: #f3eee4; --ink: #1c1712; --muted: #6b6156; --line: #d7cbb8; --copper: #b85c38; --white: #fffaf3; }
    * { box-sizing: border-box; }
    body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: var(--paper); color: var(--ink); font: 16px/1.4 "Segoe UI", sans-serif; }
    form { width: min(420px, calc(100% - 32px)); background: var(--white); border: 1px solid var(--line); border-radius: 16px; padding: 28px; }
    h1 { margin: 0 0 8px; font-size: 28px; }
    p { margin: 0 0 18px; color: var(--muted); }
    .error { color: var(--copper); margin-bottom: 12px; }
    label { display: block; font-size: 14px; margin-bottom: 6px; }
    input { width: 100%; border: 1px solid var(--line); background: var(--paper); color: var(--ink); border-radius: 10px; padding: 12px 14px; font: inherit; }
    button { margin-top: 14px; width: 100%; border: 0; border-radius: 10px; padding: 12px 14px; background: var(--copper); color: var(--white); font: inherit; cursor: pointer; }
  </style>
</head>
<body>
  <form method="post" action="/galerie">
    <h1>Galerie</h1>
    <p>Catalogue privé des icônes Lumen.</p>
    ${message}
    <label for="password">Mot de passe</label>
    <input id="password" name="password" type="password" autocomplete="current-password" required autofocus>
    <button type="submit">Entrer</button>
  </form>
</body>
</html>`;
}

function galleryPage() {
  return `<!doctype html>
<html lang="fr">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="robots" content="noindex, nofollow">
  <title>Icônes Lumen</title>
  <style>
    :root { color-scheme: light; --paper: #f3eee4; --paper-2: #efe6d6; --ink: #1c1712; --muted: #6b6156; --line: #d7cbb8; --copper: #b85c38; --white: #fffaf3; }
    * { box-sizing: border-box; }
    body { margin: 0; background: var(--paper); color: var(--ink); font: 15px/1.4 "Segoe UI", sans-serif; }
    header { position: sticky; top: 0; z-index: 2; display: flex; flex-wrap: wrap; gap: 12px; align-items: center; padding: 16px 20px; background: color-mix(in srgb, var(--paper) 92%, transparent); backdrop-filter: blur(10px); border-bottom: 1px solid var(--line); }
    h1 { margin: 0; font-size: 22px; }
    .count { color: var(--muted); }
    input[type="search"] { flex: 1 1 220px; min-width: 180px; border: 1px solid var(--line); background: var(--white); color: var(--ink); border-radius: 999px; padding: 10px 14px; font: inherit; }
    nav { display: flex; gap: 6px; }
    nav button, .groups button, .more, .quit { border: 1px solid var(--line); background: var(--white); color: var(--ink); border-radius: 999px; padding: 8px 12px; font: inherit; cursor: pointer; text-decoration: none; }
    nav button[aria-pressed="true"], .groups button[aria-pressed="true"] { background: var(--copper); color: var(--white); border-color: var(--copper); }
    .groups { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; width: 100%; }
    .groups form { display: flex; gap: 6px; }
    .groups input { width: 168px; border: 1px solid var(--line); background: var(--white); color: var(--ink); border-radius: 999px; padding: 8px 12px; font: inherit; }
    .tag { color: var(--copper); font-style: normal; font-size: 12px; }
    .card-groups { display: flex; flex-wrap: wrap; gap: 4px; }
    .card-groups button { border: 1px solid var(--line); background: var(--paper); color: var(--ink); border-radius: 999px; padding: 3px 8px; font: 11px/1.2 "Segoe UI", sans-serif; cursor: pointer; }
    .card-groups button[aria-pressed="true"] { background: var(--copper); color: var(--white); border-color: var(--copper); }
    .membership { display: flex; flex-wrap: wrap; gap: 8px 14px; margin-top: 12px; }
    .membership label { display: flex; align-items: center; gap: 6px; }
    #drop-group[hidden], #zip-group[hidden] { display: none; }
    .card .dl { align-self: flex-start; color: var(--copper); font-size: 12px; text-decoration: none; }
    .card .dl:hover { text-decoration: underline; }
    dialog menu a { border: 1px solid var(--line); background: var(--white); color: var(--ink); border-radius: 999px; padding: 8px 12px; text-decoration: none; }
    main { padding: 18px 20px 48px; }
    .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(148px, 1fr)); gap: 12px; }
    .card { display: flex; flex-direction: column; gap: 8px; padding: 8px; border: 1px solid var(--line); border-radius: 14px; background: var(--white); color: inherit; text-align: left; }
    .card .open { display: flex; flex-direction: column; gap: 8px; border: 0; padding: 0; background: transparent; color: inherit; text-align: left; cursor: pointer; font: inherit; }
    .card img, .mesh { width: 100%; aspect-ratio: 1; object-fit: contain; border-radius: 10px; background-color: #fffaf3; background-image: linear-gradient(45deg, #efe6d6 25%, transparent 25%), linear-gradient(-45deg, #efe6d6 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #efe6d6 75%), linear-gradient(-45deg, transparent 75%, #efe6d6 75%); background-size: 18px 18px; background-position: 0 0, 0 9px, 9px -9px, -9px 0; }
    .mesh { position: relative; display: grid; place-items: center; overflow: hidden; color: var(--muted); font-weight: 650; }
    .mesh img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; opacity: 0; background: transparent; }
    .mesh.ready img { opacity: 1; }
    .mesh.ready > span { display: none; }
    #thumb-maker { position: fixed; width: 256px; height: 256px; left: 0; bottom: 0; opacity: 0.02; pointer-events: none; }
    .card span { font-size: 13px; line-height: 1.3; }
    .more { display: block; margin: 18px auto 0; }
    .more[hidden] { display: none; }
    dialog { width: min(860px, calc(100% - 24px)); border: 1px solid var(--line); border-radius: 16px; padding: 16px; background: var(--white); color: var(--ink); }
    dialog::backdrop { background: rgba(28, 23, 18, 0.45); }
    dialog img, dialog model-viewer { display: block; width: 100%; height: min(68vh, 640px); object-fit: contain; background: var(--paper); border-radius: 12px; }
    dialog p { margin: 12px 0 0; }
    dialog menu { display: flex; justify-content: flex-end; gap: 8px; margin: 12px 0 0; padding: 0; }
    .status { color: var(--muted); padding: 24px 0; }
    #viewer-status[hidden] { display: none; }
  </style>
</head>
<body>
  <header>
    <h1>Icônes</h1>
    <span class="count" id="count">Chargement…</span>
    <input id="query" type="search" placeholder="Rechercher" autocomplete="off">
    <nav>
      <button type="button" data-mode="image" aria-pressed="true">Icônes</button>
      <button type="button" data-mode="mesh" aria-pressed="false">3D</button>
      <button type="button" data-mode="all" aria-pressed="false">Tout</button>
    </nav>
    <a class="quit" href="/galerie/sortir">Quitter</a>
    <div class="groups" id="groups"></div>
    <form class="groups" id="new-group">
      <input name="name" maxlength="40" placeholder="Nouveau groupe, ex. UI" autocomplete="off" required>
      <button type="submit">Créer</button>
      <button type="button" id="zip-group" hidden>Télécharger le groupe</button>
      <button type="button" id="drop-group" hidden>Supprimer ce groupe</button>
    </form>
  </header>
  <main>
    <p class="status" id="status"></p>
    <div class="grid" id="grid"></div>
    <button class="more" id="more" type="button" hidden>Afficher la suite</button>
  </main>
  <dialog id="viewer">
    <p class="status" id="viewer-status" hidden>Chargement du modèle…</p>
    <div id="stage"></div>
    <p id="caption"></p>
    <div class="membership" id="membership"></div>
    <menu>
      <a id="download" href="#" download>Télécharger</a>
      <button type="button" id="close">Fermer</button>
    </menu>
  </dialog>
  <model-viewer id="thumb-maker" camera-orbit="30deg 70deg auto" interaction-prompt="none" shadow-intensity="0"></model-viewer>
  <script type="module" src="https://cdn.jsdelivr.net/npm/@google/model-viewer@3.5.0/dist/model-viewer.min.js"></script>
  <script>
    const PAGE = 72;
    let all = [];
    let groups = [];
    let shown = PAGE;
    let mode = "image";
    let query = "";
    let activeGroup = "";
    let currentItem = null;
    const grid = document.getElementById("grid");
    const count = document.getElementById("count");
    const status = document.getElementById("status");
    const more = document.getElementById("more");
    const viewer = document.getElementById("viewer");
    const stage = document.getElementById("stage");
    const caption = document.getElementById("caption");

    function assetUrl(rel) {
      return "/vibe/" + String(rel).split("/").map(encodeURIComponent).join("/");
    }

    function fileName(rel) {
      const parts = String(rel).split("/");
      return parts[parts.length - 1] || "asset";
    }

    function downloadLink(item) {
      const link = document.createElement("a");
      link.className = "dl";
      link.href = assetUrl(item.path);
      link.download = fileName(item.path);
      link.textContent = "Télécharger";
      return link;
    }

    function loadZipper() {
      if (window.fflate) return Promise.resolve(window.fflate);
      return new Promise(function (resolve, reject) {
        const script = document.createElement("script");
        script.src = "https://cdn.jsdelivr.net/npm/fflate@0.8.2/umd/index.js";
        script.onload = function () { window.fflate ? resolve(window.fflate) : reject(new Error("zip")); };
        script.onerror = function () { reject(new Error("zip")); };
        document.head.append(script);
      });
    }

    let zipping = false;
    const ZIP_LABELS = { image: "les icônes du groupe", mesh: "la 3D du groupe", all: "le groupe" };
    const ZIP_SUFFIX = { image: " - Icônes", mesh: " - 3D", all: "" };

    function groupPaths(group) {
      if (mode === "all") return group.paths.slice();
      const kinds = new Map();
      for (const item of all) kinds.set(item.path, item.kind);
      return group.paths.filter(function (path) { return kinds.get(path) === mode; });
    }

    function renderZip() {
      const button = document.getElementById("zip-group");
      const selected = activeGroup ? groupById(activeGroup) : null;
      const count = selected ? groupPaths(selected).length : 0;
      button.hidden = !count;
      if (count && !zipping) button.textContent = "Télécharger " + ZIP_LABELS[mode] + " · " + count;
    }

    async function downloadGroup(group) {
      if (zipping || !group) return;
      const paths = groupPaths(group);
      if (!paths.length) return;
      const suffix = ZIP_SUFFIX[mode];
      zipping = true;
      const button = document.getElementById("zip-group");
      button.disabled = true;
      const files = {};
      const missing = [];
      let done = 0;
      function progress() {
        button.textContent = "Préparation… " + done + "/" + paths.length;
      }
      progress();
      try {
        const fflate = await loadZipper();
        let next = 0;
        async function worker() {
          while (next < paths.length) {
            const path = paths[next++];
            try {
              const response = await fetch(assetUrl(path));
              if (!response.ok) throw new Error(String(response.status));
              const bytes = new Uint8Array(await response.arrayBuffer());
              const packed = /\\.(png|jpe?g|webp|gif)$/i.test(path);
              files[path] = [bytes, { level: packed ? 0 : 6 }];
            } catch (error) {
              missing.push(path);
            }
            done += 1;
            progress();
          }
        }
        await Promise.all([worker(), worker(), worker(), worker(), worker(), worker()]);
        if (missing.length) {
          files["_manquants.txt"] = [fflate.strToU8(missing.join("\\n") + "\\n"), { level: 6 }];
        }
        button.textContent = "Compression…";
        const zipped = await new Promise(function (resolve, reject) {
          fflate.zip(files, function (error, data) { error ? reject(error) : resolve(data); });
        });
        const url = URL.createObjectURL(new Blob([zipped], { type: "application/zip" }));
        const link = document.createElement("a");
        link.href = url;
        link.download = (group.name.replace(/[\\\\/:*?"<>|]+/g, "-").trim() || "groupe") + suffix + ".zip";
        document.body.append(link);
        link.click();
        link.remove();
        setTimeout(function () { URL.revokeObjectURL(url); }, 60000);
        if (missing.length) {
          status.hidden = false;
          status.textContent = missing.length + " fichier(s) introuvable(s), listé(s) dans _manquants.txt.";
        }
      } catch (error) {
        status.hidden = false;
        status.textContent = "Le téléchargement du groupe a échoué.";
      } finally {
        zipping = false;
        button.disabled = false;
        renderGroups();
      }
    }

    const thumbCache = new Map();
    const thumbFailed = new Set();
    const thumbQueued = new Set();
    const thumbQueue = [];
    let thumbBusy = false;

    function thumbUrl(rel) {
      return "/thumbs/" + String(rel).split("/").map(encodeURIComponent).join("/") + ".webp";
    }

    function thumbsFor(path) {
      return Array.from(document.querySelectorAll("img[data-thumb]")).filter(function (img) {
        return img.getAttribute("data-thumb") === path;
      });
    }

    function showThumb(path, url) {
      thumbCache.set(path, url);
      thumbsFor(path).forEach(function (img) {
        img.src = url;
        if (img.parentElement) img.parentElement.classList.add("ready");
      });
    }

    function thumbNote() {
      const preparing = thumbBusy || thumbQueue.length > 0;
      if (!preparing && status.textContent.indexOf("Préparation") === 0) {
        status.hidden = true;
        status.textContent = "";
        return;
      }
      if (preparing && (status.hidden || status.textContent.indexOf("Préparation") === 0)) {
        status.hidden = false;
        status.textContent = "Préparation des aperçus 3D…";
      }
    }

    function enqueueThumb(path) {
      if (!path || thumbCache.has(path) || thumbFailed.has(path) || thumbQueued.has(path)) return;
      thumbQueued.add(path);
      thumbQueue.push(path);
      pumpThumbs();
    }

    function waitModel(maker) {
      return new Promise(function (resolve, reject) {
        const timer = setTimeout(function () { cleanup(); reject(new Error("timeout")); }, 20000);
        function onLoad() { cleanup(); resolve(); }
        function onError() { cleanup(); reject(new Error("model")); }
        function cleanup() {
          clearTimeout(timer);
          maker.removeEventListener("load", onLoad);
          maker.removeEventListener("error", onError);
        }
        maker.addEventListener("load", onLoad);
        maker.addEventListener("error", onError);
      });
    }

    async function renderThumb(path) {
      const maker = document.getElementById("thumb-maker");
      await ensureViewer();
      const loaded = waitModel(maker);
      maker.setAttribute("src", assetUrl(path));
      await loaded;
      await new Promise(function (resolve) { setTimeout(resolve, 200); });
      const blob = await maker.toBlob({ mimeType: "image/webp", qualityArgument: 0.82 });
      showThumb(path, URL.createObjectURL(blob));
      await fetch("/galerie/apercu?path=" + encodeURIComponent(path), {
        method: "POST",
        headers: { "content-type": blob.type || "image/webp" },
        body: blob,
      });
    }

    async function pumpThumbs() {
      if (thumbBusy) return;
      thumbBusy = true;
      thumbNote();
      while (thumbQueue.length) {
        const path = thumbQueue.shift();
        thumbQueued.delete(path);
        if (thumbCache.has(path)) continue;
        try {
          await renderThumb(path);
        } catch (error) {
          thumbFailed.add(path);
        }
        thumbNote();
      }
      thumbBusy = false;
      thumbNote();
    }

    function groupById(id) {
      for (const group of groups) if (group.id === id) return group;
      return null;
    }

    function itemGroups(path) {
      return groups.filter(function (group) { return group.paths.indexOf(path) !== -1; });
    }

    function inScope(item, kind) {
      const needle = query.trim().toLowerCase();
      const selected = activeGroup ? groupById(activeGroup) : null;
      if (kind && item.kind !== kind) return false;
      if (selected && selected.paths.indexOf(item.path) === -1) return false;
      if (!needle) return true;
      return (item.name + " " + item.path).toLowerCase().indexOf(needle) !== -1;
    }

    function filtered() {
      const kind = mode === "all" ? "" : mode;
      return all.filter(function (item) { return inScope(item, kind); });
    }

    function renderFilters() {
      const labels = { image: "Icônes", mesh: "3D", all: "Tout" };
      document.querySelectorAll("nav button").forEach(function (button) {
        const value = button.getAttribute("data-mode");
        let count = 0;
        for (const item of all) if (inScope(item, value === "all" ? "" : value)) count += 1;
        button.textContent = labels[value] + " · " + count.toLocaleString("fr-FR");
      });
    }

    async function saveGroups(body, options) {
      const response = await fetch("/galerie/groupes", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || "Enregistrement impossible");
      groups = data.groups || [];
      if (activeGroup && !groupById(activeGroup)) activeGroup = "";
      renderGroups();
      const soft = options && options.soft;
      const leaveGroup = soft && activeGroup && body.action === "set" && body.id === activeGroup && !body.member;
      if (!soft || leaveGroup) render();
      else renderFilters();
      if (currentItem && viewer.open) renderMembership(currentItem);
    }

    function renderGroups() {
      const bar = document.getElementById("groups");
      bar.replaceChildren();
      const allButton = document.createElement("button");
      allButton.type = "button";
      allButton.textContent = "Tous";
      allButton.setAttribute("aria-pressed", activeGroup ? "false" : "true");
      allButton.addEventListener("click", function () {
        activeGroup = "";
        shown = PAGE;
        renderGroups();
        render();
      });
      bar.append(allButton);
      for (const group of groups) {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = group.name + " · " + group.paths.length;
        button.setAttribute("aria-pressed", group.id === activeGroup ? "true" : "false");
        button.addEventListener("click", function () {
          activeGroup = group.id;
          shown = PAGE;
          renderGroups();
          render();
        });
        bar.append(button);
      }
      const drop = document.getElementById("drop-group");
      drop.hidden = !activeGroup;
      renderZip();
    }

    function renderMembership(item) {
      const box = document.getElementById("membership");
      box.replaceChildren();
      if (!groups.length) {
        box.textContent = "Crée un groupe pour y ranger cette icône.";
        return;
      }
      for (const group of groups) {
        const label = document.createElement("label");
        const input = document.createElement("input");
        input.type = "checkbox";
        input.checked = group.paths.indexOf(item.path) !== -1;
        input.addEventListener("change", function () {
          saveGroups({ action: "set", id: group.id, path: item.path, member: input.checked }).catch(function (error) {
            input.checked = !input.checked;
            box.insertAdjacentText("afterbegin", error.message + " ");
          });
        });
        label.append(input, document.createTextNode(" " + group.name));
        box.append(label);
      }
    }

    function render() {
      const items = filtered();
      const slice = items.slice(0, shown);
      grid.replaceChildren();
      for (const item of slice) {
        const card = document.createElement("article");
        card.className = "card";
        const open = document.createElement("button");
        open.type = "button";
        open.className = "open";
        if (item.kind === "image") {
          const img = document.createElement("img");
          img.alt = item.name || "";
          img.loading = "lazy";
          img.decoding = "async";
          img.src = assetUrl(item.path);
          open.append(img);
        } else {
          const frame = document.createElement("div");
          frame.className = "mesh";
          const preview = document.createElement("img");
          preview.alt = "";
          preview.setAttribute("data-thumb", item.path);
          const badge = document.createElement("span");
          badge.textContent = "3D";
          frame.append(preview, badge);
          if (thumbCache.has(item.path)) {
            preview.src = thumbCache.get(item.path);
            frame.classList.add("ready");
          } else {
            preview.addEventListener("load", function () {
              frame.classList.add("ready");
              thumbCache.set(item.path, preview.src);
            });
            preview.addEventListener("error", function () { enqueueThumb(item.path); });
            preview.src = thumbUrl(item.path);
          }
          open.append(frame);
        }
        const name = document.createElement("span");
        name.textContent = item.name || item.path;
        open.append(name);
        open.addEventListener("click", function () { openItem(item); });
        card.append(open);
        card.append(downloadLink(item));
        if (groups.length) {
          const bar = document.createElement("div");
          bar.className = "card-groups";
          for (const group of groups) {
            const chip = document.createElement("button");
            chip.type = "button";
            chip.textContent = group.name;
            const member = group.paths.indexOf(item.path) !== -1;
            chip.setAttribute("aria-pressed", member ? "true" : "false");
            chip.title = member ? "Retirer de " + group.name : "Ajouter à " + group.name;
            chip.addEventListener("click", function (event) {
              event.preventDefault();
              event.stopPropagation();
              const next = chip.getAttribute("aria-pressed") !== "true";
              chip.setAttribute("aria-pressed", next ? "true" : "false");
              chip.title = next ? "Retirer de " + group.name : "Ajouter à " + group.name;
              saveGroups({ action: "set", id: group.id, path: item.path, member: next }, { soft: true }).catch(function (error) {
                chip.setAttribute("aria-pressed", next ? "false" : "true");
                chip.title = next ? "Ajouter à " + group.name : "Retirer de " + group.name;
                status.hidden = false;
                status.textContent = error.message;
              });
            });
            bar.append(chip);
          }
          card.append(bar);
        }
        grid.append(card);
      }
      const label = items.length.toLocaleString("fr-FR");
      count.textContent = label + (items.length > 1 ? " éléments" : " élément");
      more.hidden = shown >= items.length;
      renderFilters();
      renderZip();
      if (!items.length) {
        status.hidden = false;
        if (activeGroup && mode === "image") status.textContent = "Aucune icône dans ce groupe.";
        else if (activeGroup && mode === "mesh") status.textContent = "Aucun modèle 3D dans ce groupe.";
        else if (activeGroup) status.textContent = "Ce groupe est vide.";
        else status.textContent = "Aucun résultat.";
      } else if (all.length && status.textContent.indexOf("Préparation") !== 0) {
        status.hidden = true;
      }
    }

    function ensureViewer() {
      if (customElements.get("model-viewer")) return Promise.resolve();
      return Promise.race([
        customElements.whenDefined("model-viewer"),
        new Promise(function (_, reject) {
          setTimeout(function () { reject(new Error("viewer")); }, 12000);
        }),
      ]);
    }

    async function openItem(item) {
      currentItem = item;
      stage.replaceChildren();
      const viewerStatus = document.getElementById("viewer-status");
      viewerStatus.hidden = item.kind === "image";
      viewerStatus.textContent = "Chargement du modèle…";
      caption.textContent = item.name || item.path;
      const download = document.getElementById("download");
      download.href = assetUrl(item.path);
      download.download = fileName(item.path);
      renderMembership(item);
      if (item.kind === "image") {
        const img = document.createElement("img");
        img.alt = item.name || "";
        img.src = assetUrl(item.path);
        stage.append(img);
      } else {
        try {
          await ensureViewer();
          const model = document.createElement("model-viewer");
          model.setAttribute("src", assetUrl(item.path));
          model.setAttribute("camera-controls", "");
          model.setAttribute("auto-rotate", "");
          model.setAttribute("shadow-intensity", "1");
          model.setAttribute("interaction-prompt", "none");
          model.addEventListener("load", function () { viewerStatus.hidden = true; });
          model.addEventListener("error", function () {
            viewerStatus.hidden = false;
            viewerStatus.textContent = "Aperçu 3D impossible.";
          });
          stage.append(model);
        } catch (error) {
          viewerStatus.hidden = false;
          viewerStatus.textContent = "Aperçu 3D indisponible.";
        }
      }
      viewer.showModal();
    }

    document.getElementById("query").addEventListener("input", function (event) {
      query = event.target.value;
      shown = PAGE;
      render();
    });
    document.querySelectorAll("nav button").forEach(function (button) {
      button.addEventListener("click", function () {
        mode = button.getAttribute("data-mode");
        shown = PAGE;
        document.querySelectorAll("nav button").forEach(function (other) {
          other.setAttribute("aria-pressed", other === button ? "true" : "false");
        });
        render();
      });
    });
    more.addEventListener("click", function () {
      shown += PAGE;
      render();
    });
    document.getElementById("close").addEventListener("click", function () { viewer.close(); });
    viewer.addEventListener("click", function (event) {
      if (event.target === viewer) viewer.close();
    });
    viewer.addEventListener("close", function () {
      currentItem = null;
      stage.replaceChildren();
      document.getElementById("viewer-status").hidden = true;
    });
    document.getElementById("new-group").addEventListener("submit", function (event) {
      event.preventDefault();
      const input = event.target.elements.name;
      const name = input.value;
      saveGroups({ action: "create", name: name }).then(function () {
        input.value = "";
      }).catch(function (error) {
        status.hidden = false;
        status.textContent = error.message;
      });
    });
    document.getElementById("zip-group").addEventListener("click", function () {
      downloadGroup(groupById(activeGroup));
    });
    document.getElementById("drop-group").addEventListener("click", function () {
      const group = groupById(activeGroup);
      if (!group) return;
      if (!confirm("Supprimer le groupe " + group.name + " ?")) return;
      saveGroups({ action: "delete", id: group.id }).catch(function (error) {
        status.hidden = false;
        status.textContent = error.message;
      });
    });

    fetch("/manifest.json")
      .then(function (response) { if (!response.ok) throw new Error(String(response.status)); return response.json(); })
      .then(function (items) {
        all = items;
        return fetch("/galerie/groupes").then(function (response) {
          if (!response.ok) return { groups: [] };
          return response.json();
        }).catch(function () { return { groups: [] }; });
      })
      .then(function (data) {
        groups = data.groups || [];
        status.hidden = true;
        renderGroups();
        render();
      })
      .catch(function () {
        status.textContent = "Le catalogue n’a pas pu être chargé.";
      });
  </script>
</body>
</html>`;
}

function contentRangeHeader(range, size) {
  let start;
  let end;
  if (typeof range.suffix === "number") {
    const length = Math.min(range.suffix, size);
    start = size - length;
    end = size - 1;
  } else {
    start = range.offset || 0;
    const length = typeof range.length === "number" ? range.length : size - start;
    end = Math.min(start + length, size) - 1;
  }
  if (end < start) return `bytes */${size}`;
  return `bytes ${start}-${end}/${size}`;
}

function objectResponse(object, request) {
  const headers = new Headers();
  object.writeHttpMetadata(headers);
  headers.set("etag", object.httpEtag);
  headers.set("cache-control", "public, max-age=86400");
  headers.set("accept-ranges", "bytes");
  const size = object.size;
  let status = 200;
  if (object.range && request.headers.has("range")) {
    const contentRange = contentRangeHeader(object.range, size);
    headers.set("content-range", contentRange);
    const match = contentRange.match(/bytes (\d+)-(\d+)\//);
    headers.set("content-length", match ? String(Number(match[2]) - Number(match[1]) + 1) : String(size));
    status = 206;
  } else {
    headers.set("content-length", String(size));
  }
  const body = request.method === "HEAD" ? null : object.body;
  return withCors(new Response(body, { status, headers }));
}

function jsonResponse(data, status) {
  return new Response(JSON.stringify(data), {
    status: status || 200,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
      "x-robots-tag": "noindex, nofollow",
    },
  });
}

function cleanName(value) {
  const name = String(value || "").replace(/\s+/g, " ").trim();
  if (!name || name.length > 40) return "";
  return name;
}

function cleanPath(value) {
  const path = String(value || "").replaceAll("\\", "/").trim();
  if (!path || path.length > 500 || path.startsWith("/") || path.includes("..")) return "";
  if (/[\u0000-\u001f?#\\]/.test(path)) return "";
  return path;
}

async function loadGroups(env) {
  const object = await env.BUCKET.get(GROUPS_KEY);
  if (!object) return { groups: [] };
  try {
    const data = await object.json();
    if (!data || !Array.isArray(data.groups)) return { groups: [] };
    return {
      groups: data.groups.filter((group) => group && typeof group.id === "string" && typeof group.name === "string" && Array.isArray(group.paths)),
    };
  } catch {
    return { groups: [] };
  }
}

async function saveGroups(env, data) {
  await env.BUCKET.put(GROUPS_KEY, JSON.stringify(data), {
    httpMetadata: { contentType: "application/json", cacheControl: "no-store" },
  });
}

async function handleGroups(request, env) {
  const data = await loadGroups(env);
  if (request.method === "GET" || request.method === "HEAD") {
    return jsonResponse(data);
  }
  if (request.method !== "POST") {
    return jsonResponse({ error: "Méthode refusée" }, 405);
  }
  let body;
  try {
    body = await request.json();
  } catch {
    return jsonResponse({ error: "Requête illisible" }, 400);
  }
  const action = String(body.action || "");
  if (action === "create") {
    const name = cleanName(body.name);
    if (!name) return jsonResponse({ error: "Nom invalide" }, 400);
    if (data.groups.length >= 40) return jsonResponse({ error: "Trop de groupes" }, 400);
    if (data.groups.some((group) => group.name.toLowerCase() === name.toLowerCase())) {
      return jsonResponse({ error: "Ce groupe existe déjà" }, 400);
    }
    data.groups.push({ id: crypto.randomUUID(), name, paths: [] });
    await saveGroups(env, data);
    return jsonResponse(data);
  }
  const group = data.groups.find((item) => item.id === body.id);
  if (!group) return jsonResponse({ error: "Groupe introuvable" }, 404);
  if (action === "delete") {
    data.groups = data.groups.filter((item) => item.id !== group.id);
    await saveGroups(env, data);
    return jsonResponse(data);
  }
  if (action === "set") {
    const path = cleanPath(body.path);
    if (!path) return jsonResponse({ error: "Icône invalide" }, 400);
    const member = Boolean(body.member);
    const has = group.paths.includes(path);
    if (member && !has) {
      if (group.paths.length >= 5000) return jsonResponse({ error: "Groupe plein" }, 400);
      group.paths.push(path);
    } else if (!member && has) {
      group.paths = group.paths.filter((item) => item !== path);
    }
    await saveGroups(env, data);
    return jsonResponse(data);
  }
  return jsonResponse({ error: "Action inconnue" }, 400);
}

async function handleGallery(request, env, path) {
  if (!env.GALLERY_PASSWORD) {
    return htmlResponse("<p>Galerie non configurée.</p>", 503);
  }
  if (path === "galerie/groupes") {
    if (!(await galleryAuthorized(request, env))) {
      return jsonResponse({ error: "Connexion requise" }, 401);
    }
    return handleGroups(request, env);
  }
  if (path === "galerie/apercu") {
    if (!(await galleryAuthorized(request, env))) {
      return jsonResponse({ error: "Connexion requise" }, 401);
    }
    if (request.method !== "POST") return jsonResponse({ error: "Méthode refusée" }, 405);
    const assetPath = cleanPath(new URL(request.url).searchParams.get("path") || "");
    if (!assetPath) return jsonResponse({ error: "Modèle invalide" }, 400);
    const advertised = Number(request.headers.get("content-length") || 0);
    if (advertised > 400000) return jsonResponse({ error: "Aperçu trop lourd" }, 413);
    const bytes = new Uint8Array(await request.arrayBuffer());
    if (bytes.length < 12 || bytes.length > 400000) return jsonResponse({ error: "Aperçu invalide" }, 400);
    const riff = String.fromCharCode(bytes[0], bytes[1], bytes[2], bytes[3]);
    const format = String.fromCharCode(bytes[8], bytes[9], bytes[10], bytes[11]);
    if (riff !== "RIFF" || format !== "WEBP") return jsonResponse({ error: "Aperçu invalide" }, 400);
    await env.BUCKET.put(`thumbs/${assetPath}.webp`, bytes, {
      httpMetadata: { contentType: "image/webp", cacheControl: "public, max-age=31536000" },
    });
    return jsonResponse({ ok: true });
  }
  if (path === "galerie/sortir") {
    return new Response(null, {
      status: 302,
      headers: { location: "/galerie", "set-cookie": galleryCookie("") },
    });
  }
  if (request.method === "POST") {
    const form = await request.formData();
    const password = String(form.get("password") || "");
    if (!(await passwordMatches(password, env.GALLERY_PASSWORD))) {
      return htmlResponse(loginPage(true), 401);
    }
    const token = [...(await passwordDigest(env.GALLERY_PASSWORD))]
      .map((byte) => byte.toString(16).padStart(2, "0"))
      .join("");
    return new Response(null, {
      status: 303,
      headers: { location: "/galerie", "set-cookie": galleryCookie(token) },
    });
  }
  if (request.method !== "GET" && request.method !== "HEAD") {
    return new Response("Méthode refusée", { status: 405 });
  }
  if (!(await galleryAuthorized(request, env))) {
    return htmlResponse(loginPage(false), 200);
  }
  return htmlResponse(galleryPage(), 200);
}

export default {
  async fetch(request, env) {
    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: CORS });
    }
    const url = new URL(request.url);
    const key = decodeURIComponent(url.pathname.replace(/^\/+/, ""));
    if (!key || key.includes("..")) {
      return new Response("Chemin invalide", { status: 400 });
    }
    if (key === "robots.txt" && (request.method === "GET" || request.method === "HEAD")) {
      return new Response("User-agent: *\nDisallow: /\n", {
        headers: { "content-type": "text/plain; charset=utf-8", "x-robots-tag": "noindex" },
      });
    }
    if (key === "private" || key.startsWith("private/")) {
      return new Response("Introuvable", { status: 404 });
    }
    if (key === "galerie" || key === "galerie/sortir" || key === "galerie/groupes" || key === "galerie/apercu") {
      return handleGallery(request, env, key);
    }

    if (request.method === "PUT") {
      const uploadKey = request.headers.get("X-Lumen-Upload") || "";
      if (!env.UPLOAD_KEY || !(await passwordMatches(uploadKey, env.UPLOAD_KEY))) {
        return new Response("Interdit", { status: 401 });
      }
      await env.BUCKET.put(key, request.body, {
        httpMetadata: {
          contentType: request.headers.get("content-type") || "application/octet-stream",
          cacheControl: "public, max-age=86400",
        },
      });
      return new Response("ok");
    }

    if (request.method === "GET" || request.method === "HEAD") {
      const object = await env.BUCKET.get(key, request.headers.has("range") ? { range: request.headers } : undefined);
      if (!object) return new Response("Introuvable", { status: 404 });
      return objectResponse(object, request);
    }

    return new Response("Méthode refusée", { status: 405 });
  },
};
