import { useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { AgentTerminal } from "./AgentTerminal";
import { AssetReviewHost } from "./AssetReview";
import { AssetCode, AssetLightbox, bankImageSrc, lightboxFromBank, type LightboxAsset } from "./AssetLightbox";
import { MeshPreview, MeshStill } from "./MeshPreview";
import { ptyBus } from "./ptyBus";
import { imageBrief, imageFilesFromTransfer, savePastedFiles, type PastedImage } from "./pasteImage";
import { applyTheme, readTheme, toggleTheme, type Theme } from "./theme";
import type {
  AgentKind,
  AgentStatus,
  AgentTab,
  BankItem,
  CompilerStatus,
  Keys,
  LiveAgent,
  PlaceOffer,
  Project,
  RobloxUser,
  RojoStatus,
  StartedAgent,
  StudioHeartbeat,
  SwarmState,
  ToolchainStatus,
  View,
} from "./types";

const OAUTH_REDIRECT = "http://localhost:17421/callback";
const OAUTH_APPS = "https://create.roblox.com/dashboard/credentials?activeTab=OAuthTab";

function isGenericPlaceName(name?: string | null) {
  const n = (name ?? "").trim();
  return n === "" || n === "Game" || n === "Place" || n === "Workspace" || n === "DataModel";
}

function studioPlaceLabel(
  studio: StudioHeartbeat | null | undefined,
  offer?: PlaceOffer | null,
) {
  const raw = studio?.placeName?.trim() ?? "";
  if (studio?.connected && isGenericPlaceName(raw)) {
    const bound = offer?.boundPlaceName?.trim();
    if (bound && !isGenericPlaceName(bound)) return bound;
  }
  if (studio?.connected) return raw || "…";
  return "en attente";
}

const NAV: { id: View; label: string }[] = [
  { id: "projects", label: "Projets" },
  { id: "studio", label: "Studio" },
  { id: "workshop", label: "Atelier" },
  { id: "bank", label: "Banque" },
  { id: "settings", label: "Réglages" },
];

const EMPTY_KEYS: Keys = {
  gemini: "",
  meshy: "",
  tripo: "",
  cursor: "",
  meshProvider: "meshy",
  robloxApiKey: "",
  robloxUserId: "",
  robloxOauthClientId: "",
  blenderPath: "",
};

export default function App() {
  const [view, setView] = useState<View>("projects");
  const [projects, setProjects] = useState<Project[]>([]);
  const [current, setCurrent] = useState<Project | null>(null);
  const [agents, setAgents] = useState<AgentStatus[]>([]);
  const [keys, setKeys] = useState<Keys>(EMPTY_KEYS);
  const [robloxUser, setRobloxUser] = useState<RobloxUser | null>(null);
  const [authReady, setAuthReady] = useState(false);
  const [error, setError] = useState("");
  const [theme, setTheme] = useState<Theme>(() => readTheme());
  const [bankSeen, setBankSeen] = useState(false);
  const [appUpdate, setAppUpdate] = useState<Update | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [updateMsg, setUpdateMsg] = useState("");

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  useEffect(() => {
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void check()
        .then((update) => {
          if (!cancelled && update) setAppUpdate(update);
        })
        .catch(() => undefined);
    }, 2500);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, []);

  async function installAppUpdate() {
    if (!appUpdate || updateBusy) return;
    setUpdateBusy(true);
    setUpdateMsg("Téléchargement…");
    try {
      await appUpdate.downloadAndInstall();
      setUpdateMsg("Redémarrage…");
      await relaunch();
    } catch (err) {
      setUpdateBusy(false);
      setUpdateMsg(String(err));
    }
  }

  async function refresh() {
    try {
      const [list, detected, stored, user] = await Promise.all([
        invoke<Project[]>("list_projects"),
        invoke<AgentStatus[]>("detect_agents"),
        invoke<Keys>("get_keys"),
        invoke<RobloxUser | null>("get_roblox_user"),
      ]);
      setProjects(list);
      setAgents(detected);
      setKeys({ ...EMPTY_KEYS, ...stored });
      setRobloxUser(user);
      setCurrent((prev) => prev ?? list[0] ?? null);
    } catch (err) {
      setError(String(err));
    } finally {
      setAuthReady(true);
    }
  }

  useEffect(() => {
    void refresh();
    const id = window.setInterval(() => {
      void invoke<AgentStatus[]>("detect_agents")
        .then(setAgents)
        .catch(() => {});
    }, 8000);
    const onFocus = () => {
      void invoke<Project[]>("list_projects")
        .then(setProjects)
        .catch(() => {});
    };
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearInterval(id);
      window.removeEventListener("focus", onFocus);
    };
  }, []);

  useEffect(() => {
    if (view !== "projects") return;
    void invoke<Project[]>("list_projects")
      .then(setProjects)
      .catch(() => {});
  }, [view]);

  useEffect(() => {
    if (view === "bank") setBankSeen(true);
  }, [view]);

  if (!authReady) {
    return (
      <div className="login-screen">
        <div className="login-card">
          <strong>Lumen</strong>
          <p>Chargement…</p>
        </div>
      </div>
    );
  }

  if (!robloxUser) {
    return (
      <Login
        keys={keys}
        error={error}
        theme={theme}
        onToggleTheme={() => setTheme((t) => toggleTheme(t))}
        onKeys={setKeys}
        onLoggedIn={async (user) => {
          setRobloxUser(user);
          await refresh();
        }}
      />
    );
  }

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <strong>Lumen</strong>
          <span>Jeux Roblox, à partir d’une phrase</span>
        </div>
        <nav>
          {NAV.map((item) => (
            <button
              key={item.id}
              className={view === item.id ? "active" : ""}
              onClick={() => setView(item.id)}
            >
              {item.label}
            </button>
          ))}
        </nav>
        <div className="sidebar-foot">
          {current ? `Projet : ${current.name}` : "Aucun projet ouvert"}
          <br />
          Agents : {agents.filter((a) => a.found).length}/3 détectés
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <button
            className="theme-toggle"
            type="button"
            onClick={() => setTheme((t) => toggleTheme(t))}
          >
            {theme === "dark" ? "Mode clair" : "Mode sombre"}
          </button>
          <AccountMenu
            user={robloxUser}
            onSettings={() => setView("settings")}
            onLogout={async () => {
              await invoke("logout_roblox");
              setRobloxUser(null);
            }}
          />
        </header>
        <section className={`main ${view === "studio" ? "main-fill" : ""}`}>
        {error ? <p className="err">{error}</p> : null}
        {appUpdate ? (
          <div className="update-banner">
            <span>
              Lumen {appUpdate.version} est disponible.
              {updateMsg ? ` ${updateMsg}` : ""}
            </span>
            <button
              className="btn copper"
              type="button"
              disabled={updateBusy}
              onClick={() => void installAppUpdate()}
            >
              {updateBusy ? "Mise à jour…" : "Installer"}
            </button>
          </div>
        ) : null}
        {view === "projects" && (
          <Projects
            projects={projects}
            current={current}
            onOpen={(project) => {
              setCurrent(project);
              setView("studio");
            }}
            onCreated={async (project) => {
              await refresh();
              setCurrent(project);
              setView("studio");
            }}
          />
        )}
        {view === "studio" && (
          <Studio
            key={current?.path ?? "none"}
            project={current}
            projects={projects}
            agents={agents}
            onNeedProject={() => setView("projects")}
            onProjectChange={setCurrent}
          />
        )}
        {view === "workshop" && <Workshop project={current} keys={keys} />}
        {bankSeen ? (
          <div hidden={view !== "bank"}>
            <Bank />
          </div>
        ) : null}
        {view === "settings" && (
          <Settings
            keys={keys}
            agents={agents}
            onSaved={refresh}
            theme={theme}
            onToggleTheme={() => setTheme((t) => toggleTheme(t))}
            appUpdate={appUpdate}
            updateBusy={updateBusy}
            updateMsg={updateMsg}
            onCheckUpdate={async () => {
              setUpdateMsg("");
              try {
                const update = await check();
                setAppUpdate(update);
                setUpdateMsg(update ? `Version ${update.version} disponible` : "Déjà à jour");
              } catch (err) {
                setUpdateMsg(String(err));
              }
            }}
            onInstallUpdate={() => void installAppUpdate()}
          />
        )}
        </section>
      </div>
      <AssetReviewHost />
    </div>
  );
}

function AccountMenu({
  user,
  onSettings,
  onLogout,
}: {
  user: RobloxUser;
  onSettings: () => void;
  onLogout: () => Promise<void>;
}) {
  const [openMenu, setOpenMenu] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function onDoc(event: MouseEvent) {
      if (!wrapRef.current?.contains(event.target as Node)) setOpenMenu(false);
    }
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, []);

  const profile = user.profile || `https://www.roblox.com/users/${user.id}/profile`;

  return (
    <div className="account-wrap" ref={wrapRef}>
      <button className="account-chip" onClick={() => setOpenMenu((v) => !v)} aria-label="Compte Roblox">
        {user.picture ? (
          <img src={user.picture} alt="" />
        ) : (
          <span>{(user.username || "?").slice(0, 1).toUpperCase()}</span>
        )}
      </button>
      {openMenu ? (
        <div className="account-menu">
          <div className="account-head">
            {user.picture ? <img src={user.picture} alt="" /> : <span className="account-fallback">{(user.username || "?").slice(0, 1).toUpperCase()}</span>}
            <div>
              <strong>{user.displayName || user.username}</strong>
              <button
                className="account-link"
                onClick={() => void openUrl(profile)}
              >
                {user.username}
              </button>
            </div>
          </div>
          <button
            onClick={() => {
              setOpenMenu(false);
              onSettings();
            }}
          >
            Paramètres
          </button>
          <button
            className="danger"
            onClick={async () => {
              setOpenMenu(false);
              await onLogout();
            }}
          >
            Déconnexion
          </button>
        </div>
      ) : null}
    </div>
  );
}

function Login({
  keys,
  error,
  theme,
  onToggleTheme,
  onKeys,
  onLoggedIn,
}: {
  keys: Keys;
  error: string;
  theme: Theme;
  onToggleTheme: () => void;
  onKeys: (keys: Keys) => void;
  onLoggedIn: (user: RobloxUser) => Promise<void>;
}) {
  const [clientId, setClientId] = useState(keys.robloxOauthClientId);
  const [showClient, setShowClient] = useState(!keys.robloxOauthClientId.trim());
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState(error);

  async function saveClientId() {
    const next = { ...keys, robloxOauthClientId: clientId.trim() };
    await invoke("set_keys", { keys: next });
    onKeys(next);
    return next;
  }

  return (
    <div className="login-screen">
      <div className="login-card">
        <strong>Lumen</strong>
        <h1>Connecte-toi avec Roblox.</h1>
        <p>
          Lumen ouvre la page officielle Roblox. Ton mot de passe reste chez eux — on ne
          conserve qu’un jeton OAuth.
        </p>
        {showClient ? (
          <>
            <label>
              Client ID OAuth (une seule fois)
              <input
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                placeholder="Créé dans le Creator Dashboard"
              />
            </label>
            <small>
              App OAuth <strong>privée, non publiée</strong> — ignore « Réviser et publier ».
              Catégorie Creation &amp; Productivity. Permissions : <code>openid</code>,{" "}
              <code>profile</code>, <code>asset:read</code>, <code>asset:write</code>. Redirect
              URI exact : <code>{OAUTH_REDIRECT}</code>.
            </small>
            <div className="row">
              <button className="btn secondary" type="button" onClick={() => void openUrl(OAUTH_APPS)}>
                Ouvrir le Dashboard
              </button>
              <button
                className="btn secondary"
                type="button"
                onClick={() => void navigator.clipboard.writeText(OAUTH_REDIRECT)}
              >
                Copier l’URI
              </button>
            </div>
          </>
        ) : (
          <button className="text-link" type="button" onClick={() => setShowClient(true)}>
            Modifier le Client ID
          </button>
        )}
        <button
          className="btn copper"
          disabled={busy || !clientId.trim()}
          onClick={async () => {
            setBusy(true);
            setErr("");
            try {
              await saveClientId();
              const user = await invoke<RobloxUser>("start_roblox_login");
              await onLoggedIn(user);
            } catch (error) {
              setErr(String(error));
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? "En attente de Roblox…" : "Continuer avec Roblox"}
        </button>
        {err ? <p className="err">{err}</p> : null}
        <button className="text-link" type="button" onClick={onToggleTheme}>
          {theme === "dark" ? "Passer en mode clair" : "Passer en mode sombre"}
        </button>
      </div>
    </div>
  );
}

function displayPath(path: string) {
  const sep = path.includes("\\") ? "\\" : "/";
  const parts = path.split(/[/\\]/).filter(Boolean);
  if (parts.length <= 4) return path;
  return `…${sep}${parts.slice(-3).join(sep)}`;
}

function Projects({
  projects,
  current,
  onOpen,
  onCreated,
}: {
  projects: Project[];
  current: Project | null;
  onOpen: (project: Project) => void;
  onCreated: (project: Project) => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  return (
    <div className="hero">
      <h1>Tes mondes.</h1>
      <p className="lede">
        Un projet Lumen, c’est un jeu Roblox en TypeScript, prêt pour Claude Code, Codex et Cursor.
        Pas de VM, pas de terminal à configurer : les agents tournent dans l’app.
      </p>
      <div className="toolbar">
        <input
          type="text"
          placeholder="Nom du jeu — ex. Seasonal Tycoon"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <button
          className="btn copper"
          disabled={busy || !name.trim()}
          onClick={async () => {
            setBusy(true);
            setErr("");
            try {
              const project = await invoke<Project>("create_project", { name: name.trim() });
              setName("");
              await onCreated(project);
            } catch (error) {
              setErr(String(error));
            } finally {
              setBusy(false);
            }
          }}
        >
          Nouveau projet
        </button>
      </div>
      {err ? <p className="err">{err}</p> : null}
      {projects.length === 0 ? (
        <div className="empty">Aucun projet pour l’instant. Donne un nom, Lumen prépare le squelette Rojo.</div>
      ) : (
        <div className="grid">
          {projects.map((project) => (
            <button
              key={project.path}
              className={`card ${current?.path === project.path ? "active" : ""}`}
              onClick={() => onOpen(project)}
            >
              <h2 style={{ fontSize: 22 }}>{project.name}</h2>
              <small className="card-path" title={project.path}>
                {displayPath(project.path)}
              </small>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

type BootStep = {
  id: string;
  label: string;
  detail?: string;
  status: "wait" | "run" | "ok" | "warn" | "err";
};

function bootStepsTemplate(): BootStep[] {
  return [
    { id: "node", label: "Node.js", status: "wait" },
    { id: "rojo", label: "Binaire Rojo", status: "wait" },
    { id: "plugin", label: "Plugin LumenSync", status: "wait" },
    {
      id: "compile",
      label: "Préparation du projet",
      detail: "Dépendances et compilation Luau",
      status: "wait",
    },
    { id: "serve", label: "Serveur Rojo", status: "wait" },
    {
      id: "studio",
      label: "Connexion Studio",
      detail: "Sélection de la bonne fenêtre Studio.",
      status: "wait",
    },
  ];
}

function Studio({
  project,
  projects,
  agents,
  onNeedProject,
  onProjectChange,
}: {
  project: Project | null;
  projects: Project[];
  agents: AgentStatus[];
  onNeedProject: () => void;
  onProjectChange: (project: Project) => void;
}) {
  const [tabs, setTabs] = useState<AgentTab[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);
  const [prompt, setPrompt] = useState("");
  const [shots, setShots] = useState<PastedImage[]>([]);
  const [busy, setBusy] = useState(false);
  const [swarmBusy, setSwarmBusy] = useState(false);
  const [rojo, setRojo] = useState<RojoStatus | null>(null);
  const [compiler, setCompiler] = useState<CompilerStatus | null>(null);
  const [studio, setStudio] = useState<StudioHeartbeat | null>(null);
  const [toolchain, setToolchain] = useState<ToolchainStatus | null>(null);
  const [offer, setOffer] = useState<PlaceOffer | null>(null);
  const [syncErr, setSyncErr] = useState("");
  const [booting, setBooting] = useState(true);
  const [bootSteps, setBootSteps] = useState<BootStep[]>(bootStepsTemplate);
  const [placeWarn, setPlaceWarn] = useState<{ open: string; bound: string } | null>(null);
  const swarmReady = useRef(false);
  const tabsRef = useRef<AgentTab[]>([]);

  const currentTab = tabs.find((tab) => tab.localId === active) ?? null;
  const runningCount = tabs.filter((tab) => tab.running).length;
  const canResume = tabs.some((tab) => !tab.running && !tab.error);

  useEffect(() => {
    tabsRef.current = tabs;
  }, [tabs]);

  useEffect(() => {
    const unlistenChunk = listen<{ sessionId: string; data: string }>("agent-chunk", (event) => {
      ptyBus.push(event.payload.sessionId, event.payload.data);
    });
    const unlistenExit = listen<{ sessionId: string }>("agent-exit", (event) => {
      setTabs((prev) =>
        prev.map((tab) =>
          tab.sessionId === event.payload.sessionId ? { ...tab, running: false } : tab,
        ),
      );
    });
    const unlistenResume = listen<{ sessionId: string; localId: string; resumeId: string }>(
      "agent-resume-id",
      (event) => {
        setTabs((prev) =>
          prev.map((tab) =>
            tab.localId === event.payload.localId || tab.sessionId === event.payload.sessionId
              ? { ...tab, resumeId: event.payload.resumeId }
              : tab,
          ),
        );
      },
    );
    return () => {
      void unlistenChunk.then((fn) => fn());
      void unlistenExit.then((fn) => fn());
      void unlistenResume.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    setShots((prev) => {
      for (const shot of prev) URL.revokeObjectURL(shot.preview);
      return [];
    });
  }, [project?.path]);

  useEffect(() => {
    swarmReady.current = false;
    if (!project) {
      setTabs([]);
      setPaused(false);
      return;
    }
    const projectPath = project.path;
    let cancel = false;
    async function restore() {
      try {
        const [swarm, live] = await Promise.all([
          invoke<SwarmState>("load_swarm", { projectPath }),
          invoke<LiveAgent[]>("live_agents", { projectPath }),
        ]);
        if (cancel) return;
        const restored: AgentTab[] = (swarm.agents ?? []).map((agent) => {
          const liveMatch = live.find((item) => item.localId === agent.localId);
          return {
            localId: agent.localId,
            kind: agent.kind,
            title: agent.title,
            resumeId: liveMatch?.resumeId || agent.resumeId || null,
            sessionId: liveMatch?.sessionId ?? null,
            running: Boolean(liveMatch),
          };
        });
        setTabs(restored);
        setActive(restored[0]?.localId ?? null);
        setPaused(Boolean(swarm.paused) && live.length === 0);
        swarmReady.current = true;
      } catch {
        swarmReady.current = true;
      }
    }
    void restore();
    return () => {
      cancel = true;
    };
  }, [project?.path]);

  useEffect(() => {
    if (!project || !swarmReady.current) return;
    const swarm: SwarmState = {
      paused,
      agents: tabs.map((tab) => ({
        localId: tab.localId,
        kind: tab.kind,
        title: tab.title,
        resumeId: tab.resumeId,
      })),
    };
    void invoke("save_swarm", { projectPath: project.path, swarm });
  }, [project?.path, tabs, paused]);

  useEffect(() => {
    let cancel = false;
    async function tick() {
      try {
        const [r, c, s, t, o] = await Promise.all([
          invoke<RojoStatus>("rojo_status"),
          invoke<CompilerStatus>("compiler_status"),
          invoke<StudioHeartbeat>("studio_status"),
          invoke<ToolchainStatus>("toolchain_status"),
          invoke<PlaceOffer>("sync_offer"),
        ]);
        if (!cancel) {
          setRojo(r);
          setCompiler(c);
          setStudio(s);
          setToolchain(t);
          setOffer(o);
        }
      } catch {
        /* ignore */
      }
    }
    void tick();
    const id = setInterval(() => void tick(), 2000);
    const unlisten = listen<StudioHeartbeat>("studio-heartbeat", (event) => {
      setStudio(event.payload);
    });
    return () => {
      cancel = true;
      clearInterval(id);
      void unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (!project) {
      setBooting(false);
      return;
    }
    let cancel = false;
    const projectPath = project.path;

    function patch(id: string, next: Partial<BootStep>) {
      setBootSteps((prev) => prev.map((step) => (step.id === id ? { ...step, ...next } : step)));
    }

    async function boot() {
      setBooting(true);
      setSyncErr("");
      setPlaceWarn(null);
      setBootSteps(bootStepsTemplate());
      try {
        const [rojoNow, compilerNow, tools] = await Promise.all([
          invoke<RojoStatus>("rojo_status"),
          invoke<CompilerStatus>("compiler_status"),
          invoke<ToolchainStatus>("toolchain_status"),
        ]);
        if (cancel) return;
        setToolchain(tools);
        const already =
          rojoNow.reachable &&
          rojoNow.projectPath === projectPath &&
          Boolean(compilerNow.watching);
        if (already) {
          setBooting(false);
          return;
        }

        patch("node", { status: "run" });
        if (!tools.node || !tools.npm) {
          patch("node", {
            status: "err",
            detail: "Installe Node.js LTS, puis relance Lumen.",
          });
          throw new Error("Node.js / npm introuvable. Installe Node LTS, puis relance Lumen.");
        }
        patch("node", { status: "ok", detail: "détecté" });

        patch("rojo", { status: "run" });
        if (!rojoNow.found) {
          patch("rojo", { detail: "Installation…" });
          await invoke("install_rojo");
        }
        if (cancel) return;
        patch("rojo", { status: "ok" });

        patch("plugin", { status: "run" });
        await invoke("install_studio_plugin");
        if (cancel) return;
        patch("plugin", { status: "ok" });

        patch("compile", { status: "run", detail: "npm + rbxtsc…" });
        await invoke("start_sync", { projectPath });
        if (cancel) return;
        patch("compile", { status: "ok" });

        const live = await invoke<RojoStatus>("rojo_status");
        if (cancel) return;
        setRojo(live);
        if (!live.reachable) {
          patch("serve", { status: "err", detail: "Rojo n’écoute pas." });
          throw new Error("Rojo n’écoute pas. Réessaie la sync.");
        }
        patch("serve", { status: "ok", detail: `127.0.0.1:${live.port}` });

        patch("studio", { status: "run" });
        let connected: StudioHeartbeat | null = null;
        for (let i = 0; i < 8; i += 1) {
          connected = await invoke<StudioHeartbeat>("studio_status");
          const offerNow = await invoke<PlaceOffer>("sync_offer");
          if (cancel) return;
          setStudio(connected);
          setOffer(offerNow);
          if (connected.connected) break;
          await new Promise((resolve) => window.setTimeout(resolve, 800));
        }
        if (cancel) return;
        if (!connected?.connected) {
          patch("studio", {
            status: "warn",
            detail: "Ouvre Studio — ou continue sans attendre.",
          });
          return;
        }
        const offerNow = await invoke<PlaceOffer>("sync_offer");
        const boundName = offerNow.boundPlaceName;
        const sameId =
          offerNow.boundPlaceId !== 0 && connected.placeId === offerNow.boundPlaceId;
        const unknownGeneric =
          isGenericPlaceName(connected.placeName) &&
          (connected.placeId === 0 || sameId);
        if (
          boundName &&
          boundName !== connected.placeName &&
          !sameId &&
          !unknownGeneric
        ) {
          patch("studio", {
            status: "warn",
            detail: `Place ouverte : ${studioPlaceLabel(connected, offerNow)}`,
          });
          setPlaceWarn({ open: studioPlaceLabel(connected, offerNow), bound: boundName });
          return;
        }
        if (!connected.bound && !sameId) {
          patch("studio", {
            status: "warn",
            detail: `Studio : ${studioPlaceLabel(connected, offerNow)}`,
          });
          return;
        }
        patch("studio", { status: "ok", detail: studioPlaceLabel(connected, offerNow) });
        setBooting(false);
      } catch (err) {
        if (!cancel) setSyncErr(String(err));
      }
    }

    void boot();
    return () => {
      cancel = true;
    };
  }, [project?.path]);

  async function startTab(
    kind: AgentKind,
    localId: string,
    resumeId: string | null,
    resuming = false,
  ) {
    if (!project) return;
    setSwarmBusy(true);
    setPaused(false);
    try {
      const started = await invoke<StartedAgent>("start_agent", {
        kind,
        projectPath: project.path,
        resumeId,
        localId,
        resume: resuming,
      });
      setTabs((prev) =>
        prev.map((item) =>
          item.localId === localId
            ? {
                ...item,
                sessionId: started.sessionId,
                resumeId: started.resumeId || item.resumeId,
                running: true,
                error: undefined,
              }
            : item,
        ),
      );
    } catch (error) {
      setTabs((prev) =>
        prev.map((item) =>
          item.localId === localId
            ? { ...item, error: String(error), running: false, sessionId: null }
            : item,
        ),
      );
    } finally {
      setSwarmBusy(false);
    }
  }

  async function addAgent(kind: AgentKind) {
    if (!project) return onNeedProject();
    const status = agents.find((agent) => agent.id === kind);
    const localId = crypto.randomUUID();
    const tab: AgentTab = {
      localId,
      kind,
      sessionId: null,
      resumeId: null,
      title: labelFor(kind),
      error: status?.found ? undefined : `${labelFor(kind)} n’est pas installé. Va dans Réglages.`,
      running: Boolean(status?.found),
    };
    setTabs((prev) => [...prev, tab]);
    setActive(localId);
    if (!status?.found) return;
    await startTab(kind, localId, null, false);
  }

  async function pauseSwarm() {
    if (!project) return;
    setSwarmBusy(true);
    try {
      await invoke("pause_project", { projectPath: project.path });
      setTabs((prev) => prev.map((tab) => ({ ...tab, sessionId: null, running: false })));
      setPaused(true);
    } finally {
      setSwarmBusy(false);
    }
  }

  async function resumeSwarm() {
    const stopped = tabsRef.current.filter((tab) => !tab.running && !tab.error);
    for (const tab of stopped) {
      await startTab(tab.kind, tab.localId, tab.resumeId, true);
    }
  }

  async function resumeTab(tab: AgentTab) {
    await startTab(tab.kind, tab.localId, tab.resumeId, true);
  }

  async function pauseTab(tab: AgentTab) {
    if (tab.sessionId) {
      try {
        await invoke("stop_agent", { sessionId: tab.sessionId });
      } catch {
        /* already gone */
      }
    }
    setTabs((prev) => {
      const next = prev.map((item) =>
        item.localId === tab.localId ? { ...item, sessionId: null, running: false } : item,
      );
      if (next.every((item) => !item.running)) setPaused(true);
      return next;
    });
  }

  async function attachImages(files: File[]) {
    if (!project || !files.length) return;
    try {
      const saved = await savePastedFiles(project.path, files);
      setShots((prev) => [...prev, ...saved]);
    } catch (error) {
      setSyncErr(String(error));
    }
  }

  async function send() {
    if (!currentTab?.sessionId) return;
    const body = prompt.trim();
    if (!body && !shots.length) return;
    const header = shots.length ? `${imageBrief(shots)}\n\n` : "";
    const merged = `${header}${body}`;
    const text = merged.endsWith("\r") || merged.endsWith("\n") ? merged : `${merged}\r`;
    setPrompt("");
    setShots((prev) => {
      for (const shot of prev) URL.revokeObjectURL(shot.preview);
      return [];
    });
    await invoke("write_agent", { sessionId: currentTab.sessionId, data: text });
  }

  async function closeTab(tab: AgentTab) {
    if (tab.sessionId) {
      try {
        await invoke("stop_agent", { sessionId: tab.sessionId });
      } catch {
        /* already gone */
      }
    }
    setTabs((prev) => {
      const next = prev.filter((item) => item.localId !== tab.localId);
      if (active === tab.localId) setActive(next[next.length - 1]?.localId ?? null);
      return next;
    });
  }

  if (!project) {
    return (
      <div className="empty">
        Ouvre ou crée un projet d’abord.
        <div style={{ marginTop: 12 }}>
          <button className="btn" onClick={onNeedProject}>
            Aller aux projets
          </button>
        </div>
      </div>
    );
  }

  const bootDone = bootSteps.filter(
    (step) => step.status === "ok" || step.status === "warn" || step.status === "err",
  ).length;
  const canSkip = bootSteps.some((step) => step.id === "serve" && step.status === "ok") || Boolean(syncErr);
  const showPlaceChoice =
    studio?.connected &&
    (!studio.bound || Boolean(placeWarn));

  return (
    <div className="studio">
      {booting ? (
        <div className="boot-screen">
          <div className="boot-card">
            <div className="boot-kicker">Roblox Studio</div>
            <h2>{project.name}</h2>
            <p className="boot-progress-label">
              Mise en route du projet…
              <span>
                {bootDone}/{bootSteps.length}
              </span>
            </p>
            <div className="boot-bar">
              <i style={{ width: `${Math.round((bootDone / bootSteps.length) * 100)}%` }} />
            </div>
            {showPlaceChoice ? (
              <div className="boot-warn">
                <strong>
                  {placeWarn ? "Une autre place est ouverte" : "Lier cette place ?"}
                </strong>
                <p>
                  {placeWarn
                    ? `Ouvert : ${placeWarn.open} · lié : ${placeWarn.bound}`
                    : `Studio affiche « ${studioPlaceLabel(studio, offer)} ».`}
                </p>
                <button
                  className="btn copper"
                  type="button"
                  onClick={async () => {
                    try {
                      await invoke("bind_open_place");
                      setPlaceWarn(null);
                      setBooting(false);
                    } catch (err) {
                      setSyncErr(String(err));
                    }
                  }}
                >
                  Utiliser cette place
                </button>
              </div>
            ) : null}
            <ul className="boot-steps">
              {bootSteps.map((step) => (
                <li key={step.id} className={step.status}>
                  <span className="boot-dot" />
                  <div>
                    <strong>{step.label}</strong>
                    {step.detail ? <small>{step.detail}</small> : null}
                  </div>
                </li>
              ))}
            </ul>
            {syncErr ? <p className="err">{syncErr}</p> : null}
            <div className="boot-actions">
              <button className="text-link" type="button" onClick={onNeedProject}>
                Retour aux projets
              </button>
              {canSkip ? (
                <button className="btn secondary" type="button" onClick={() => setBooting(false)}>
                  Continuer sans attendre
                </button>
              ) : null}
            </div>
          </div>
        </div>
      ) : null}
      <div className="studio-head">
        <h1>{project.name}</h1>
        <div className="syncbar">
        <span className={`pill ${toolchain?.node && toolchain.npm ? "ok" : "no"}`}>
          Node {toolchain?.node && toolchain.npm ? "ok" : "manquant"}
        </span>
        <span
          className={`pill ${compiler?.ready && compiler.watching ? "ok" : compiler?.watching ? "watch" : "no"}`}
        >
          Luau{" "}
          {compiler?.ready && compiler.watching
            ? "watch"
            : compiler?.watching
              ? "compile…"
              : compiler?.lastError
                ? "erreur"
                : "arrêté"}
        </span>
        <span className={`pill ${rojo?.reachable ? "ok" : "no"}`}>
          Rojo {rojo?.reachable ? `live :${rojo.port}` : rojo?.found ? "arrêté" : "absent"}
        </span>
        <span className={`pill ${studio?.connected ? "ok" : "no"}`}>
          Studio {studioPlaceLabel(studio, offer)}
        </span>
        <span className={`pill ${studio?.pluginInstalled ? "ok" : "no"}`}>
          Plugin {studio?.pluginInstalled ? "installé" : "manquant"}
        </span>
        {rojo?.reachable ? null : (
        <button
          className="btn copper"
          disabled={!project || busy || booting}
          onClick={async () => {
            setBusy(true);
            setSyncErr("");
            try {
              if (!toolchain?.node || !toolchain.npm) {
                throw new Error("Node.js / npm introuvable. Installe Node LTS, puis relance Lumen.");
              }
              if (!rojo?.found) await invoke("install_rojo");
              await invoke("install_studio_plugin");
              await invoke("start_sync", { projectPath: project.path });
            } catch (err) {
              setSyncErr(String(err));
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? "Compilation…" : "Relancer la sync"}
        </button>
        )}
        {rojo?.serving || compiler?.watching ? (
          <button className="btn secondary" onClick={() => void invoke("stop_sync")}>
            Stop sync
          </button>
        ) : null}
        <button
          className="btn secondary"
          onClick={() => void invoke("open_project_dir", { path: project.path })}
        >
          Dossier
        </button>
        <div className="ref-pick">
          <span>S’inspirer de</span>
          {(project.referencePaths?.length ? project.referencePaths : project.referencePath ? [project.referencePath] : [])
            .map((path) => {
              const item = projects.find((p) => p.path === path);
              return (
                <span className="ref-chip" key={path}>
                  {item?.name ?? path}
                  <button
                    type="button"
                    aria-label="Retirer"
                    onClick={async () => {
                      const current = project.referencePaths?.length
                        ? project.referencePaths
                        : project.referencePath
                          ? [project.referencePath]
                          : [];
                      try {
                        const next = await invoke<Project>("set_reference_projects", {
                          projectPath: project.path,
                          referencePaths: current.filter((p) => p !== path),
                        });
                        onProjectChange(next);
                      } catch (err) {
                        setSyncErr(String(err));
                      }
                    }}
                  >
                    ×
                  </button>
                </span>
              );
            })}
          <select
            value=""
            disabled={
              projects.filter(
                (item) =>
                  item.path !== project.path &&
                  !(project.referencePaths ?? []).includes(item.path) &&
                  project.referencePath !== item.path,
              ).length === 0
            }
            onChange={async (event) => {
              const added = event.target.value;
              if (!added) return;
              const current = project.referencePaths?.length
                ? project.referencePaths
                : project.referencePath
                  ? [project.referencePath]
                  : [];
              try {
                const next = await invoke<Project>("set_reference_projects", {
                  projectPath: project.path,
                  referencePaths: [...current, added],
                });
                onProjectChange(next);
              } catch (err) {
                setSyncErr(String(err));
              }
            }}
          >
            <option value="">+ Projet</option>
            {projects
              .filter(
                (item) =>
                  item.path !== project.path &&
                  !(project.referencePaths ?? []).includes(item.path) &&
                  project.referencePath !== item.path,
              )
              .map((item) => (
                <option key={item.path} value={item.path}>
                  {item.name}
                </option>
              ))}
          </select>
        </div>
        {runningCount > 0 ? (
          <button className="btn pause-swarm" disabled={swarmBusy} onClick={() => void pauseSwarm()}>
            <span aria-hidden="true">❚❚</span>
            Pause swarm
          </button>
        ) : null}
        {tabs.length > 0 && canResume ? (
          <button className="btn copper" disabled={swarmBusy} onClick={() => void resumeSwarm()}>
            Reprendre
          </button>
        ) : null}
        {(["claude", "codex", "cursor", "antigravity"] as AgentKind[]).map((kind) => {
          const found = agents.find((agent) => agent.id === kind)?.found;
          return (
            <button
              key={kind}
              className="btn secondary"
              disabled={swarmBusy}
              onClick={() => void addAgent(kind)}
            >
              + {labelFor(kind)} {found ? "" : "(à installer)"}
            </button>
          );
        })}
      </div>
      </div>
      {syncErr ? <p className="sync-error">{syncErr}</p> : null}
      {rojo?.serving ? (
        <div className="place-banner compact">
          {studio?.connected ? (
            studio.bound ||
            (offer?.boundPlaceId !== 0 && offer?.boundPlaceId === studio.placeId) ||
            offer?.boundPlaceName === studio.placeName ||
            (isGenericPlaceName(studio.placeName) && Boolean(offer?.boundPlaceName)) ? (
              <span>
                Place liée : <strong>{studioPlaceLabel(studio, offer)}</strong>
                {" · "}
                plugin Rojo → <strong>127.0.0.1:{rojo.port}</strong>
                {rojo.port !== 34872 ? " (pas 34872, c’est VibeStarter)" : ""}
              </span>
            ) : (
              <span>
                Studio : <strong>{studioPlaceLabel(studio, offer)}</strong> — lie cette place au projet, puis
                connecte Rojo sur le port <strong>{rojo.port}</strong>.
              </span>
            )
          ) : (
            <span>
              Plugin Rojo : hôte <strong>127.0.0.1</strong>, port <strong>{rojo.port}</strong>
              {" — pas 34872 (VibeStarter). "}
              Ouvre Studio pour lier <strong>{project.name}</strong>
            </span>
          )}
          <button className="btn secondary" onClick={() => void invoke("remind_studio")}>
            Rappeler
          </button>
        </div>
      ) : null}
      {paused && tabs.length > 0 ? (
        <div className="swarm-banner">
          Swarm en pause — les conversations sont conservées. Tu peux fermer Lumen et les
          reprendre ici.
          <button className="btn copper" disabled={swarmBusy} onClick={() => void resumeSwarm()}>
            Reprendre
          </button>
        </div>
      ) : null}
      {tabs.length === 0 ? (
        <div className="agent-empty">
          <strong>Aucun terminal pour l’instant</strong>
          <p>Ajoute Claude Code, Codex ou Cursor. Un vrai TUI s’ouvre dans ce panneau.</p>
        </div>
      ) : (
        <div
          className={`agent-grid cols-${tabs.length <= 1 ? 1 : tabs.length <= 4 ? 2 : 3}`}
        >
          {tabs.map((tab) => (
            <div
              key={tab.localId}
              className={`agent-pane ${tab.localId === active ? "on" : ""}`}
              onClick={() => setActive(tab.localId)}
            >
              <div className="agent-pane-head">
                <span className={tab.running ? "live" : paused || !tab.running ? "paused" : ""}>
                  {tab.running ? "●" : "❚❚"}
                </span>
                {tab.title}
                <div className="pane-actions">
                  {tab.running ? (
                    <button
                      type="button"
                      title="Mettre en pause"
                      onClick={(event) => {
                        event.stopPropagation();
                        void pauseTab(tab);
                      }}
                    >
                      Pause
                    </button>
                  ) : (
                    <button
                      type="button"
                      title="Reprendre la conversation"
                      onClick={(event) => {
                        event.stopPropagation();
                        void resumeTab(tab);
                      }}
                    >
                      Reprendre
                    </button>
                  )}
                  <button
                    type="button"
                    title="Retirer du swarm"
                    onClick={(event) => {
                      event.stopPropagation();
                      void closeTab(tab);
                    }}
                  >
                    Fermer
                  </button>
                </div>
              </div>
              {tab.sessionId ? (
                <AgentTerminal
                  sessionId={tab.sessionId}
                  projectPath={project.path}
                  active={tab.localId === active}
                />
              ) : (
                <div className="agent-boot">
                  {tab.error ||
                    (tab.running
                      ? `Ouverture de ${tab.title}…`
                      : "En pause. La conversation reprendra au même endroit.")}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
      <div
        className="prompt"
        onDragOver={(event) => {
          if (imageFilesFromTransfer(event.dataTransfer).length) event.preventDefault();
        }}
        onDrop={(event) => {
          const files = imageFilesFromTransfer(event.dataTransfer);
          if (!files.length) return;
          event.preventDefault();
          void attachImages(files);
        }}
      >
        {shots.length ? (
          <div className="prompt-shots">
            {shots.map((shot) => (
              <button
                key={shot.path}
                type="button"
                className="prompt-shot"
                title="Retirer"
                onClick={() => {
                  URL.revokeObjectURL(shot.preview);
                  setShots((prev) => prev.filter((item) => item.path !== shot.path));
                }}
              >
                <img src={shot.preview} alt="" />
              </button>
            ))}
          </div>
        ) : null}
        <div className="prompt-row">
          <textarea
            value={prompt}
            placeholder="Brief vers le terminal actif (Ctrl+Entrée) — colle une capture ici"
            onChange={(e) => setPrompt(e.target.value)}
            onPaste={(event) => {
              const files = imageFilesFromTransfer(event.clipboardData);
              if (!files.length) return;
              event.preventDefault();
              void attachImages(files);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void send();
            }}
          />
          <button
            className="btn copper"
            disabled={!currentTab?.sessionId || (!prompt.trim() && !shots.length)}
            onClick={() => void send()}
          >
            Envoyer
          </button>
        </div>
      </div>
    </div>
  );
}

function Workshop({ project, keys }: { project: Project | null; keys: Keys }) {
  const [imagePrompt, setImagePrompt] = useState("Icône d’un tycoon saisonnier, style Roblox, PNG fond transparent");
  const [meshPrompt, setMeshPrompt] = useState("Coffre au trésor stylisé, low poly, pour Roblox");
  const [blenderScript, setBlenderScript] = useState(
    `import bpy

for obj in list(bpy.data.objects):
    bpy.data.objects.remove(obj, do_unlink=True)

mat = bpy.data.materials.new("Wood")
mat.use_nodes = True
bsdf = mat.node_tree.nodes.get("Principled BSDF")
bsdf.inputs["Base Color"].default_value = (0.55, 0.32, 0.14, 1)
bsdf.inputs["Roughness"].default_value = 0.7

bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, 0, 0.5))
box = bpy.context.active_object
box.name = "Crate"
box.data.materials.append(mat)
`,
  );
  const [image, setImage] = useState<string | null>(null);
  const [mesh, setMesh] = useState<string>("");
  const [meshPath, setMeshPath] = useState<string | null>(null);
  const [meshPreview, setMeshPreview] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [lightbox, setLightbox] = useState<LightboxAsset | null>(null);
  const [imageCode, setImageCode] = useState<string | null>(null);
  const [imageRbx, setImageRbx] = useState<string | null>(null);
  const [meshCode, setMeshCode] = useState<string | null>(null);
  const [meshRbx, setMeshRbx] = useState<string | null>(null);

  return (
    <div>
      <h1>Atelier</h1>
      <p className="lede">
        Images via Gemini. Modèles 3D via{" "}
        {keys.meshProvider === "blender"
          ? "Blender (script bpy)"
          : keys.meshProvider === "tripo"
            ? "Tripo"
            : "Meshy"}
        . Les clés restent dans Réglages. Claude, Codex et Cursor passent par{" "}
        <code>node tools/lumen-asset.mjs</code> — ils ne voient pas les clés.
      </p>
      {err ? <p className="err">{err}</p> : null}
      <div className="grid">
        <div className="card">
          <h2 style={{ fontSize: 22 }}>Image · Gemini</h2>
          <textarea value={imagePrompt} onChange={(e) => setImagePrompt(e.target.value)} />
          <div className="row" style={{ marginTop: 12 }}>
            <button
              className="btn copper"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                setErr("");
                try {
                  const result = await invoke<{ dataUrl: string }>("generate_image", {
                    prompt: imagePrompt,
                  });
                  setImage(result.dataUrl);
                  setImageCode(null);
                  setImageRbx(null);
                  if (project) {
                    const saved = await invoke<{ code: string; robloxAssetId?: string | null }>(
                      "save_image_to_project",
                      {
                        projectPath: project.path,
                        dataUrl: result.dataUrl,
                        filename: `gemini-${Date.now()}.png`,
                      },
                    );
                    setImageCode(saved.code);
                    setImageRbx(saved.robloxAssetId ?? null);
                  }
                } catch (error) {
                  setErr(String(error));
                } finally {
                  setBusy(false);
                }
              }}
            >
              Générer
            </button>
          </div>
          {image ? (
            <div
              className="asset-open"
              role="button"
              tabIndex={0}
              onClick={() =>
                setLightbox({
                  kind: "image",
                  title: "Image Gemini",
                  src: image,
                  code: imageCode ?? undefined,
                  robloxAssetId: imageRbx ?? undefined,
                })
              }
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  setLightbox({
                    kind: "image",
                    title: "Image Gemini",
                    src: image,
                    code: imageCode ?? undefined,
                    robloxAssetId: imageRbx ?? undefined,
                  });
                }
              }}
            >
              <img className="preview" src={image} alt="Génération Gemini" />
              <div className="asset-ids">
                <AssetCode code={imageCode} />
                <AssetCode code={imageRbx} variant="roblox" />
              </div>
            </div>
          ) : null}
        </div>
        <div className="card">
          <h2 style={{ fontSize: 22 }}>
            3D ·{" "}
            {keys.meshProvider === "blender"
              ? "Blender"
              : keys.meshProvider === "tripo"
                ? "Tripo"
                : "Meshy"}
          </h2>
          {keys.meshProvider === "blender" ? (
            <>
              <p className="lede" style={{ marginBottom: 12 }}>
                Script bpy local — pas de clé Meshy. Installe Blender 4.x, ou indique le chemin dans
                Réglages.
              </p>
              <textarea
                value={blenderScript}
                onChange={(e) => setBlenderScript(e.target.value)}
                style={{ minHeight: 220, fontFamily: "ui-monospace, monospace", fontSize: 13 }}
              />
              <div className="row" style={{ marginTop: 12 }}>
                <button
                  className="btn"
                  disabled={busy || !project}
                  onClick={async () => {
                    if (!project) return;
                    setBusy(true);
                    setErr("");
                    try {
                      const saved = await invoke<BankItem & { path: string; code?: string; robloxAssetId?: string; previewPath?: string }>(
                        "run_blender_mesh_cmd",
                        {
                          projectPath: project.path,
                          script: blenderScript,
                          title: "atelier-blender",
                        },
                      );
                      setMeshPath(saved.path);
                      setMeshPreview(saved.previewPath ?? saved.path);
                      setMeshCode(saved.code || null);
                      setMeshRbx(saved.robloxAssetId || null);
                      setMesh("GLB Blender prêt");
                    } catch (error) {
                      setErr(String(error));
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  Construire dans Blender
                </button>
              </div>
            </>
          ) : (
            <>
          <textarea value={meshPrompt} onChange={(e) => setMeshPrompt(e.target.value)} />
          <div className="row" style={{ marginTop: 12 }}>
            <button
              className="btn"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                setErr("");
                try {
                  const job = await invoke<{ id: string; provider: string; status: string }>(
                    "generate_mesh",
                    { prompt: meshPrompt },
                  );
                  setMesh(`Job ${job.id} · ${job.status}`);
                  setMeshCode(null);
                  setMeshRbx(null);
                  const started = Date.now();
                  while (Date.now() - started < 120000) {
                    await new Promise((r) => setTimeout(r, 4000));
                    const polled = await invoke<{
                      status: string;
                      modelUrl: string | null;
                      thumbnailUrl: string | null;
                    }>("poll_mesh", { provider: job.provider, id: job.id });
                    setMesh(`Job ${job.id} · ${polled.status}`);
                    if (polled.modelUrl) {
                      setMesh(polled.modelUrl);
                      const saved = await invoke<BankItem>("save_mesh_url", {
                        url: polled.modelUrl,
                        name: `mesh-${Date.now()}`,
                        projectPath: project?.path ?? null,
                        thumbnailUrl: polled.thumbnailUrl,
                      });
                      setMeshPath(saved.path);
                      setMeshPreview(saved.previewPath ?? null);
                      setMeshCode(saved.code || null);
                      setMeshRbx(saved.robloxAssetId || null);
                      break;
                    }
                    if (["FAILED", "CANCELED", "ERROR"].includes(polled.status.toUpperCase())) {
                      break;
                    }
                  }
                } catch (error) {
                  setErr(String(error));
                } finally {
                  setBusy(false);
                }
              }}
            >
              Sculpturer
            </button>
          </div>
            </>
          )}
          {mesh ? <small>{mesh}</small> : null}
          {meshPath ? (
            <div
              className="asset-open"
              role="button"
              tabIndex={0}
              onClick={() =>
                setLightbox({
                  kind: "mesh",
                  title: "Modèle 3D",
                  path: meshPath,
                  code: meshCode ?? undefined,
                  robloxAssetId: meshRbx ?? undefined,
                })
              }
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  setLightbox({
                    kind: "mesh",
                    title: "Modèle 3D",
                    path: meshPath,
                    code: meshCode ?? undefined,
                    robloxAssetId: meshRbx ?? undefined,
                  });
                }
              }}
            >
              {meshPreview ? (
                <img className="preview" src={convertFileSrc(meshPreview)} alt="Aperçu 3D" />
              ) : (
                <MeshPreview path={meshPath} />
              )}
              <div className="asset-ids">
                <AssetCode code={meshCode} />
                <AssetCode code={meshRbx} variant="roblox" />
              </div>
            </div>
          ) : null}
        </div>
      </div>
      {lightbox ? <AssetLightbox asset={lightbox} onClose={() => setLightbox(null)} /> : null}
    </div>
  );
}

function BlenderPathField({
  value,
  onChange,
}: {
  value: string;
  onChange: (path: string) => void;
}) {
  const [detected, setDetected] = useState<string | null>(null);
  const [found, setFound] = useState(false);
  useEffect(() => {
    void invoke<{ found: boolean; path: string | null }>("detect_blender")
      .then((status) => {
        setFound(status.found);
        setDetected(status.path);
      })
      .catch(() => {
        setFound(false);
        setDetected(null);
      });
  }, [value]);
  return (
    <label>
      Chemin Blender (optionnel)
      <div className="row" style={{ gap: 8 }}>
        <input
          type="text"
          value={value}
          placeholder={detected || "C:\\Program Files\\Blender Foundation\\Blender 4.2\\blender.exe"}
          onChange={(e) => onChange(e.target.value)}
          style={{ flex: 1 }}
        />
        <button
          type="button"
          className="btn secondary"
          onClick={async () => {
            const picked = await open({
              multiple: false,
              filters: [{ name: "Blender", extensions: ["exe"] }],
            });
            if (typeof picked === "string" && picked) onChange(picked);
          }}
        >
          Parcourir
        </button>
      </div>
      <small className="lede" style={{ marginTop: 6, display: "block" }}>
        {found
          ? `Détecté : ${detected}`
          : "Blender introuvable. Installe Blender 4.x sur blender.org, ou colle le chemin de blender.exe."}
      </small>
    </label>
  );
}

function Settings({
  keys,
  agents,
  onSaved,
  theme,
  onToggleTheme,
  appUpdate,
  updateBusy,
  updateMsg,
  onCheckUpdate,
  onInstallUpdate,
}: {
  keys: Keys;
  agents: AgentStatus[];
  onSaved: () => Promise<void>;
  theme: Theme;
  onToggleTheme: () => void;
  appUpdate: Update | null;
  updateBusy: boolean;
  updateMsg: string;
  onCheckUpdate: () => Promise<void>;
  onInstallUpdate: () => void;
}) {
  const [form, setForm] = useState(keys);
  const [msg, setMsg] = useState("");
  useEffect(() => setForm(keys), [keys]);

  const fields = useMemo(
    () => [
      ["gemini", "Clé API Gemini", "images, icônes, miniatures"],
      ["meshy", "Clé API Meshy", "modèles 3D texturés"],
      ["tripo", "Clé API Tripo", "alternative 3D"],
      ["cursor", "Clé API Cursor", "agent Cursor embarqué"],
      ["robloxOauthClientId", "Client ID OAuth Roblox", "connexion Compte Roblox"],
      ["robloxApiKey", "Clé Open Cloud Roblox", "secours si l’OAuth n’a pas asset:write"],
      ["robloxUserId", "UserId Roblox", "ton identifiant créateur"],
    ] as const,
    [],
  );

  return (
    <div>
      <h1>Réglages</h1>
      <div className="card" style={{ maxWidth: 640, marginBottom: 24 }}>
        <h2 style={{ fontSize: 20 }}>Mises à jour</h2>
        <p className="lede" style={{ marginBottom: 12 }}>
          Lumen vérifie GitHub au démarrage. Ton ami installe le .exe une fois, puis
          les versions suivantes s’installent d’ici.
        </p>
        <div className="row">
          <button className="btn secondary" type="button" disabled={updateBusy} onClick={() => void onCheckUpdate()}>
            Vérifier
          </button>
          {appUpdate ? (
            <button className="btn copper" type="button" disabled={updateBusy} onClick={onInstallUpdate}>
              Installer {appUpdate.version}
            </button>
          ) : null}
          {updateMsg ? <small>{updateMsg}</small> : null}
        </div>
      </div>
      <p className="lede">
        Claude Code, Codex et Antigravity se connectent avec leur propre compte (login officiel).
        La connexion Lumen passe par OAuth Roblox (ajoute <code>asset:read</code> et{" "}
        <code>asset:write</code> à l’app, puis reconnecte-toi pour publier). Cursor, Gemini et
        Meshy/Tripo restent des clés collées ici. Blender tourne en local, sans clé. Rien n’est
        revendu.
      </p>
      <div className="card" style={{ maxWidth: 640, marginBottom: 24 }}>
        <h2 style={{ fontSize: 20 }}>Apparence</h2>
        <p className="lede" style={{ marginBottom: 12 }}>
          Crème le jour, encres et cuivre la nuit. Le choix est mémorisé sur cet ordinateur.
        </p>
        <button className="btn copper" type="button" onClick={onToggleTheme}>
          {theme === "dark" ? "Passer en mode clair" : "Passer en mode sombre"}
        </button>
      </div>
      <div className="row" style={{ marginBottom: 24 }}>
        {agents.map((agent) => (
          <span key={agent.id} className={`pill ${agent.found ? "ok" : "no"}`}>
            {agent.label} {agent.found ? "détecté" : "absent"}
          </span>
        ))}
      </div>
      <div className="form">
        {fields.map(([key, label, hint]) => (
          <label key={key}>
            {label}
            <input
              type="password"
              value={form[key]}
              placeholder={hint}
              onChange={(e) => setForm({ ...form, [key]: e.target.value })}
            />
          </label>
        ))}
        <label>
          Moteur 3D
          <select
            value={form.meshProvider}
            onChange={(e) =>
              setForm({ ...form, meshProvider: e.target.value as Keys["meshProvider"] })
            }
          >
            <option value="meshy">Meshy (IA texturée)</option>
            <option value="tripo">Tripo (IA texturée)</option>
            <option value="blender">Blender (script local, sans clé)</option>
          </select>
        </label>
        {form.meshProvider === "blender" ? (
          <BlenderPathField
            value={form.blenderPath}
            onChange={(blenderPath) => setForm({ ...form, blenderPath })}
          />
        ) : null}
        <div className="row">
          <button
            className="btn copper"
            onClick={async () => {
              await invoke("set_keys", { keys: form });
              setMsg("Enregistré dans AppData/Lumen/keys.json");
              await onSaved();
            }}
          >
            Enregistrer
          </button>
          {msg ? <small>{msg}</small> : null}
        </div>
        <div className="card">
          <h2 style={{ fontSize: 20 }}>Installer les agents</h2>
          <p className="lede" style={{ marginBottom: 0 }}>
            Claude Code : <code>irm https://claude.ai/install.ps1 | iex</code>
            <br />
            Codex : <code>irm https://chatgpt.com/codex/install.ps1 | iex</code>
            <br />
            Cursor : installe Cursor (clé API dans Intégrations) — Lumen s’en sert via le SDK.
            <br />
            Antigravity : <code>irm https://antigravity.google/cli/install.ps1 | iex</code>
            {" "}
            — connexion Google au premier lancement, pas de clé dans Lumen.
          </p>
        </div>
        <div className="card">
          <h2 style={{ fontSize: 20 }}>Studio & Rojo</h2>
          <p className="lede">
            Ouvrir un projet démarre tout seul Node, Rojo, LumenSync et la compilation.
            Dans Studio, Lumen te demande de lier la place si besoin. Plugin Rojo : hôte
            `127.0.0.1` et le port affiché (pas 34872, c’est VibeStarter).
          </p>
          <div className="row">
            <button
              className="btn secondary"
              onClick={async () => {
                const path = await invoke<string>("install_rojo");
                setMsg(`Rojo : ${path}`);
              }}
            >
              Installer Rojo
            </button>
            <button
              className="btn secondary"
              onClick={async () => {
                const path = await invoke<string>("install_studio_plugin");
                setMsg(`Plugin : ${path}`);
              }}
            >
              Installer le plugin Studio
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function isInspirationItem(item: BankItem) {
  return item.source === "inspiration" || (item.code ?? "").toUpperCase().startsWith("INS-");
}

function isTextureItem(item: BankItem) {
  return item.source === "texture" || (item.code ?? "").toUpperCase().startsWith("TEX-");
}

function isStudioTexture(item: BankItem) {
  return item.id.includes("studio:");
}

const SCALE_TYPES = ["Stretch", "Fit", "Crop", "Tile", "Slice"] as const;

function StudioTilingFields({
  item,
  onSaved,
}: {
  item: BankItem;
  onSaved: (item: BankItem) => void;
}) {
  const [scaleType, setScaleType] = useState(item.scaleType || "Stretch");
  const [xOffset, setXOffset] = useState(item.tileSize?.xOffset ?? 100);
  const [yOffset, setYOffset] = useState(item.tileSize?.yOffset ?? 100);
  const [saving, setSaving] = useState(false);
  const [hint, setHint] = useState("");

  async function save() {
    setSaving(true);
    setHint("");
    try {
      const saved = await invoke<BankItem>("set_texture_tiling", {
        id: item.id,
        scaleType,
        tileSize: {
          xScale: item.tileSize?.xScale ?? 0,
          xOffset: Number(xOffset) || 0,
          yScale: item.tileSize?.yScale ?? 0,
          yOffset: Number(yOffset) || 0,
        },
      });
      onSaved(saved);
      setHint("Enregistré");
    } catch (error) {
      setHint(String(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="tex-tiling" onClick={(event) => event.stopPropagation()}>
      <label>
        ScaleType
        <select
          value={scaleType}
          onChange={(event) => setScaleType(event.target.value)}
        >
          {SCALE_TYPES.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
      </label>
      {scaleType === "Tile" ? (
        <div className="tex-tilesize">
          <label>
            TileSize X
            <input
              type="number"
              value={xOffset}
              onChange={(event) => setXOffset(Number(event.target.value))}
            />
          </label>
          <label>
            TileSize Y
            <input
              type="number"
              value={yOffset}
              onChange={(event) => setYOffset(Number(event.target.value))}
            />
          </label>
        </div>
      ) : null}
      <button className="btn secondary" type="button" disabled={saving} onClick={() => void save()}>
        Enregistrer le pavage
      </button>
      {hint ? <small>{hint}</small> : null}
    </div>
  );
}

function Bank() {
  const [shelf, setShelf] = useState<"lumen" | "vibestarter" | "inspiration" | "textures">("lumen");
  const [lumenItems, setLumenItems] = useState<BankItem[]>([]);
  const [vibeItems, setVibeItems] = useState<BankItem[]>([]);
  const [textureItems, setTextureItems] = useState<BankItem[]>([]);
  const [kindFilter, setKindFilter] = useState<"all" | "image" | "mesh">("all");
  const [vibeCat, setVibeCat] = useState<"home" | "image" | "mesh">("home");
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(0);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const [texName, setTexName] = useState("");
  const [texId, setTexId] = useState("");
  const [lightbox, setLightbox] = useState<LightboxAsset | null>(null);
  const [ready, setReady] = useState(false);
  const pageSize = 48;

  async function refresh(force = false) {
    const [lumen, vibe, textures] = await Promise.all([
      invoke<BankItem[]>("list_bank"),
      invoke<BankItem[]>("list_vibestarter_bank", { force }).catch(() => [] as BankItem[]),
      invoke<BankItem[]>("list_textures_bank", { force }).catch(() => [] as BankItem[]),
    ]);
    setLumenItems(lumen);
    setVibeItems(vibe);
    setTextureItems(textures);
    setReady(true);
  }

  useEffect(() => {
    void refresh().catch((error) => setErr(String(error)));
  }, []);

  useEffect(() => {
    setPage(0);
  }, [shelf, kindFilter, vibeCat, query]);

  const playableItems = useMemo(
    () => lumenItems.filter((item) => !isInspirationItem(item)),
    [lumenItems],
  );
  const inspirationItems = useMemo(
    () => lumenItems.filter(isInspirationItem),
    [lumenItems],
  );
  const items =
    shelf === "lumen"
      ? playableItems
      : shelf === "inspiration"
        ? inspirationItems
        : shelf === "textures"
          ? textureItems
          : vibeItems;
  const activeKind =
    shelf === "vibestarter"
      ? vibeCat === "home"
        ? "all"
        : vibeCat
      : shelf === "inspiration" || shelf === "textures"
        ? "all"
        : kindFilter;
  const vibeHome = shelf === "vibestarter" && vibeCat === "home" && !query.trim();
  const recentVibe = useMemo(
    () =>
      [...vibeItems]
        .sort((a, b) => Number(b.createdAt || 0) - Number(a.createdAt || 0))
        .slice(0, 12),
    [vibeItems],
  );
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return items.filter((item) => {
      if (activeKind !== "all" && item.kind !== activeKind) return false;
      if (!q) return true;
      return (
        item.name.toLowerCase().includes(q) ||
        (item.code ?? "").toLowerCase().includes(q) ||
        item.path.toLowerCase().includes(q) ||
        (item.robloxAssetId ?? "").toLowerCase().includes(q)
      );
    });
  }, [items, activeKind, query]);
  const pageCount = Math.max(1, Math.ceil(filtered.length / pageSize));
  const safePage = Math.min(page, pageCount - 1);
  const visible = filtered.slice(safePage * pageSize, safePage * pageSize + pageSize);
  const iconCount = useMemo(
    () => vibeItems.filter((item) => item.kind === "image").length,
    [vibeItems],
  );
  const meshCount = useMemo(
    () => vibeItems.filter((item) => item.kind === "mesh").length,
    [vibeItems],
  );

  return (
    <div>
      <h1>Banque</h1>
      <p className="lede">
        Tes créations Lumen, VibeStarter, les textures (dossier local **et** IDs Roblox Studio),
        et des captures d’inspiration. Publier envoie l’image (Decal) ou le mesh (Model GLB)
        vers Roblox — pas les inspirations. Les IDs Studio sont déjà des `rbxassetid://`.
      </p>
      <div className="bank-shelves" role="tablist">
        <button
          type="button"
          role="tab"
          className={`bank-shelf ${shelf === "lumen" ? "on" : ""}`}
          onClick={() => setShelf("lumen")}
        >
          Lumen <span>{playableItems.length}</span>
        </button>
        <button
          type="button"
          role="tab"
          className={`bank-shelf ${shelf === "vibestarter" ? "on" : ""}`}
          onClick={() => setShelf("vibestarter")}
        >
          VibeStarter <span>{vibeItems.length}</span>
        </button>
        <button
          type="button"
          role="tab"
          className={`bank-shelf ${shelf === "textures" ? "on" : ""}`}
          onClick={() => setShelf("textures")}
        >
          Textures <span>{textureItems.length}</span>
        </button>
        <button
          type="button"
          role="tab"
          className={`bank-shelf ${shelf === "inspiration" ? "on" : ""}`}
          onClick={() => setShelf("inspiration")}
        >
          Inspiration <span>{inspirationItems.length}</span>
        </button>
      </div>
      <div className="toolbar">
        {shelf === "lumen" ? (
          <button
            className="btn secondary"
            disabled={busy}
            onClick={async () => {
              const selected = await open({
                multiple: true,
                filters: [
                  { name: "Assets", extensions: ["png", "jpg", "jpeg", "webp", "glb", "gltf", "fbx", "ogg", "mp3"] },
                ],
              });
              const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
              for (const filePath of paths) {
                await invoke("import_to_bank", { filePath, source: "import" });
              }
              await refresh();
            }}
          >
            Importer
          </button>
        ) : shelf === "textures" ? (
          <span className="bank-pack-meta">Dossier Images/textures · HUD / UI · codes TEX-xxxx</span>
        ) : shelf === "inspiration" ? (
          <button
            className="btn secondary"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              setErr("");
              try {
                const selected = await open({
                  multiple: true,
                  filters: [
                    { name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] },
                  ],
                });
                const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
                for (const filePath of paths) {
                  await invoke("import_to_bank", { filePath, source: "inspiration" });
                }
                await refresh();
              } catch (error) {
                setErr(String(error));
              } finally {
                setBusy(false);
              }
            }}
          >
            Importer une capture
          </button>
        ) : vibeCat !== "home" ? (
          <button className="btn secondary" type="button" onClick={() => setVibeCat("home")}>
            ← Catégories
          </button>
        ) : (
          <span className="bank-pack-meta">Comme VibeStarter : récemment ajoutés, puis par type</span>
        )}
        <input
          type="search"
          placeholder={
            shelf === "inspiration"
              ? "Rechercher une capture…"
              : shelf === "textures"
                ? "Rechercher une texture…"
                : "Rechercher des assets…"
          }
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        {shelf === "lumen" ? (
          <select
            value={kindFilter}
            onChange={(event) => setKindFilter(event.target.value as "all" | "image" | "mesh")}
          >
            <option value="all">Tout</option>
            <option value="image">Icônes / images</option>
            <option value="mesh">3D</option>
          </select>
        ) : null}
        {shelf === "inspiration" ? (
          <span className="bank-pack-meta">Pas envoyé sur Roblox · codes INS-xxxx</span>
        ) : null}
        {shelf === "textures" ? (
          <span className="bank-pack-meta">
            PNG locaux · IDs Studio (`rbxassetid://`) · ScaleType / TileSize sur les importées
          </span>
        ) : null}
        <button className="btn secondary" onClick={() => void refresh(true)}>
          Actualiser
        </button>
      </div>
      {shelf === "textures" ? (
        <div className="toolbar" style={{ marginTop: 8 }}>
          <button
            className="btn copper"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              setErr("");
              try {
                const added = await invoke<number>("import_studio_textures");
                await refresh(true);
                setErr(
                  added
                    ? `${added} texture${added > 1 ? "s" : ""} importée${added > 1 ? "s" : ""} depuis Studio`
                    : "Aucune nouvelle texture dans la place ouverte. Les IDs déjà en banque sont ignorés.",
                );
              } catch (error) {
                setErr(String(error));
              } finally {
                setBusy(false);
              }
            }}
          >
            Importer depuis Studio
          </button>
          <input
            value={texId}
            onChange={(e) => setTexId(e.target.value)}
            placeholder="rbxassetid://123456789"
            style={{ minWidth: 220 }}
          />
          <input
            value={texName}
            onChange={(e) => setTexName(e.target.value)}
            placeholder="Nom (ex. brique, herbe)"
          />
          <button
            className="btn secondary"
            disabled={busy || !texId.trim()}
            onClick={async () => {
              setBusy(true);
              setErr("");
              try {
                await invoke("add_studio_texture", {
                  name: texName.trim(),
                  content: texId.trim(),
                });
                setTexId("");
                setTexName("");
                await refresh(true);
              } catch (error) {
                setErr(String(error));
              } finally {
                setBusy(false);
              }
            }}
          >
            Ajouter l’ID
          </button>
        </div>
      ) : null}
      {err ? <p className="err">{err}</p> : null}
      {!ready ? <p className="lede">Chargement de la banque…</p> : null}
      {vibeHome ? (
        <div className="vibe-home">
          {recentVibe.length > 0 ? (
            <section>
              <h2 className="vibe-heading">Récemment ajoutés</h2>
              <div className="vibe-recent">
                {recentVibe.map((item) => (
                  <button
                    type="button"
                    className="vibe-recent-card"
                    key={item.id}
                    onClick={() => {
                      const next = lightboxFromBank(item);
                      if (next) setLightbox(next);
                    }}
                  >
                    {item.kind === "image" ? (
                      <img src={convertFileSrc(item.path)} alt={item.name} />
                    ) : (
                      <MeshStill
                        id={item.id}
                        path={item.path}
                        previewPath={item.previewPath}
                        className="bank-thumb bank-mesh"
                        onReady={(previewPath) => {
                          setVibeItems((prev) =>
                            prev.map((row) =>
                              row.id === item.id ? { ...row, previewPath } : row,
                            ),
                          );
                        }}
                      />
                    )}
                    <span>{item.name}</span>
                  </button>
                ))}
              </div>
            </section>
          ) : null}
          <section>
            <h2 className="vibe-heading">Catégories</h2>
            <div className="vibe-cats">
              <button type="button" className="card vibe-cat" onClick={() => setVibeCat("image")}>
                <div className="vibe-cat-mark">Icônes</div>
                <strong>Icônes</strong>
                <small>{iconCount} assets</small>
              </button>
              <button type="button" className="card vibe-cat" onClick={() => setVibeCat("mesh")}>
                <div className="vibe-cat-mark mesh">3D</div>
                <strong>Modèles 3D</strong>
                <small>{meshCount} assets</small>
              </button>
            </div>
          </section>
        </div>
      ) : filtered.length === 0 ? (
        <div className="empty">
          {shelf === "inspiration"
            ? "Importe des captures d’HUD, boutiques ou menus. Dis ensuite à un agent de s’en inspirer — elles ne partent pas sur Roblox."
            : shelf === "lumen"
            ? "La banque Lumen est vide. Génère dans l’Atelier ou importe un fichier."
            : shelf === "textures"
            ? "Aucune texture. Ajoute des PNG dans Images/textures, importe depuis Studio, ou colle un rbxassetid."
            : "Aucun asset VibeStarter. Vérifie le dossier AssetsDownloader/vibestarter_assets."}
        </div>
      ) : (
        <>
          {shelf === "vibestarter" && vibeCat !== "home" ? (
            <h2 className="vibe-heading">
              {vibeCat === "image" ? "Icônes" : "Modèles 3D"}
              <span>
                {" "}
                · {filtered.length} assets
              </span>
            </h2>
          ) : shelf === "inspiration" ? (
            <h2 className="vibe-heading">
              Captures UI
              <span>
                {" "}
                · {filtered.length}
              </span>
            </h2>
          ) : shelf === "textures" ? (
            <h2 className="vibe-heading">
              Textures
              <span>
                {" "}
                · {filtered.length}
              </span>
            </h2>
          ) : null}
          <div className="grid">
            {visible.map((item) => (
              <div
                className="card bank-card"
                key={item.id}
                onClick={() => {
                  const next = lightboxFromBank(item);
                  if (next) setLightbox(next);
                }}
              >
                {item.kind === "image" ? (
                  <img
                    className="bank-thumb"
                    src={bankImageSrc(item)}
                    alt={item.name}
                  />
                ) : item.kind === "mesh" ? (
                  <MeshStill
                    id={item.id}
                    path={item.path}
                    previewPath={item.previewPath}
                    onReady={(previewPath) => {
                      const apply = (rows: BankItem[]) =>
                        rows.map((row) =>
                          row.id === item.id ? { ...row, previewPath } : row,
                        );
                      setLumenItems(apply);
                      setVibeItems(apply);
                    }}
                  />
                ) : (
                  <div className="bank-thumb">{item.kind}</div>
                )}
                <div className="bank-card-head">
                  <h2>{item.name}</h2>
                </div>
                <div className="asset-ids">
                  <AssetCode code={item.code} />
                  {isInspirationItem(item) ? null : (
                    <AssetCode code={item.robloxAssetId} variant="roblox" />
                  )}
                </div>
                <small>
                  {isInspirationItem(item)
                    ? "référence UI · pas un asset Roblox"
                    : isTextureItem(item)
                      ? `texture${item.id.includes("studio:") ? " Studio" : " HUD/UI"}${item.robloxAssetId ? "" : " · pas encore sur Roblox"}`
                    : `${item.source}${item.robloxAssetId ? "" : " · pas encore sur Roblox"}`}
                  {item.kind === "image" || item.kind === "mesh" ? " · cliquer pour agrandir" : ""}
                </small>
                {isStudioTexture(item) ? (
                  <StudioTilingFields
                    key={`${item.id}:${item.scaleType ?? ""}:${item.tileSize?.xOffset ?? ""}:${item.tileSize?.yOffset ?? ""}`}
                    item={item}
                    onSaved={(saved) => {
                      setTextureItems((rows) =>
                        rows.map((row) => (row.id === saved.id ? saved : row)),
                      );
                    }}
                  />
                ) : null}
                {!isInspirationItem(item) &&
                !item.robloxAssetId &&
                (item.kind === "image" || item.kind === "mesh") ? (
                  <div className="row" style={{ marginTop: 10 }}>
                    <button
                      className="btn copper"
                      disabled={busy}
                      onClick={async (event) => {
                        event.stopPropagation();
                        setBusy(true);
                        setErr("");
                        try {
                          await invoke("publish_bank_item", { id: item.id });
                          await refresh();
                        } catch (error) {
                          setErr(String(error));
                        } finally {
                          setBusy(false);
                        }
                      }}
                    >
                      Publier vers Roblox
                    </button>
                  </div>
                ) : null}
              </div>
            ))}
          </div>
          {pageCount > 1 ? (
            <div className="bank-pager">
              <button
                className="btn secondary"
                type="button"
                disabled={safePage <= 0}
                onClick={() => setPage((n) => Math.max(0, n - 1))}
              >
                Précédent
              </button>
              <span>
                {safePage + 1} / {pageCount}
                {" · "}
                {filtered.length} assets
              </span>
              <button
                className="btn secondary"
                type="button"
                disabled={safePage >= pageCount - 1}
                onClick={() => setPage((n) => n + 1)}
              >
                Suivant
              </button>
            </div>
          ) : null}
        </>
      )}
      {lightbox ? <AssetLightbox asset={lightbox} onClose={() => setLightbox(null)} /> : null}
    </div>
  );
}

function labelFor(kind: AgentKind) {
  if (kind === "claude") return "Claude Code";
  if (kind === "codex") return "Codex";
  if (kind === "antigravity") return "Antigravity";
  return "Cursor";
}
