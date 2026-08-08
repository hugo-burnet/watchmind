import { chromium } from "playwright";

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ deviceScaleFactor: 1 });

await page.setViewportSize({ width: 1440, height: 1050 });
await page.goto("http://127.0.0.1:4173", { waitUntil: "networkidle" });
await page.getByRole("button", { name: "Bibliothèque" }).click();

if (await page.locator(".work-card").count() < 2) {
  await page.getByLabel("Titre de l’œuvre").fill("Cowboy Bebop");
  await page.getByRole("button", { name: "Rechercher" }).click();
  await page.getByRole("button", { name: /Ajouter/ }).nth(1).click();
  await page.locator(".work-card").nth(1).waitFor();
  await page.locator(".work-card").first().click();
  await page.getByRole("button", { name: "Enregistrer" }).click();
  await page.locator(".work-sheet").waitFor({ state: "detached" });
}

await page.getByRole("button", { name: "Aujourd’hui" }).click();
await page.locator(".recommendation-card").first().waitFor();
await page.screenshot({ path: "../../docs/screenshots/watchmind-l17-desktop.png", fullPage: true });
await page.getByRole("button", { name: "Cette recommandation est utile" }).first().click();

await page.setViewportSize({ width: 390, height: 844 });
await page.reload({ waitUntil: "networkidle" });
await page.locator(".recommendation-card").first().waitFor();
await page.screenshot({ path: "../../docs/screenshots/watchmind-l17-mobile.png", fullPage: true });

await page.getByRole("button", { name: "Carte de goût" }).click();
await page.locator(".profile-map").waitFor();
await page.screenshot({ path: "../../docs/screenshots/watchmind-l18-mobile.png", fullPage: true });
await page.setViewportSize({ width: 1440, height: 1050 });
await page.screenshot({ path: "../../docs/screenshots/watchmind-l18-desktop.png", fullPage: true });

await page.getByRole("button", { name: "Bibliothèque" }).click();
await page.locator(".work-card__open").first().click();
await page.getByRole("button", { name: "Marquer un rewatch" }).click();
await page.getByText("Rewatch enregistré.").waitFor();
await page.getByRole("button", { name: "Fermer la fiche" }).click();
await page.locator(".work-card__open").first().click();
await page.locator(".rewatch-count").waitFor();
await page.getByRole("button", { name: "Fermer la fiche" }).click();
const cardCount = await page.locator(".work-card").count();
await page.getByRole("button", { name: /^Supprimer .* de la bibliothèque$/ }).last().click();
await page.getByRole("button", { name: "Confirmer" }).click();
await page.locator(".work-card").nth(cardCount - 1).waitFor({ state: "detached" });

await browser.close();
