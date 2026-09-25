import { writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const cmd = process.argv[2];
if (cmd !== "drop-pack") {
  console.log("Usage : node tools/lumen-studio.mjs drop-pack");
  process.exit(cmd ? 1 : 0);
}

const dir = join(process.cwd(), "assets", "ui");
mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, "drop-pack"), "1\n");
console.log(
  "Demande envoyée. Avec Lumen ouvert et Studio rouvert, le plugin retire le pack de la place. L'UI du jeu dans src/client reste.",
);
