import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const carsDir = join(root, "assets", "cars");

function names() {
  if (!existsSync(carsDir)) return [];
  return readdirSync(carsDir)
    .filter((file) => file.toLowerCase().endsWith(".rbxmx"))
    .map((file) => file.slice(0, -6))
    .sort((a, b) => a.localeCompare(b));
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

const [cmd, ...rest] = process.argv.slice(2);
const list = names();

if (!cmd || cmd === "list") {
  if (!list.length) fail("Aucune voiture dans assets/cars.");
  for (const name of list) console.log(name);
  process.exit(0);
}

if (cmd !== "place") {
  fail("Usage :\n  node tools/lumen-car.mjs list\n  node tools/lumen-car.mjs place Civic");
}

const want = rest.join(" ").trim();
if (!want) fail("Donne le nom de la voiture. node tools/lumen-car.mjs list");
const match = list.find((name) => name.toLowerCase() === want.toLowerCase());
if (!match) {
  console.error("Voiture inconnue. Noms :");
  for (const name of list) console.error(name);
  process.exit(1);
}

const projectFile = join(root, "default.project.json");
if (!existsSync(projectFile)) fail("default.project.json introuvable.");
const data = JSON.parse(readFileSync(projectFile, "utf8"));
const tree = data.tree || (data.tree = {});
const workspace = tree.Workspace || (tree.Workspace = { $className: "Workspace" });
const pack = workspace.CarPack || (workspace.CarPack = { $className: "Folder" });
if (!pack.$className) pack.$className = "Folder";
pack[match] = { $path: `assets/cars/${match}.rbxmx` };
writeFileSync(projectFile, JSON.stringify(data, null, 2) + "\n");
console.log(`Voiture ${match} branchée dans Workspace.CarPack.${match}.`);
console.log("Au prochain sync Rojo elle apparaît à sa position du pack. Déplace-la avec PivotTo. Ne pose pas les autres.");
