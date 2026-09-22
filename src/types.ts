export type View = "projects" | "studio" | "workshop" | "bank" | "settings";

export type RobloxUser = {
  id: string;
  username: string;
  displayName: string;
  picture: string;
  profile: string;
};

export type AgentKind = "claude" | "codex" | "cursor" | "antigravity";

export type ProjectShare = {
  shared: boolean;
  remote: string;
  dirty: boolean;
  detail: string;
};

export type Project = {
  name: string;
  path: string;
  createdAt: string;
  boundPlaceName?: string | null;
  boundPlaceId?: number | null;
  referencePaths?: string[];
  referencePath?: string | null;
};

export type AgentStatus = {
  id: AgentKind;
  label: string;
  found: boolean;
  path: string | null;
};

export type Keys = {
  gemini: string;
  meshy: string;
  tripo: string;
  cursor: string;
  meshProvider: "meshy" | "tripo" | "blender";
  robloxApiKey: string;
  robloxUserId: string;
  robloxOauthClientId: string;
  blenderPath: string;
  vibeAssetsPath: string;
  shareBank: boolean;
  bankSyncToken: string;
};

export type AgentTab = {
  localId: string;
  kind: AgentKind;
  sessionId: string | null;
  resumeId: string | null;
  title: string;
  error?: string;
  running: boolean;
};

export type StartedAgent = {
  sessionId: string;
  resumeId: string;
  localId: string;
};

export type SwarmState = {
  paused: boolean;
  pausedAt?: string | null;
  agents: Array<{
    localId: string;
    kind: AgentKind;
    title: string;
    resumeId?: string | null;
  }>;
};

export type LiveAgent = {
  sessionId: string;
  localId: string;
  kind: AgentKind;
  resumeId: string;
};

export type CompilerStatus = {
  watching: boolean;
  ready: boolean;
  projectPath: string | null;
  lastError: string | null;
};

export type ToolchainStatus = {
  node: boolean;
  npm: boolean;
  nodePath: string | null;
};

export type RojoStatus = {
  found: boolean;
  path: string | null;
  serving: boolean;
  projectPath: string | null;
  port: number;
  reachable: boolean;
};

export type StudioHeartbeat = {
  connected: boolean;
  placeName: string;
  placeId: number;
  lastSeen: number;
  pluginInstalled: boolean;
  bound: boolean;
};

export type PlaceOffer = {
  serving: boolean;
  port: number;
  projectName: string;
  projectPath: string;
  boundPlaceName: string;
  boundPlaceId: number;
  generation: number;
};

export type BankItem = {
  id: string;
  name: string;
  kind: string;
  path: string;
  source: string;
  createdAt: string;
  robloxAssetId: string | null;
  previewPath?: string | null;
  code?: string;
  scaleType?: string | null;
  shared?: boolean;
  tileSize?: {
    xScale: number;
    xOffset: number;
    yScale: number;
    yOffset: number;
  } | null;
};
