// Offline source audit. Runtime/build/tests never need the upstream checkout.
// Usage: bun crates/renderer/particle_inventory.ts /path/to/melee /tmp/inventory.json
import { join, relative } from "node:path";

const [root, output] = Bun.argv.slice(2);
if (!root || !output) throw new Error("usage: particle_inventory.ts UPSTREAM OUTPUT.json");
const lock = await Bun.file("upstream.lock.json").json();
const revision = Bun.spawnSync(["git", "-C", root, "rev-parse", "HEAD"]);
if (revision.exitCode || revision.stdout.toString().trim() !== lock.revision)
  throw new Error("upstream revision differs from upstream.lock.json");

// Remove comments and literals while retaining character offsets and lines.
function codeOnly(text: string): string {
  return text.replace(/\/\*[\s\S]*?\*\/|\/\/[^\n]*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'/g,
    token => token.replace(/[^\n]/g, " "));
}

function argumentsAt(text: string, start: number): string[] {
  const args: string[] = [];
  let depth = 0, begin = start;
  for (let i = start; i < text.length; ++i) {
    if (text[i] === "(") depth++;
    if (text[i] === ")") {
      if (depth === 0) { args.push(text.slice(begin, i).trim()); return args; }
      depth--;
    }
    if (text[i] === "," && depth === 0) {
      args.push(text.slice(begin, i).trim()); begin = i + 1;
    }
  }
  throw new Error("unterminated call");
}

const sites: object[] = [];
const effects = new Map<string, {kind: string, id: number, sites: number[]}>();
const glob = new Bun.Glob("src/melee/**/*.c");
for (const file of [...glob.scanSync({cwd: root})].sort()) {
  const source = await Bun.file(join(root, file)).text();
  const code = codeOnly(source);
  const calls = /\b(efLib_CreateGenerator\w*|efSync_Spawn|efAsync_Spawn|hsd_8039F05C|hsd_8039EFAC|hsd_8039F6CC)\s*\(/g;
  for (const match of code.matchAll(calls)) {
    const api = match[1];
    const args = argumentsAt(code, match.index! + match[0].length);
    // efAsync_Spawn(gobj, queue, spawn kind, ID, ...); hsd(link, bank, ID, ...).
    const arg = api === "efAsync_Spawn" ? 3 : api.startsWith("hsd_") ? 2 : 0;
    const expression = args[arg]?.replace(/\s+/g, " ") ?? "";
    const literal = /^(0[xX][\da-fA-F]+|\d+)[uUlL]*$/.exec(expression);
    const id = literal ? Number(literal[1]) : null;
    const kind = api.includes("Spawn") ? "dispatch" : "generator";
    const bank = api.startsWith("hsd_") ? args[1] : null;
    const index = sites.length;
    sites.push({file, line: source.slice(0, match.index).split("\n").length,
      api, kind, id, expression, bank_expression: bank});
    if (id !== null) {
      // HSD bank selection can be dynamic (not simply gfx_id / 1000).
      const key = `${kind}:${id}:${bank ?? "implicit"}`;
      if (!effects.has(key)) effects.set(key, {kind, id, sites: []});
      effects.get(key)!.sites.push(index);
    }
  }
}
const inventory = {
  schema: "skirmish-particle-source-audit-v1", upstream: lock,
  method: "Literal and dynamic arguments at selected Melee effect/generator API references; declarations may appear as dynamic entries.",
  limitations: [
    "Not a count of actual effects: direct dispatch IDs can resolve to mesh effects or composite spawns.",
    "Dynamic arguments, data-driven emitters, fighter animation commands and stage bank contents require native resource exports.",
    "No effect is marked implemented merely because it has a source reference.",
  ],
  counts: {references: sites.length, literal_keys: effects.size},
  effects: [...effects].sort(([a], [b]) => a.localeCompare(b)).map(([key, value]) => ({key, ...value})),
  references: sites,
};
await Bun.write(output, JSON.stringify(inventory, null, 2) + "\n");
console.log(`${relative(process.cwd(), output)}: ${sites.length} references, ${effects.size} literal keys`);
