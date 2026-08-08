import { readdir, stat } from "node:fs/promises";
import { join } from "node:path";

const limits = { ".js": 250_000, ".css": 40_000 };
const assets = join(import.meta.dirname, "..", "dist", "assets");
for (const file of await readdir(assets)) {
  const extension = Object.keys(limits).find((candidate) => file.endsWith(candidate));
  if (!extension) continue;
  const bytes = (await stat(join(assets, file))).size;
  if (bytes > limits[extension]) throw new Error(`${file}: ${bytes} octets dépasse le budget de ${limits[extension]}`);
  console.log(`${file}: ${bytes}/${limits[extension]} octets`);
}
