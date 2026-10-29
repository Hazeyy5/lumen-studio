// Tendances Fortnite depuis l'API publique d'Epic (api.fortnite.com, sans clé).
// Classements par genre (maintenant et il y a 7 jours) + métriques du top de chaque genre.

import { invoke } from "@tauri-apps/api/core";

const CACHE_KEY = "lumen.trends.v1";
const CACHE_MS = 60 * 60 * 1000;
const TOP = 10;
const RISER_DEPTH = 25;

export type TrendIsland = {
  code: string;
  title: string;
  creator: string;
  tags: string[];
  genre: string;
  rank: number;
  oldRank: number | null;
  ccu: number | null;
  ccu7: number | null;
  players: number | null;
  minutes: number | null;
  d1: number | null;
  series: (number | null)[];
};

export type TrendGenre = {
  slug: string;
  name: string;
  total: number;
  total7: number | null;
  demand: number;
  momentum: number | null;
  top1Share: number;
  floor10: number | null;
  minutes: number | null;
  d1: number | null;
  score: number;
  islands: TrendIsland[];
};

export type TrendTag = { tag: string; ccu: number; momentum: number | null; islands: number };

export type TrendSnapshot = {
  snapshot: string;
  fetchedAt: number;
  ecoCcu: number | null;
  ecoCcu7: number | null;
  genres: TrendGenre[];
  tags: TrendTag[];
  risers: TrendIsland[];
  ranked: string[];
};

type Point = { value: number | null; timestamp: string };
type Metrics = Record<string, Point[]> & { retention?: { d1: number | null; timestamp: string }[] };
type Meta = { code: string; creatorCode?: string; title?: string; tags?: string[] };

// Passe par Rust : réessais sur 429/5xx, et pas de CORS pour transformer un 429 en « Failed to fetch ».
async function api<T>(path: string): Promise<T | null> {
  return invoke<T | null>("fortnite_api", { path });
}

async function pool<T, R>(items: T[], size: number, work: (item: T) => Promise<R>, tick: () => void) {
  const out: R[] = new Array(items.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: Math.min(size, items.length) }, async () => {
      while (next < items.length) {
        const i = next++;
        out[i] = await work(items[i]);
        tick();
      }
    }),
  );
  return out;
}

// Le dernier jour est en cours : on lit l'avant-dernier, complet.
function lastFull(points: (number | null)[]) {
  for (let i = points.length - 2; i >= 0; i -= 1) if (points[i] != null) return points[i];
  return points[points.length - 1] ?? null;
}

function avg(values: (number | null)[]) {
  const ok = values.filter((v): v is number => v != null);
  return ok.length ? ok.reduce((a, b) => a + b, 0) / ok.length : null;
}

function spread(values: number[]) {
  const min = Math.min(...values);
  const max = Math.max(...values);
  return (v: number) => (max === min ? 0.5 : (v - min) / (max - min));
}

function isoHour(d: Date) {
  return `${d.toISOString().slice(0, 13)}:00:00.000Z`;
}

export function readCachedTrends(): TrendSnapshot | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    return raw ? (JSON.parse(raw) as TrendSnapshot) : null;
  } catch {
    return null;
  }
}

export function isFresh(snapshot: TrendSnapshot | null) {
  return Boolean(snapshot && Date.now() - snapshot.fetchedAt < CACHE_MS);
}

export async function loadTrends(onProgress: (done: number, total: number) => void): Promise<TrendSnapshot> {
  const genres = (await api<{ data: { slug: string; displayName: string }[] }>("/genres"))?.data ?? [];
  const weekAgo = isoHour(new Date(Date.now() - (7 * 24 - 2) * 3600 * 1000));
  const from = `${new Date(Date.now() - 7 * 24 * 3600 * 1000).toISOString().slice(0, 10)}T00:00:00.000Z`;

  type Ranking = { meta: { total: number; snapshot?: string }; data: { islandCode: string; rank: number }[] };
  let done = 0;
  let total = genres.length * 2;
  const tick = () => onProgress(++done, total);

  const rankings = await pool(
    genres.flatMap((g) => [`/genres/${g.slug}/rankings?size=100`, `/genres/${g.slug}/rankings?size=100&at=${encodeURIComponent(weekAgo)}`]),
    3,
    (path) => api<Ranking>(path),
    tick,
  );

  const boards = genres.map((g, i) => {
    const now = rankings[i * 2];
    const old = rankings[i * 2 + 1];
    const oldRank = new Map((old?.data ?? []).map((row) => [row.islandCode, row.rank]));
    const rows = (now?.data ?? []).map((row) => ({ code: row.islandCode, rank: row.rank, oldRank: oldRank.get(row.islandCode) ?? null }));
    return { genre: g, now, old, rows };
  });

  const wanted = new Set<string>();
  for (const board of boards) {
    for (const row of board.rows.slice(0, RISER_DEPTH)) {
      const rising = row.oldRank == null || row.oldRank - row.rank >= 5;
      if (row.rank <= TOP || rising) wanted.add(row.code);
    }
  }
  const codes = [...wanted];
  total += codes.length;

  const fetched = await pool(
    codes,
    3,
    async (code) => {
      const [meta, metrics] = await Promise.all([
        api<Meta>(`/islands/${code}`),
        api<Metrics>(`/islands/${code}/metrics?from=${encodeURIComponent(from)}`),
      ]);
      return { code, meta, metrics };
    },
    tick,
  );
  const byCode = new Map(fetched.map((item) => [item.code, item]));

  const island = (genre: string, row: { code: string; rank: number; oldRank: number | null }): TrendIsland => {
    const item = byCode.get(row.code);
    const m = item?.metrics;
    const series = (m?.peakCCU ?? []).map((p) => p.value);
    return {
      code: row.code,
      title: item?.meta?.title ?? row.code,
      creator: item?.meta?.creatorCode ?? "",
      tags: item?.meta?.tags ?? [],
      genre,
      rank: row.rank,
      oldRank: row.oldRank,
      ccu: lastFull(series),
      ccu7: series[0] ?? null,
      players: lastFull((m?.uniquePlayers ?? []).map((p) => p.value)),
      minutes: lastFull((m?.averageMinutesPerPlayer ?? []).map((p) => p.value)),
      d1: avg((m?.retention ?? []).map((p) => p.d1)),
      series,
    };
  };

  const rawGenres = boards.map(({ genre, now, old, rows }) => {
    const islands = rows.filter((row) => row.rank <= TOP).map((row) => island(genre.displayName, row));
    const ccus = islands.map((i) => i.ccu ?? 0);
    const demand = ccus.reduce((a, b) => a + b, 0);
    const withPast = islands.filter((i) => i.ccu != null);
    const past = withPast.reduce((a, i) => a + (i.ccu7 ?? 0), 0);
    const momentum = past > 0 ? Math.min(2, withPast.reduce((a, i) => a + (i.ccu ?? 0), 0) / past - 1) : null;
    return {
      slug: genre.slug,
      name: genre.displayName,
      total: now?.meta.total ?? 0,
      total7: old?.meta.total ?? null,
      demand,
      momentum,
      top1Share: demand > 0 ? Math.max(...ccus) / demand : 1,
      floor10: islands.length >= TOP ? islands[TOP - 1].ccu : null,
      minutes: avg(islands.map((i) => i.minutes)),
      d1: avg(islands.map((i) => i.d1)),
      score: 0,
      islands,
      risers: rows
        .slice(0, RISER_DEPTH)
        .filter((row) => row.oldRank == null || row.oldRank - row.rank >= 5)
        .map((row) => island(genre.displayName, row)),
    };
  });

  // Score d'opportunité : demande forte, en hausse, peu d'îles par joueur, pas monopolisée par un seul hit.
  const live = rawGenres.filter((g) => g.demand > 0);
  const nDemand = spread(live.map((g) => Math.log10(g.demand)));
  const nMomentum = spread(live.map((g) => Math.max(-0.5, Math.min(1, g.momentum ?? 0))));
  const nCrowd = spread(live.map((g) => Math.log10(Math.max(1, g.total) / g.demand)));
  for (const g of rawGenres) {
    if (g.demand <= 0) continue;
    g.score = Math.round(
      100 *
        (0.35 * nDemand(Math.log10(g.demand)) +
          0.25 * nMomentum(Math.max(-0.5, Math.min(1, g.momentum ?? 0))) +
          0.25 * (1 - nCrowd(Math.log10(Math.max(1, g.total) / g.demand))) +
          0.15 * (1 - g.top1Share)),
    );
  }

  const seen = new Map<string, TrendIsland>();
  for (const g of rawGenres) for (const i of [...g.islands, ...g.risers]) if (!seen.has(i.code)) seen.set(i.code, i);

  const tagMap = new Map<string, { ccu: number; past: number; islands: number }>();
  for (const i of seen.values()) {
    if (i.ccu == null) continue;
    for (const tag of i.tags) {
      const t = tagMap.get(tag) ?? { ccu: 0, past: 0, islands: 0 };
      t.ccu += i.ccu;
      t.past += i.ccu7 ?? 0;
      t.islands += 1;
      tagMap.set(tag, t);
    }
  }
  const tags = [...tagMap.entries()]
    .map(([tag, t]) => ({ tag, ccu: t.ccu, islands: t.islands, momentum: t.past > 0 ? t.ccu / t.past - 1 : null }))
    .sort((a, b) => b.ccu - a.ccu)
    .slice(0, 24);

  const risers = rawGenres
    .flatMap((g) => g.risers)
    .filter((i) => (i.ccu ?? 0) >= 100)
    .sort((a, b) => riserWeight(b) - riserWeight(a))
    .filter((i, idx, all) => all.findIndex((j) => j.code === i.code) === idx)
    .slice(0, 12);

  const eco = await api<Record<string, Point[]>>(`/metrics/day?from=${encodeURIComponent(from)}`);
  const ecoSeries = (eco?.inMatchPeakCCU ?? []).map((p) => p.value);

  const snapshot: TrendSnapshot = {
    snapshot: boards.find((b) => b.now?.meta.snapshot)?.now?.meta.snapshot ?? new Date().toISOString(),
    fetchedAt: Date.now(),
    ecoCcu: lastFull(ecoSeries),
    ecoCcu7: ecoSeries[0] ?? null,
    genres: rawGenres.map(({ risers: _risers, ...g }) => g).sort((a, b) => b.score - a.score),
    tags,
    risers,
    ranked: boards
      .flatMap((b) => b.rows)
      .sort((a, b) => a.rank - b.rank)
      .map((row) => row.code)
      .filter((code, idx, all) => all.indexOf(code) === idx),
  };
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(snapshot));
  } catch {
    /* quota : on garde juste en mémoire */
  }
  return snapshot;
}

// ---------- Roblox → Fortnite : un jeu Roblox en tendance a-t-il déjà sa version Fortnite ? ----------

const ROBLOX_CACHE_KEY = "lumen.trends.roblox.v1";
export const TAKEN_CCU = 1000;
const CANDIDATES = 3;

export type RobloxGame = {
  universeId: number;
  placeId: number;
  name: string;
  players: number;
  genre: string;
  upVotes: number;
  downVotes: number;
  sorts: string[];
};

export type FortniteMatch = {
  name: string;
  copies: number;
  candidates: { code: string; title: string; creator: string; tags: string[] }[];
};

export type FortniteCopy = { code: string; title: string; creator: string; tags: string[]; ccu: number | null };

export type RobloxVerdict = "libre" | "faible" | "pris";

export type RobloxCheck = RobloxGame & {
  copies: number;
  best: FortniteCopy | null;
  verdict: RobloxVerdict;
};

export type RobloxReport = { fetchedAt: number; catalogSize: number; games: RobloxCheck[] };

export const SORT_LABELS: Record<string, string> = {
  "top-trending": "Tendance",
  "up-and-coming": "En montée",
  "top-playing-now": "Top joueurs",
};

export const VERDICT_LABELS: Record<RobloxVerdict, string> = {
  libre: "Libre sur Fortnite",
  faible: "Copies faibles",
  pris: "Déjà pris",
};

export function readCachedRoblox(): RobloxReport | null {
  try {
    const raw = localStorage.getItem(ROBLOX_CACHE_KEY);
    return raw ? (JSON.parse(raw) as RobloxReport) : null;
  } catch {
    return null;
  }
}

export function isRobloxFresh(report: RobloxReport | null) {
  return Boolean(report && Date.now() - report.fetchedAt < CACHE_MS);
}

// Les candidats viennent du catalogue (titres proches) ; on lit le pic de joueurs d'hier des 3 premiers
// pour savoir si la place est déjà prise par une île qui marche.
export async function buildRobloxReport(
  games: RobloxGame[],
  matches: FortniteMatch[],
  catalogSize: number,
  onProgress: (done: number, total: number) => void,
): Promise<RobloxReport> {
  const byName = new Map(matches.map((m) => [m.name, m]));
  const codes = [...new Set(matches.flatMap((m) => m.candidates.slice(0, CANDIDATES).map((c) => c.code)))];
  let done = 0;
  const from = `${new Date(Date.now() - 2 * 24 * 3600 * 1000).toISOString().slice(0, 10)}T00:00:00.000Z`;
  const ccus = await pool(
    codes,
    3,
    async (code) => {
      const m = await api<{ intervals?: Point[] }>(`/islands/${code}/metrics/day/peak-ccu?from=${encodeURIComponent(from)}`);
      return lastFull((m?.intervals ?? []).map((p) => p.value));
    },
    () => onProgress(++done, codes.length),
  );
  const ccuOf = new Map(codes.map((code, i) => [code, ccus[i]]));

  const checks = games.map((game): RobloxCheck => {
    const match = byName.get(game.name);
    const copies = (match?.candidates ?? []).slice(0, CANDIDATES).map((c) => ({ ...c, ccu: ccuOf.get(c.code) ?? null }));
    const best = copies.sort((a, b) => (b.ccu ?? -1) - (a.ccu ?? -1))[0] ?? null;
    const verdict: RobloxVerdict = !match?.copies ? "libre" : (best?.ccu ?? 0) >= TAKEN_CCU ? "pris" : "faible";
    return { ...game, copies: match?.copies ?? 0, best, verdict };
  });

  const report: RobloxReport = { fetchedAt: Date.now(), catalogSize, games: checks.sort((a, b) => b.players - a.players) };
  try {
    localStorage.setItem(ROBLOX_CACHE_KEY, JSON.stringify(report));
  } catch {
    /* quota : on garde juste en mémoire */
  }
  return report;
}

function riserWeight(i: TrendIsland) {
  const growth = i.ccu7 ? (i.ccu ?? 0) / i.ccu7 : 3;
  return (i.ccu ?? 0) * Math.min(growth, 5);
}

export function formatCount(n: number | null | undefined) {
  if (n == null) return "—";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(".0", "")} M`;
  if (n >= 10_000) return `${Math.round(n / 1000)} k`;
  if (n >= 1000) return `${(n / 1000).toFixed(1).replace(".0", "")} k`;
  return String(Math.round(n));
}

export function formatDelta(x: number | null | undefined) {
  if (x == null) return "—";
  const pct = Math.round(x * 100);
  return `${pct > 0 ? "+" : ""}${pct} %`;
}

// Brief envoyé à l'agent : les chiffres utiles, compacts, puis la demande.
export function trendsBrief(s: TrendSnapshot, note: string, roblox?: RobloxReport | null) {
  const lines: string[] = [];
  lines.push("Tu conseilles un petit studio qui crée des îles Fortnite avec UEFN et Verse.");
  lines.push(`Données réelles de l'API publique Fortnite (classement du ${s.snapshot.slice(0, 16).replace("T", " ")} UTC).`);
  lines.push("CCU = pic de joueurs simultanés sur la dernière journée complète ; Δ7j = évolution sur 7 jours.");
  lines.push("");
  lines.push("## Genres (score d'opportunité Lumen /100, îles en concurrence, CCU cumulé du top 10, Δ7j, part du n°1, CCU du 10e, min/joueur, rétention J1)");
  for (const g of s.genres) {
    lines.push(
      `- ${g.name} : score ${g.score} · ${g.total} îles · top10 ${formatCount(g.demand)} · Δ7j ${formatDelta(g.momentum)} · n°1 ${Math.round(g.top1Share * 100)} % · 10e ${formatCount(g.floor10)} · ${g.minutes?.toFixed(0) ?? "—"} min · J1 ${g.d1 != null ? Math.round(g.d1 * 100) + " %" : "—"}`,
    );
  }
  lines.push("");
  lines.push("## Top 5 des 5 meilleurs genres");
  for (const g of s.genres.slice(0, 5)) {
    lines.push(`${g.name} :`);
    for (const i of g.islands.slice(0, 5)) {
      lines.push(`  ${i.rank}. ${i.title} — ${formatCount(i.ccu)} CCU (Δ7j ${i.ccu7 ? formatDelta((i.ccu ?? 0) / i.ccu7 - 1) : "nouveau"}) — ${i.tags.join(", ")}`);
    }
  }
  lines.push("");
  lines.push("## Îles qui montent (entrées récentes ou forte remontée dans leur genre)");
  for (const i of s.risers) {
    lines.push(`- ${i.title} (${i.genre}) — rang ${i.rank}${i.oldRank ? ` (était ${i.oldRank})` : " (absent il y a 7 j)"} — ${formatCount(i.ccu)} CCU — ${i.tags.join(", ")}`);
  }
  lines.push("");
  lines.push("## Tags pondérés par les joueurs (CCU cumulé, Δ7j)");
  lines.push(s.tags.map((t) => `${t.tag} ${formatCount(t.ccu)} (${formatDelta(t.momentum)})`).join(" · "));
  lines.push("");
  if (roblox?.games.length) {
    lines.push(`## Jeux Roblox en tendance et leur présence sur Fortnite (catalogue de ${roblox.catalogSize} îles)`);
    lines.push(`« Déjà pris » = une copie Fortnite dépasse ${TAKEN_CCU} CCU ; « copies faibles » = des îles reprennent le titre mais peu jouées ; « libre » = aucune île au titre proche.`);
    for (const g of roblox.games.slice(0, 30)) {
      const copy = g.best ? ` — meilleure copie : ${g.best.title} (${formatCount(g.best.ccu)} CCU), ${g.copies} îles au titre proche` : "";
      lines.push(`- ${g.name} (Roblox, ${g.genre || "genre ?"}, ${formatCount(g.players)} joueurs, ${g.sorts.map((x) => SORT_LABELS[x] ?? x).join("/")}) : ${VERDICT_LABELS[g.verdict]}${copy}`);
    }
    lines.push("");
  }
  if (note.trim()) {
    lines.push("## Contexte du créateur");
    lines.push(note.trim());
    lines.push("");
  }
  lines.push("## Ta mission");
  lines.push(
    "Propose 3 idées de map à faire maintenant, de la plus recommandée à la moins. Pour chacune : un nom accrocheur, le genre, pourquoi les chiffres la justifient (cite-les), la boucle de jeu en 2 phrases, ce qui la différencie des maps du top, la taille du projet en UEFN/Verse (petit, moyen, gros) et le principal risque. Termine par une phrase de verdict.",
  );
  if (roblox?.games.length) {
    lines.push(
      "Un hit Roblox libre ou mal copié sur Fortnite est une piste sérieuse : au moins une des 3 idées doit en adapter un, en disant ce qu'il faut changer pour le public Fortnite.",
    );
  }
  lines.push(
    "Réponds en français, sans tableau. Format : une ligne « ## » par idée, puis des lignes « - ». Mets en **gras** l'essentiel. Pas d'introduction.",
  );
  return lines.join("\n");
}
