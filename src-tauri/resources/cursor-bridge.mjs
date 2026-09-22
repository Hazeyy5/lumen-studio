import { createInterface } from "node:readline";
import { Agent } from "@cursor/sdk";

const cwd = process.argv[2] || process.cwd();
const apiKey = process.env.CURSOR_API_KEY;

if (!apiKey) {
  process.stderr.write("CURSOR_API_KEY manquante. Ajoute-la dans Réglages Lumen.\n");
  process.exit(1);
}

process.stdout.write("Lumen × Cursor prêt. Décris le jeu, je construis.\n");

const rl = createInterface({ input: process.stdin, terminal: false });
let agent = null;

async function ensureAgent() {
  if (agent) return agent;
  agent = await Agent.create({
    apiKey,
    model: { id: "composer-2.5" },
    local: { cwd },
  });
  return agent;
}

rl.on("line", async (line) => {
  const prompt = line.trim();
  if (!prompt) return;
  try {
    const current = await ensureAgent();
    const run = await current.send(prompt);
    for await (const event of run.stream()) {
      if (event.type === "assistant") {
        for (const block of event.message.content) {
          if (block.type === "text") process.stdout.write(block.text);
        }
      }
    }
    const result = await run.wait();
    process.stdout.write(`\n\n[Lumen] run ${result.status}\n`);
  } catch (err) {
    process.stderr.write(`\n[Lumen] ${err?.message || err}\n`);
  }
});
