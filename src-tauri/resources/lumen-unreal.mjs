#!/usr/bin/env node
/**
 * Pont Lumen ↔ Unreal Engine : prépare le module C++, compile, et parle au MCP officiel de l'éditeur.
 *
 * Usage:
 *   node tools/lumen-unreal.mjs status
 *   node tools/lumen-unreal.mjs init-cpp
 *   node tools/lumen-unreal.mjs build
 *   node tools/lumen-unreal.mjs tools
 *
 * `build` : éditeur fermé → Build.bat (UnrealBuildTool). Éditeur ouvert → Live Coding via le MCP
 * de l'éditeur (Build.bat refuse de compiler pendant que l'éditeur tient les DLL).
 */

import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const MCP_URL = process.env.UNREAL_MCP_URL || "http://127.0.0.1:8000/mcp";

function fail(message, code = 1) {
  console.error(message);
  process.exit(code);
}

function projectRoot() {
  return path.resolve(process.env.LUMEN_PROJECT || process.cwd());
}

function uprojectPath() {
  const root = projectRoot();
  const file = readdirSync(root).find((f) => f.toLowerCase().endsWith(".uproject"));
  if (!file) fail(`Aucun .uproject dans ${root}.`);
  return path.join(root, file);
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8").replace(/^\uFEFF/, ""));
}

function projectName() {
  return path.basename(uprojectPath(), ".uproject");
}

/** Nom de module C++ valide : lettres, chiffres, _ ; ne commence pas par un chiffre. */
function moduleName() {
  const raw = projectName().replace(/[^A-Za-z0-9_]/g, "");
  return /^[0-9]/.test(raw) || !raw ? `Game${raw}` : raw;
}

function regValue(key, value) {
  try {
    const out = execFileSync("reg", ["query", key, "/v", value], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
    const match = new RegExp(`${value.replace(/[{}]/g, "\\$&")}\\s+REG_\\w+\\s+(.+)`).exec(out);
    return match ? match[1].trim() : "";
  } catch {
    return "";
  }
}

/** Dossier du moteur associé au projet (EngineAssociation : « 5.8 » via le launcher, ou GUID d'un build source). */
function engineDir() {
  const assoc = String(readJson(uprojectPath()).EngineAssociation || "").trim();
  const candidates = [];
  if (assoc) {
    try {
      const dat = readJson("C:/ProgramData/Epic/UnrealEngineLauncher/LauncherInstalled.dat");
      for (const item of dat.InstallationList || []) {
        if (item.AppName === `UE_${assoc}`) candidates.push(item.InstallLocation);
      }
    } catch {
      /* pas de launcher */
    }
    candidates.push(regValue(`HKLM\\SOFTWARE\\EpicGames\\Unreal Engine\\${assoc}`, "InstalledDirectory"));
    candidates.push(regValue("HKCU\\Software\\Epic Games\\Unreal Engine\\Builds", assoc));
  }
  candidates.push(`C:/Program Files/Epic Games/UE_${assoc}`);
  const found = candidates.find((dir) => dir && existsSync(path.join(dir, "Engine/Build/BatchFiles/Build.bat")));
  if (!found) {
    fail(`Moteur Unreal ${assoc || "?"} introuvable. Installe-le depuis l'Epic Games Launcher (EngineAssociation du .uproject : « ${assoc} »).`);
  }
  return found;
}

function hasCppModule() {
  const project = readJson(uprojectPath());
  return Array.isArray(project.Modules) && project.Modules.length > 0 && existsSync(path.join(projectRoot(), "Source"));
}

// ---------- MCP de l'éditeur (Streamable HTTP, plugin ModelContextProtocol d'Unreal 5.8) ----------

async function mcpPost(body, sessionId, timeoutMs) {
  const headers = { "content-type": "application/json", accept: "application/json, text/event-stream" };
  if (sessionId) headers["mcp-session-id"] = sessionId;
  const res = await fetch(MCP_URL, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(timeoutMs),
  });
  const text = await res.text();
  const session = res.headers.get("mcp-session-id") || sessionId;
  if (!text.trim()) return { session, message: null };
  // Réponse JSON directe, ou flux SSE (« data: {...} »).
  const payload = text.trimStart().startsWith("{")
    ? text
    : text
        .split(/\r?\n/)
        .filter((line) => line.startsWith("data:"))
        .map((line) => line.slice(5).trim())
        .pop() || "{}";
  return { session, message: JSON.parse(payload) };
}

async function mcpSession() {
  try {
    const init = await mcpPost(
      {
        jsonrpc: "2.0",
        id: 1,
        method: "initialize",
        params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "lumen-unreal", version: "1.0" } },
      },
      null,
      3000,
    );
    if (!init.message?.result) return null;
    await mcpPost({ jsonrpc: "2.0", method: "notifications/initialized" }, init.session, 3000).catch(() => {});
    let id = 2;
    return {
      info: init.message.result.serverInfo,
      async call(method, params, timeoutMs = 60000) {
        const { message } = await mcpPost({ jsonrpc: "2.0", id: id++, method, params }, init.session, timeoutMs);
        if (message?.error) throw new Error(message.error.message || JSON.stringify(message.error));
        return message?.result;
      },
    };
  } catch {
    return null;
  }
}

function toolText(result) {
  return (result?.content || [])
    .map((part) => (part.type === "text" ? part.text : JSON.stringify(part)))
    .join("\n")
    .trim();
}

// ---------- Commandes ----------

async function status() {
  const uproject = uprojectPath();
  console.log(`Projet : ${projectName()} (${uproject})`);
  console.log(`Moteur : ${engineDir()}`);
  console.log(`Module C++ : ${hasCppModule() ? moduleName() : "aucun (projet Blueprint seul) → node tools/lumen-unreal.mjs init-cpp"}`);
  const mcp = await mcpSession();
  console.log(
    mcp
      ? `Éditeur : ouvert, MCP joignable sur ${MCP_URL}`
      : `Éditeur : fermé (ou MCP non démarré sur ${MCP_URL})`,
  );
}

const TEMPLATES = {
  target: (name, type) => `using UnrealBuildTool;

public class ${name}${type === "Editor" ? "Editor" : ""}Target : TargetRules
{
	public ${name}${type === "Editor" ? "Editor" : ""}Target(TargetInfo Target) : base(Target)
	{
		Type = TargetType.${type};
		DefaultBuildSettings = BuildSettingsVersion.Latest;
		IncludeOrderVersion = EngineIncludeOrderVersion.Latest;
		ExtraModuleNames.Add("${name}");
	}
}
`,
  build: (name) => `using UnrealBuildTool;

public class ${name} : ModuleRules
{
	public ${name}(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
		PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine", "InputCore", "EnhancedInput" });
	}
}
`,
  header: () => `#pragma once

#include "CoreMinimal.h"
`,
  source: (name) => `#include "${name}.h"
#include "Modules/ModuleManager.h"

IMPLEMENT_PRIMARY_GAME_MODULE(FDefaultGameModuleImpl, ${name}, "${name}");
`,
};

function initCpp() {
  if (hasCppModule()) {
    console.log(`Module C++ déjà présent : ${moduleName()}. Rien à faire.`);
    return;
  }
  const root = projectRoot();
  const name = moduleName();
  const src = path.join(root, "Source");
  const mod = path.join(src, name);
  mkdirSync(mod, { recursive: true });
  const write = (file, body) => {
    if (!existsSync(file)) writeFileSync(file, body);
  };
  write(path.join(src, `${name}.Target.cs`), TEMPLATES.target(name, "Game"));
  write(path.join(src, `${name}Editor.Target.cs`), TEMPLATES.target(name, "Editor"));
  write(path.join(mod, `${name}.Build.cs`), TEMPLATES.build(name));
  write(path.join(mod, `${name}.h`), TEMPLATES.header());
  write(path.join(mod, `${name}.cpp`), TEMPLATES.source(name));

  const file = uprojectPath();
  const project = readJson(file);
  project.Modules = [{ Name: name, Type: "Runtime", LoadingPhase: "Default" }];
  writeFileSync(file, JSON.stringify(project, null, "\t"));
  console.log(`Module C++ « ${name} » créé dans Source/${name}/ et déclaré dans le .uproject.`);
  console.log("Lance maintenant : node tools/lumen-unreal.mjs build (éditeur fermé la première fois).");
}

/** Lignes d'erreur MSVC / UBT : « C:\...\Foo.cpp(12): error C2065: ... ». */
function summarize(output) {
  const lines = output.split(/\r?\n/);
  const errors = lines.filter((l) => /\b(error|fatal error)\b\s*[A-Z]*\d*\s*:/i.test(l) || /^ERROR:/i.test(l.trim()));
  const warnings = lines.filter((l) => /\bwarning\b\s*[A-Z]*\d*\s*:/i.test(l));
  return { errors: [...new Set(errors)], warnings: [...new Set(warnings)] };
}

function runBuildBat() {
  const engine = engineDir();
  const bat = path.join(engine, "Engine/Build/BatchFiles/Build.bat");
  const args = [`${moduleName()}Editor`, "Win64", "Development", `-Project=${uprojectPath()}`, "-WaitMutex", "-NoHotReloadFromIDE"];
  console.log(`Build.bat ${args.join(" ")}`);
  return new Promise((resolve) => {
    // /s retire la première et la dernière guillemet : la commande entière doit donc être entourée d'une paire en plus.
    const line = `"${bat}" ${args.map((a) => `"${a}"`).join(" ")}`;
    const child = spawn("cmd.exe", ["/d", "/s", "/c", `"${line}"`], {
      windowsVerbatimArguments: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let output = "";
    const keep = (data) => {
      output += data.toString();
    };
    child.stdout.on("data", keep);
    child.stderr.on("data", keep);
    child.on("close", (code) => resolve({ code: code ?? 1, output }));
  });
}

async function liveCoding(mcp) {
  const listed = await mcp.call("tools/list", {});
  const names = (listed?.tools || []).map((t) => t.name);
  // Recherche d'outils activée (par défaut) : seuls list_toolsets / describe_toolset / call_tool sont exposés.
  const direct = names.find((n) => /compile_?live_?coding/i.test(n));
  let result;
  if (direct) {
    result = await mcp.call("tools/call", { name: direct, arguments: {} }, 600000);
  } else if (names.includes("call_tool")) {
    // Ex. « LiveCodingToolset.LiveCodingToolset » ; call_tool attend le nom de l'outil sans ce préfixe.
    const toolsets = toolText(await mcp.call("tools/call", { name: "list_toolsets", arguments: {} }));
    const toolset = /\bLiveCodingToolset\.[A-Za-z_]+/.exec(toolsets)?.[0];
    if (!toolset) {
      fail("Le toolset Live Coding n'est pas chargé dans l'éditeur (plugin LiveCodingToolset). Ferme l'éditeur puis relance build.");
    }
    result = await mcp.call(
      "tools/call",
      { name: "call_tool", arguments: { toolset_name: toolset, tool_name: "CompileLiveCoding", arguments: {} } },
      600000,
    );
  } else {
    fail("Le MCP de l'éditeur ne propose pas Live Coding. Ferme l'éditeur puis relance build.");
  }
  // Réponse : {"returnValue": "Result: Success\n\nCompile Output:\n..."} (ELiveCodingCompileResult).
  let text = toolText(result);
  try {
    text = JSON.parse(text).returnValue ?? text;
  } catch {
    /* texte brut */
  }
  console.log(text || "(Live Coding n'a rien renvoyé)");
  const state = /Result:\s*(\w+)/.exec(text)?.[1] || "";
  if (/^(Success|NoChanges)$/i.test(state) && !result?.isError) {
    console.log(state === "NoChanges" ? "\nLive Coding : rien à recompiler." : "\nLive Coding OK : le code est rechargé dans l'éditeur ouvert.");
    return;
  }
  if (/NotStarted/i.test(state)) {
    fail(
      "\nLive Coding n'a pas pu démarrer (désactivé dans Préférences de l'éditeur → Live Coding, ou module jamais compilé). Demande à l'utilisateur de fermer l'éditeur, puis relance build (compilation complète).",
    );
  }
  const { errors } = summarize(text);
  for (const e of errors.slice(0, 60)) console.log(e.trim());
  fail(
    "\nLive Coding a échoué. Corrige puis relance build. Si tu as ajouté une classe, un UPROPERTY/UFUNCTION ou modifié un .h en profondeur : demande à l'utilisateur de fermer l'éditeur, puis relance build (compilation complète).",
  );
}

async function build() {
  if (!hasCppModule()) fail("Pas de module C++. Lance d'abord : node tools/lumen-unreal.mjs init-cpp");
  const mcp = await mcpSession();
  if (mcp) {
    console.log("Éditeur ouvert : compilation par Live Coding.");
    await liveCoding(mcp);
    return;
  }
  const { code, output } = await runBuildBat();
  const { errors, warnings } = summarize(output);
  if (/Live Coding|Unable to build while/i.test(output) && code !== 0) {
    fail(
      "L'éditeur Unreal est ouvert mais son MCP ne répond pas, et Build.bat ne peut pas compiler pendant qu'il tourne. Demande à l'utilisateur de fermer l'éditeur (ou de lancer « ModelContextProtocol.StartServer » dans sa console), puis relance build.",
    );
  }
  if (/NetFxSDK/i.test(output) && code !== 0) {
    fail(
      "Il manque le « SDK .NET Framework 4.8 » de Visual Studio : Unreal en a besoin pour compiler le C++. Demande à l'utilisateur de l'ajouter (Visual Studio Installer → Modifier → Composants individuels → « SDK .NET Framework 4.8 », ou le bouton dans Lumen), puis relance build. Ne contourne pas.",
    );
  }
  if (code === 0) {
    console.log(`Compilation réussie (0 erreur${warnings.length ? `, ${warnings.length} avertissement(s)` : ""}).`);
    for (const w of warnings.slice(0, 20)) console.log(w.trim());
    return;
  }
  console.log(errors.length ? `${errors.length} erreur(s) :` : "La compilation a échoué :");
  for (const e of (errors.length ? errors : output.split(/\r?\n/).slice(-40)).slice(0, 60)) console.log(e.trim());
  process.exit(1);
}

async function listTools() {
  const mcp = await mcpSession();
  if (!mcp) fail(`Le MCP de l'éditeur ne répond pas sur ${MCP_URL}. Ouvre le projet dans Unreal.`);
  const listed = await mcp.call("tools/list", {});
  for (const tool of listed?.tools || []) console.log(`- ${tool.name} : ${(tool.description || "").split("\n")[0]}`);
}

const command = process.argv[2];
if (command === "status") await status();
else if (command === "init-cpp") initCpp();
else if (command === "build") await build();
else if (command === "tools") await listTools();
else fail("Usage : node tools/lumen-unreal.mjs status | init-cpp | build | tools");
