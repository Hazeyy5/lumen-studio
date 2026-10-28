import { Fragment, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  buildRobloxReport,
  formatCount,
  formatDelta,
  isFresh,
  isRobloxFresh,
  loadTrends,
  readCachedRoblox,
  readCachedTrends,
  SORT_LABELS,
  trendsBrief,
  VERDICT_LABELS,
  type FortniteMatch,
  type RobloxGame,
  type RobloxReport,
  type RobloxVerdict,
  type TrendIsland,
  type TrendSnapshot,
} from "./fortniteTrends";

const NOTE_KEY = "lumen.trends.note";
const ADVICE_KEY = "lumen.trends.advice";

export function Trends() {
  const [data, setData] = useState<TrendSnapshot | null>(() => readCachedTrends());
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [err, setErr] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [note, setNote] = useState(() => localStorage.getItem(NOTE_KEY) ?? "");
  const [advice, setAdvice] = useState(() => localStorage.getItem(ADVICE_KEY) ?? "");
  const [thinking, setThinking] = useState(false);
  const [copied, setCopied] = useState(false);
  const [roblox, setRoblox] = useState<RobloxReport | null>(() => readCachedRoblox());
  const [robloxStep, setRobloxStep] = useState("");
  const [robloxErr, setRobloxErr] = useState("");
  const [verdictFilter, setVerdictFilter] = useState<RobloxVerdict | "all">("all");

  async function loadRoblox(force = false, snapshot = data) {
    if (robloxStep) return;
    setRobloxErr("");
    setRobloxStep("Jeux en tendance sur Roblox…");
    const unlisten = await listen<{ islands: number; first: boolean }>("catalog-progress", (event) => {
      setRobloxStep(
        event.payload.first
          ? `Premier téléchargement du catalogue Fortnite : ${formatCount(event.payload.islands)} îles (environ 1 min 30, une seule fois)`
          : `Mise à jour du catalogue Fortnite : ${formatCount(event.payload.islands)} îles`,
      );
    });
    try {
      const games = await invoke<RobloxGame[]>("roblox_trending", { limit: 40 });
      setRobloxStep("Recherche des copies dans le catalogue Fortnite…");
      const found = await invoke<{ islands: number; matches: FortniteMatch[] }>("fortnite_catalog_match", {
        names: games.map((g) => g.name),
        prefer: snapshot?.ranked ?? [],
        force,
      });
      const report = await buildRobloxReport(games, found.matches, found.islands, (done, total) =>
        setRobloxStep(`Joueurs des copies Fortnite : ${done}/${total}`),
      );
      setRoblox(report);
    } catch (error) {
      setRobloxErr(String(error));
    } finally {
      unlisten();
      setRobloxStep("");
    }
  }

  async function refresh(): Promise<TrendSnapshot | null> {
    if (progress) return null;
    setErr("");
    setProgress({ done: 0, total: 1 });
    try {
      const next = await loadTrends((done, total) => setProgress({ done, total }));
      setData(next);
      setOpen((prev) => prev ?? next.genres[0]?.slug ?? null);
      return next;
    } catch (error) {
      setErr(String(error));
      return null;
    } finally {
      setProgress(null);
    }
  }

  useEffect(() => {
    void (async () => {
      let snapshot = data;
      if (!isFresh(data)) snapshot = (await refresh()) ?? data;
      else setOpen(data?.genres[0]?.slug ?? null);
      if (!isRobloxFresh(roblox)) await loadRoblox(false, snapshot);
    })();
  }, []);

  useEffect(() => {
    localStorage.setItem(NOTE_KEY, note);
  }, [note]);

  async function ask() {
    if (!data || thinking) return;
    setThinking(true);
    setErr("");
    try {
      const text = await invoke<string>("trends_advice", { prompt: trendsBrief(data, note, roblox) });
      setAdvice(text);
      localStorage.setItem(ADVICE_KEY, text);
    } catch (error) {
      setErr(String(error));
    } finally {
      setThinking(false);
    }
  }

  async function copyBrief() {
    if (!data) return;
    await navigator.clipboard.writeText(trendsBrief(data, note, roblox));
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  }

  const maxScore = Math.max(1, ...(data?.genres.map((g) => g.score) ?? [1]));
  const maxTag = Math.max(1, ...(data?.tags.map((t) => t.ccu) ?? [1]));

  return (
    <div className="trends">
      <h1>Tendances Fortnite</h1>
      <p className="lede">
        Les classements officiels d’Epic par genre, mis à jour chaque heure. Lumen croise les joueurs, le nombre d’îles
        en concurrence et l’évolution sur 7 jours, puis l’IA te dit quoi construire.
      </p>

      <div className="toolbar trends-bar">
        <span className="trends-stamp">
          {data
            ? `Classement du ${new Date(data.snapshot).toLocaleString("fr-FR", { dateStyle: "short", timeStyle: "short" })} · ${formatCount(data.ecoCcu)} joueurs en partie au pic d’hier${data.ecoCcu && data.ecoCcu7 ? ` (${formatDelta(data.ecoCcu / data.ecoCcu7 - 1)} sur 7 j)` : ""}`
            : "Aucune donnée encore"}
        </span>
        <button className="btn secondary" type="button" disabled={Boolean(progress)} onClick={() => void refresh()}>
          {progress ? "Chargement…" : "Actualiser"}
        </button>
      </div>
      {progress ? (
        <div className="trends-progress" aria-label="Chargement des tendances">
          <span style={{ width: `${Math.round((progress.done / Math.max(1, progress.total)) * 100)}%` }} />
          <small>
            {progress.done}/{progress.total} requêtes · l’API d’Epic limite le débit, compte environ une minute
          </small>
        </div>
      ) : null}
      {err ? <p className="err">{err}</p> : null}

      {data ? (
        <>
          <section className="trends-ai card">
            <div className="trends-ai-head">
              <div>
                <h2>Quelle map faire ?</h2>
                <small>Ton agent (Claude Code, sinon Codex) lit ces chiffres et propose 3 idées argumentées.</small>
              </div>
              <div className="trends-ai-actions">
                <button className="btn secondary" type="button" onClick={() => void copyBrief()}>
                  {copied ? "Copié" : "Copier le brief"}
                </button>
                <button className="btn copper" type="button" disabled={thinking} onClick={() => void ask()}>
                  {thinking ? "L’IA réfléchit…" : advice ? "Redemander" : "Demander à l’IA"}
                </button>
              </div>
            </div>
            <textarea
              rows={2}
              placeholder="Ton contexte (facultatif) : seul ou en équipe, niveau en Verse, temps dispo, ce que tu aimes faire…"
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
            {advice ? <div className="trends-advice">{renderAdvice(advice)}</div> : null}
          </section>

          <section>
            <h2 className="trends-h2">Genres, du plus prometteur au moins prometteur</h2>
            <p className="trends-help">
              Le score combine les joueurs du top 10 (35 %), leur évolution sur 7 jours (25 %), le peu d’îles par joueur
              (25 %) et l’absence d’un hit qui écrase tout (15 %). Clique sur un genre pour voir son top 10.
            </p>
            <div className="trends-table" role="table">
              <div className="trends-row head" role="row">
                <span>Genre</span>
                <span>Score</span>
                <span title="Pic de joueurs cumulé du top 10, dernière journée complète">Joueurs top 10</span>
                <span title="Évolution sur 7 jours des îles du top 10 actuel">7 jours</span>
                <span title="Îles classées dans ce genre">Îles</span>
                <span title="Part des joueurs du top 10 captée par le n°1">Part du n°1</span>
                <span title="Pic de joueurs de la 10e île : le seuil pour entrer dans le top 10">Seuil top 10</span>
              </div>
              {data.genres.map((g) => (
                <Fragment key={g.slug}>
                  <button
                    type="button"
                    role="row"
                    className={`trends-row ${open === g.slug ? "on" : ""}`}
                    onClick={() => setOpen(open === g.slug ? null : g.slug)}
                  >
                    <span className="trends-genre">{g.name}</span>
                    <span className="trends-score">
                      <i style={{ width: `${(g.score / maxScore) * 100}%` }} />
                      <b>{g.score}</b>
                    </span>
                    <span>{formatCount(g.demand)}</span>
                    <span className={deltaClass(g.momentum)}>{formatDelta(g.momentum)}</span>
                    <span>{formatCount(g.total)}</span>
                    <span>{Math.round(g.top1Share * 100)} %</span>
                    <span>{formatCount(g.floor10)}</span>
                  </button>
                  {open === g.slug ? (
                    <div className="trends-islands">
                      {g.islands.map((island) => (
                        <IslandLine key={island.code} island={island} />
                      ))}
                    </div>
                  ) : null}
                </Fragment>
              ))}
            </div>
          </section>

          <section>
            <div className="trends-section-head">
              <div>
                <h2 className="trends-h2">Roblox → Fortnite</h2>
                <p className="trends-help">
                  Les jeux qui cartonnent sur Roblox, et s’ils existent déjà sur Fortnite. Lumen cherche dans les{" "}
                  {roblox ? formatCount(roblox.catalogSize) : "~285 k"} îles du catalogue celles dont le titre reprend le
                  jeu, puis lit leurs joueurs. « Déjà pris » : une copie dépasse 1 000 joueurs simultanés.
                </p>
              </div>
              <button className="btn secondary" type="button" disabled={Boolean(robloxStep)} onClick={() => void loadRoblox(true)}>
                {robloxStep ? "Chargement…" : "Actualiser"}
              </button>
            </div>
            {robloxStep ? <p className="trends-step">{robloxStep}</p> : null}
            {robloxErr ? <p className="err">{robloxErr}</p> : null}
            {roblox ? (
              <>
                <div className="bank-shelves trends-filters" role="tablist">
                  {(["all", "libre", "faible", "pris"] as const).map((key) => (
                    <button
                      key={key}
                      type="button"
                      role="tab"
                      className={`bank-shelf ${verdictFilter === key ? "on" : ""}`}
                      onClick={() => setVerdictFilter(key)}
                    >
                      {key === "all" ? "Tous" : VERDICT_LABELS[key]}{" "}
                      <span>{key === "all" ? roblox.games.length : roblox.games.filter((g) => g.verdict === key).length}</span>
                    </button>
                  ))}
                </div>
                <div className="trends-roblox">
                  {roblox.games
                    .filter((g) => verdictFilter === "all" || g.verdict === verdictFilter)
                    .map((g) => (
                      <div key={g.universeId} className="trends-rbx">
                        <span className="trends-title">
                          <button
                            type="button"
                            className="trends-link"
                            onClick={() => void openUrl(`https://www.roblox.com/games/${g.placeId}`)}
                          >
                            {g.name}
                          </button>
                          <small>{[g.genre, ...g.sorts.map((x) => SORT_LABELS[x] ?? x)].filter(Boolean).join(" · ")}</small>
                        </span>
                        <span className="trends-num">
                          <b>{formatCount(g.players)}</b>
                          <small>sur Roblox</small>
                        </span>
                        <span className={`pill ${g.verdict === "libre" ? "ok" : g.verdict === "pris" ? "no" : "watch"}`}>
                          {VERDICT_LABELS[g.verdict]}
                        </span>
                        <span className="trends-title">
                          {g.best ? (
                            <>
                              <b>{g.best.title}</b>
                              <small>
                                {formatCount(g.best.ccu)} joueurs · {g.copies} île{g.copies > 1 ? "s" : ""} au titre proche ·{" "}
                                {g.best.code}
                              </small>
                            </>
                          ) : (
                            <small>Aucune île Fortnite au titre proche</small>
                          )}
                        </span>
                      </div>
                    ))}
                </div>
              </>
            ) : null}
          </section>

          <section>
            <h2 className="trends-h2">Les îles qui montent</h2>
            <p className="trends-help">Entrées récentes dans le top 25 de leur genre, ou remontées d’au moins 5 places en 7 jours.</p>
            <div className="grid trends-risers">
              {data.risers.map((island) => (
                <div key={`${island.genre}-${island.code}`} className="card trends-riser">
                  <span className="engine-tag uefn">{island.genre}</span>
                  <h3>{island.title}</h3>
                  <div className="trends-riser-stats">
                    <span>
                      <b>{formatCount(island.ccu)}</b> joueurs
                    </span>
                    <span className="up">
                      {island.oldRank ? `#${island.oldRank} → #${island.rank}` : `nouveau · #${island.rank}`}
                    </span>
                  </div>
                  <Spark series={island.series} />
                  <small>{island.tags.join(" · ")}</small>
                  <code>{island.code}</code>
                </div>
              ))}
            </div>
          </section>

          <section>
            <h2 className="trends-h2">Tags qui attirent les joueurs</h2>
            <p className="trends-help">Taille = joueurs cumulés des îles qui portent ce tag. Couleur = évolution sur 7 jours.</p>
            <div className="trends-tags">
              {data.tags.map((t) => (
                <span
                  key={t.tag}
                  className={`trends-tag ${deltaClass(t.momentum)}`}
                  style={{ fontSize: `${13 + Math.sqrt(t.ccu / maxTag) * 13}px` }}
                  title={`${formatCount(t.ccu)} joueurs · ${t.islands} îles · ${formatDelta(t.momentum)} sur 7 j`}
                >
                  {t.tag}
                </span>
              ))}
            </div>
          </section>
        </>
      ) : null}
    </div>
  );
}

function IslandLine({ island }: { island: TrendIsland }) {
  const move = island.oldRank == null ? "nouveau" : island.oldRank - island.rank;
  return (
    <div className="trends-island">
      <span className="trends-rank">#{island.rank}</span>
      <span className="trends-title">
        <b>{island.title}</b>
        <small>
          {island.creator ? `${island.creator} · ` : ""}
          {island.tags.join(" · ")}
        </small>
      </span>
      <Spark series={island.series} />
      <span className="trends-num">
        <b>{formatCount(island.ccu)}</b>
        <small>joueurs</small>
      </span>
      <span className="trends-num">
        <b>{island.minutes != null ? `${Math.round(island.minutes)} min` : "—"}</b>
        <small>par joueur</small>
      </span>
      <span className={`trends-move ${move === "nouveau" || (typeof move === "number" && move > 0) ? "up" : move === 0 ? "" : "down"}`}>
        {move === "nouveau" ? "nouveau" : move === 0 ? "=" : move > 0 ? `▲ ${move}` : `▼ ${-move}`}
      </span>
      <code>{island.code}</code>
    </div>
  );
}

function Spark({ series }: { series: (number | null)[] }) {
  const values = series.slice(0, -1).map((v) => v ?? 0);
  if (values.length < 2 || Math.max(...values) === 0) return <span className="trends-spark" />;
  const max = Math.max(...values);
  const points = values.map((v, i) => `${(i / (values.length - 1)) * 60},${18 - (v / max) * 16}`).join(" ");
  return (
    <svg className="trends-spark" viewBox="0 0 60 20" aria-hidden="true">
      <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" />
    </svg>
  );
}

function deltaClass(x: number | null) {
  if (x == null) return "";
  if (x > 0.05) return "up";
  if (x < -0.05) return "down";
  return "";
}

// Mini rendu du format demandé à l'IA : « ## titre », « - point », **gras**.
function renderAdvice(text: string) {
  const blocks: ReactNode[] = [];
  let list: ReactNode[] = [];
  const flush = () => {
    if (list.length) blocks.push(<ul key={`ul-${blocks.length}`}>{list}</ul>);
    list = [];
  };
  text.split(/\r?\n/).forEach((raw, i) => {
    const line = raw.trim();
    if (!line) return flush();
    if (/^#{1,4}\s/.test(line)) {
      flush();
      blocks.push(<h3 key={i}>{bold(line.replace(/^#{1,4}\s*/, ""))}</h3>);
    } else if (/^[-*•]\s/.test(line)) {
      list.push(<li key={i}>{bold(line.replace(/^[-*•]\s*/, ""))}</li>);
    } else {
      flush();
      blocks.push(<p key={i}>{bold(line)}</p>);
    }
  });
  flush();
  return blocks;
}

function bold(line: string) {
  return line.split(/\*\*(.+?)\*\*/g).map((part, i) => (i % 2 ? <strong key={i}>{part}</strong> : part));
}
