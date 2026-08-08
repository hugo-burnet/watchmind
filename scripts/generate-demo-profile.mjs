import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const [outputDirectory, ...cacheFiles] = process.argv.slice(2);
if (!outputDirectory || cacheFiles.length === 0) {
  throw new Error("usage: node scripts/generate-demo-profile.mjs <output-dir> <cache...>");
}

const preferences = new Map([
  ["Psychological", 2.0], ["Mystery", 1.4], ["Urban Fantasy", 1.5],
  ["Vampire", 1.2], ["Coming of Age", 0.9], ["Philosophy", 1.2],
  ["Detective", 1.1], ["Surreal Comedy", 0.8], ["Romance", 0.8],
  ["Sports", -1.5], ["War", -0.9], ["Primarily Child Cast", -1.0],
  ["Ecchi", -0.7], ["Idol", -0.6],
]);
const formatMap = { TV: "tv", TV_SHORT: "tv", MOVIE: "movie", OVA: "ova", ONA: "ona", SPECIAL: "special", MUSIC: "music" };
const clamp = (value, minimum, maximum) => Math.max(minimum, Math.min(maximum, value));
const jitter = (id) => (((id * 2654435761) >>> 0) % 301) / 100 - 1.5;

const media = [];
for (const file of cacheFiles) {
  const wrapper = JSON.parse(await readFile(file, "utf8"));
  media.push(...JSON.parse(wrapper.payload).data.Page.media);
}
const unique = [...new Map(media.map((item) => [item.id, item])).values()].slice(0, 150);
if (unique.length !== 150) throw new Error(`150 œuvres distinctes attendues, ${unique.length} reçues`);

const catalog = unique.map((item) => {
  const tags = item.tags.filter((tag) => !tag.isMediaSpoiler && tag.rank > 0).map((tag) => ({ name: tag.name, weight: tag.rank / 100 }));
  const work = {
    id: item.id,
    title: item.title.userPreferred ?? item.title.english ?? item.title.romaji ?? item.title.native ?? `AniList #${item.id}`,
    global_score: item.averageScore == null ? null : item.averageScore / 10,
    tags,
  };
  const runtime = item.duration && item.episodes ? item.duration * item.episodes : item.duration;
  if (runtime > 0) work.runtime_minutes = runtime;
  if (formatMap[item.format]) work.format = formatMap[item.format];
  if (item.startDate?.year >= 1900) work.release_year = item.startDate.year;
  const studios = item.studios?.nodes?.map((studio) => studio.name).filter(Boolean) ?? [];
  if (studios.length) work.studios = studios;
  return work;
});

const rows = ["work_id,rating,status,drop_position,total_episodes,rewatches"];
const distribution = { completed: 0, dropped: 0, rewatches: 0 };
for (const work of catalog) {
  const preference = work.tags.reduce((sum, tag) => sum + (preferences.get(tag.name) ?? 0) * tag.weight, 0);
  const rating = Math.round(clamp(6 + preference + jitter(work.id), 2, 10) * 2) / 2;
  const dropped = rating < 5;
  const rewatches = rating >= 9 ? work.id % 3 : 0;
  distribution[dropped ? "dropped" : "completed"] += 1;
  distribution.rewatches += rewatches;
  rows.push(dropped
    ? `${work.id},${rating},dropped,${1 + work.id % 4},12,0`
    : `${work.id},${rating},completed,,,${rewatches}`);
}

await mkdir(outputDirectory, { recursive: true });
await writeFile(join(outputDirectory, "catalog.json"), `${JSON.stringify(catalog, null, 2)}\n`);
await writeFile(join(outputDirectory, "ratings.csv"), `${rows.join("\n")}\n`);
await writeFile(join(outputDirectory, "persona.json"), `${JSON.stringify({ works: catalog.length, distribution, preferences: Object.fromEntries(preferences) }, null, 2)}\n`);
console.log(JSON.stringify({ outputDirectory, works: catalog.length, distribution }, null, 2));
