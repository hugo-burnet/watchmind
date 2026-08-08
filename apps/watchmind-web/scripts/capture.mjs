import { chromium } from "playwright";

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ deviceScaleFactor: 1 });

await page.setViewportSize({ width: 1440, height: 1050 });
await page.goto("http://127.0.0.1:4173", { waitUntil: "networkidle" });
if (await page.locator(".work-card").count() === 0) {
  await page.getByLabel("Titre de l’œuvre").fill("Cowboy Bebop");
  await page.getByRole("button", { name: "Rechercher" }).click();
  await page.getByRole("button", { name: /Ajouter/ }).first().click();
  await page.locator(".work-card").first().waitFor();
}
await page.screenshot({ path: "../../docs/screenshots/watchmind-l16-desktop.png", fullPage: true });

await page.setViewportSize({ width: 390, height: 844 });
await page.reload({ waitUntil: "networkidle" });
await page.screenshot({ path: "../../docs/screenshots/watchmind-l16-mobile.png", fullPage: true });

await page.locator(".work-card").first().click();
await page.getByRole("button", { name: "Terminée" }).click();
await page.locator('input[type="range"]').fill("8.5");
await page.getByRole("button", { name: "Récit" }).click();
await page.getByRole("button", { name: "Mise en scène" }).click();
await page.getByLabel("Une phrase pour vous, si utile").fill("Une mélancolie qui reste longtemps.");
await page.getByRole("button", { name: "Enregistrer" }).click();
await page.getByText("8.5", { exact: true }).waitFor();

await browser.close();
